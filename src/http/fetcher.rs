use crate::errors::{ApitapError, Result};
use crate::utils::datafusion_ext::{DataFrameExt, JsonValueExt, QueryResultStream};
use crate::utils::http_retry;
use crate::writer::{DataWriter, WriteMode};
use async_trait::async_trait;
use futures::Stream;
use futures::stream::{self, BoxStream, StreamExt, TryStreamExt};
use reqwest::Client;
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::pin::Pin;
use std::sync::Arc;
use tokio_util::{
    codec::{FramedRead, LinesCodec},
    io::StreamReader,
};
use tracing::{debug, debug_span, error, info, info_span, trace};

// =========================== NDJSON helper ===================================

/// Stream an HTTP response as NDJSON and flatten an optional JSON pointer (`/data`, etc.).
/// If `data_path` is None, it will try to flatten the top-level array; otherwise it yields the object.
pub async fn ndjson_stream_qs(
    client: &reqwest::Client,
    url: &str,
    query: &[(String, String)],
    data_path: Option<&str>,
    config_retry: &crate::pipeline::Retry,
) -> Result<BoxStream<'static, Result<Value>>> {
    // Instrument HTTP/NDJSON parsing for tracing with source and optional data_path
    let span = info_span!("http.ndjson_stream", source = %url, query_len = query.len());
    let _g = span.enter();
    let client_with_retry = http_retry::build_client_with_retry(client.clone(), config_retry);

    // Instrument the HTTP request/response at debug level with timing and status
    let req_span =
        debug_span!("http.request", method = "GET", source = %url, query_len = query.len());
    let _req_g = req_span.enter();
    let started = std::time::Instant::now();

    let resp = client_with_retry.get(url).query(query).send().await?;

    let status = resp.status();
    let elapsed = started.elapsed();
    debug!(status = %status, elapsed_ms = elapsed.as_millis(), "http response received");

    let resp = resp.error_for_status()?;

    // Heuristic: treat as NDJSON only if content-type says so
    let is_ndjson = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .map(|ct| ct.contains("ndjson") || ct.contains("x-ndjson"))
        .unwrap_or(false);

    if !is_ndjson {
        // -------- Regular JSON (object or array) path --------
        let bytes = resp.bytes().await.map_err(|e| ApitapError::Reqwest(e))?;
        let v: Value = serde_json::from_slice(&bytes).map_err(|e| ApitapError::SerdeJson(e))?;

        // If data_path is provided, drill into it; else use the whole value.
        let target = if let Some(p) = data_path {
            v.pointer(p).cloned().unwrap_or(Value::Null)
        } else {
            v
        };

        let items: Vec<Value> = if let Some(arr) = target.as_array() {
            arr.clone()
        } else if target.is_null() {
            Vec::new()
        } else {
            vec![target]
        };

        debug!(items = items.len(), "parsed JSON response items");

        // Emit as a stream of Values
        let st = stream::iter(items.into_iter().map(Ok)).boxed();
        return Ok(st);
    }

    // -------- NDJSON path (one JSON per line) --------
    let byte_stream = resp
        .bytes_stream()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));

    let reader = StreamReader::new(byte_stream);
    let lines = FramedRead::new(reader, LinesCodec::new());
    let data_path_owned = data_path.map(|s| s.to_owned());

    let s = async_stream::try_stream! {
        let mut lines = lines;
        while let Some(line_res) = lines.next().await {
            let line = line_res.map_err(|e| ApitapError::LinesCodecError(e))?;
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }

            trace!(len = trimmed.len(), "ndjson line");

            let v: Value = serde_json::from_str(trimmed)
                .map_err(|e| ApitapError::SerdeJson(e))?;

            if let Some(ref p) = data_path_owned {
                if let Some(inner) = v.pointer(p) {
                    if let Some(arr) = inner.as_array() {
                        for item in arr { yield item.clone(); }
                    } else if !inner.is_null() {
                        yield inner.clone();
                    }
                    continue;
                }
            }

            if let Some(arr) = v.as_array() {
                for item in arr { yield item.clone(); }
            } else {
                yield v;
            }
        }
    };
    Ok(s.boxed())
}

// =============================== Page Writer =================================

#[async_trait]
pub trait PageWriter: Send + Sync {
    async fn write_page(
        &self,
        page_number: u64,
        data: Vec<Value>,
        _write_mode: WriteMode,
    ) -> Result<()>;

    async fn write_page_stream(
        &self,
        _page_number: u64,
        _stream_data: Pin<Box<dyn Stream<Item = Result<Value>> + Send>>,
    ) -> Result<()> {
        Ok(())
    }

    async fn on_page_error(&self, page_number: u64, error: String) -> Result<()> {
        error!(page = page_number, %error, "error fetching page");
        Ok(())
    }

    async fn begin(&self) -> Result<()> {
        Ok(())
    }
    async fn commit(&self) -> Result<()> {
        Ok(())
    }
}

// =========================== Pagination types ================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Pagination {
    LimitOffset {
        limit_param: String,
        offset_param: String,
    },
    PageNumber {
        page_param: String,
        per_page_param: String,
    },
    PageOnly {
        page_param: String,
    },
    Cursor {
        cursor_param: String,
        page_size_param: Option<String>,
    },
    Default,
}

/// Hint to compute total pages.
/// - Items: pointer points to total items; pages = ceil(items/limit)
/// - Pages:  pointer points directly to total pages
#[derive(Debug, Clone)]
pub enum TotalHint {
    Items { pointer: String },
    Pages { pointer: String },
}

// =========================== Fetcher =========================================

pub struct PaginatedFetcher {
    client: Client,
    base_url: String,
    concurrency: usize,
    pagination_config: Pagination,
    batch_size: usize,
}

impl PaginatedFetcher {
    pub fn new(client: Client, base_url: impl Into<String>, concurrency: usize) -> Self {
        Self {
            client,
            base_url: base_url.into(),
            concurrency,
            pagination_config: Pagination::Default,
            batch_size: 256,
        }
    }

    pub fn with_limit_offset(
        mut self,
        limit_param: impl Into<String>,
        offset_param: impl Into<String>,
    ) -> Self {
        self.pagination_config = Pagination::LimitOffset {
            limit_param: limit_param.into(),
            offset_param: offset_param.into(),
        };
        self
    }

    pub fn with_page_number(
        mut self,
        page_param: impl Into<String>,
        per_page_param: impl Into<String>,
    ) -> Self {
        self.pagination_config = Pagination::PageNumber {
            page_param: page_param.into(),
            per_page_param: per_page_param.into(),
        };
        self
    }

    pub fn with_batch_size(mut self, n: usize) -> Self {
        self.batch_size = n.max(1);
        self
    }

    // -------------------- Public entry points --------------------------------

    /// LIMIT/OFFSET mode. If `total_hint` is None, it fetches until a page yields 0 rows.
    pub async fn fetch_limit_offset(
        &self,
        limit: u64,
        data_path: Option<&str>,
        total_hint: Option<TotalHint>,
        writer: Arc<dyn PageWriter>,
        write_mode: WriteMode,
        config_retry: &crate::pipeline::Retry,
    ) -> Result<FetchStats> {
        let (limit_param, offset_param) = match &self.pagination_config {
            Pagination::LimitOffset {
                limit_param,
                offset_param,
            } => (limit_param.clone(), offset_param.clone()),
            other => {
                return Err(ApitapError::PaginationError(format!(
                    "Pagination::LimitOffset not configured {other:?}"
                )));
            }
        };

        // Span for the fetch operation (fetch → pages → writes)
        let span = info_span!("fetch.limit_offset", source = %self.base_url, limit = limit);
        let _g = span.enter();

        writer.begin().await?;

        // ---- First request (offset=0) as JSON to read totals; also process it ----
        let first_json: Value = self
            .client
            .get(&self.base_url)
            .query(&[(limit_param.as_str(), limit.to_string())])
            .query(&[(offset_param.as_str(), "0")])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let mut stats = FetchStats::new();

        // Write first page immediately (array path or stream fallback)
        let mut wrote_first = false;
        if let Some(p) = data_path {
            if let Some(arr) = first_json.pointer(p).and_then(|v| v.as_array()).cloned() {
                let n = arr.len();
                writer.write_page(0, arr, write_mode.clone()).await?;
                info!(page = 0, items = n, source = %self.base_url, "wrote first page (json array path)");
                stats.add_page(0, n);
                wrote_first = true;
            }
        }
        if !wrote_first {
            // Fallback: NDJSON stream with limit/offset=0
            let mut s = ndjson_stream_qs(
                &self.client,
                &self.base_url,
                &[
                    (limit_param.clone(), limit.to_string()),
                    (offset_param.clone(), "0".into()),
                ],
                data_path,
                config_retry,
            )
            .await?;
            let wrote = self
                .write_streamed_page(0, &mut s, &*writer, &mut stats, write_mode.clone())
                .await?;
            info!(page = 0, items = wrote, source = %self.base_url, "wrote first page (stream path)");
        }

        // Determine total pages if possible
        let pages_opt = match total_hint {
            Some(TotalHint::Items { ref pointer }) => first_json
                .pointer(pointer)
                .and_then(|v| v.as_u64())
                .map(|total_items| (total_items + limit - 1) / limit),
            Some(TotalHint::Pages { ref pointer }) => {
                first_json.pointer(pointer).and_then(|v| v.as_u64())
            }
            None => None,
        };

        // If we know pages, parallel fetch the rest; else loop until empty page.
        let result = if let Some(pages) = pages_opt {
            // already did offset=0; iterate i=1..pages (offset = i*limit)
            self.fetch_remaining_known_pages_limit_offset(
                pages,
                limit,
                data_path,
                &limit_param,
                &offset_param,
                writer.clone(),
                &mut stats,
                write_mode.clone(),
                config_retry,
            )
            .await
        } else {
            // Unknown total: keep stepping offsets until a page yields 0 items
            self.fetch_until_empty_limit_offset(
                limit,
                data_path,
                &limit_param,
                &offset_param,
                writer.clone(),
                &mut stats,
                write_mode.clone(),
                config_retry,
            )
            .await
        };

        let _ = result; // propagate errors if any
        writer.commit().await?;
        Ok(stats)
    }

    /// PAGE/PER_PAGE mode.
    pub async fn fetch_page_number(
        &self,
        per_page: u64,
        data_path: Option<&str>,
        total_hint: Option<TotalHint>,
        writer: Arc<dyn PageWriter>,
        write_mode: WriteMode,
        config_retry: &crate::pipeline::Retry,
    ) -> Result<FetchStats> {
        let (page_param, per_page_param) = match &self.pagination_config {
            Pagination::PageNumber {
                page_param,
                per_page_param,
            } => (page_param.clone(), per_page_param.clone()),
            other => {
                return Err(ApitapError::PaginationError(format!(
                    "expected Pagination::PageNumber, got {other:?}"
                )));
            }
        };

        let span = info_span!("fetch.page_number", source = %self.base_url, per_page = per_page);
        let _g = span.enter();

        writer.begin().await?;

        // First request as JSON (page=1)
        let first_json: Value = self
            .client
            .get(&self.base_url)
            .query(&[(page_param.as_str(), "1".to_string())])
            .query(&[(per_page_param.as_str(), per_page.to_string())])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let mut stats = FetchStats::new();

        // Write page 1
        let mut wrote_first = false;
        if let Some(p) = data_path {
            if let Some(arr) = first_json.pointer(p).and_then(|v| v.as_array()).cloned() {
                let n = arr.len();
                writer.write_page(1, arr, write_mode.clone()).await?;
                info!(page = 1, items = n, source = %self.base_url, "wrote first page (json array path)");
                stats.add_page(1, n);
                wrote_first = true;
            }
        }
        if !wrote_first {
            let mut s = ndjson_stream_qs(
                &self.client,
                &self.base_url,
                &[
                    (page_param.clone(), "1".into()),
                    (per_page_param.clone(), per_page.to_string()),
                ],
                data_path,
                config_retry,
            )
            .await?;
            self.write_streamed_page(1, &mut s, &*writer, &mut stats, write_mode.clone())
                .await?;
        }

        // Determine total pages
        let pages_opt = match total_hint {
            Some(TotalHint::Items { ref pointer }) => first_json
                .pointer(pointer)
                .and_then(|v| v.as_u64())
                .map(|total_items| (total_items + per_page - 1) / per_page),
            Some(TotalHint::Pages { ref pointer }) => {
                first_json.pointer(pointer).and_then(|v| v.as_u64())
            }
            None => None,
        };

        if let Some(total_pages) = pages_opt {
            // pages 2..=total_pages
            let client = self.client.clone();
            let url = self.base_url.clone();
            let page_param_c = page_param.clone();
            let per_page_param_c = per_page_param.clone();
            let data_path_c = data_path.map(|s| s.to_string());
            let writer_ref = Arc::clone(&writer);
            let batch_size = self.batch_size;
            let write_mode_clone = write_mode.clone();

            stream::iter(2..=total_pages)
                .map(move |page| {
                    let client = client.clone();
                    let url = url.clone();
                    let page_param = page_param_c.clone();
                    let per_page_param = per_page_param_c.clone();
                    let data_path = data_path_c.clone();
                    let writer = Arc::clone(&writer_ref);
                    let write_mode_c = write_mode_clone.clone();

                    async move {
                        let mut s = match ndjson_stream_qs(
                            &client,
                            &url,
                            &[
                                (page_param, page.to_string()),
                                (per_page_param, per_page.to_string()),
                            ],
                            data_path.as_deref(),
                            config_retry,
                        )
                        .await
                        {
                            Ok(s) => s,
                            Err(e) => {
                                let _ = writer.on_page_error(page, e.to_string()).await;
                                return;
                            }
                        };
                        let mut buf = Vec::with_capacity(batch_size);
                        while let Some(item) = s.next().await {
                            match item {
                                Ok(v) => {
                                    buf.push(v);
                                    if buf.len() == batch_size {
                                        let out = std::mem::take(&mut buf);
                                        if let Err(e) =
                                            writer.write_page(page, out, write_mode_c.clone()).await
                                        {
                                            let _ = writer.on_page_error(page, e.to_string()).await;
                                        }
                                        trace!(page = page, batch = true, "wrote batch for page");
                                    }
                                }
                                Err(e) => {
                                    let _ = writer.on_page_error(page, e.to_string()).await;
                                }
                            }
                        }
                        if !buf.is_empty() {
                            let out = std::mem::take(&mut buf);
                            let cnt = out.len();
                            if let Err(e) = writer.write_page(page, out, write_mode_c.clone()).await
                            {
                                let _ = writer.on_page_error(page, e.to_string()).await;
                            } else {
                                info!(page = page, items = cnt, source = %url, "wrote page remainder");
                            }
                        }
                    }
                })
                .buffer_unordered(self.concurrency)
                .collect::<Vec<_>>()
                .await;
        } else {
            // Unknown total pages: fetch page=2,3,... until empty
            let mut page = 2u64;
            loop {
                let mut s = match ndjson_stream_qs(
                    &self.client,
                    &self.base_url,
                    &[
                        (page_param.clone(), page.to_string()),
                        (per_page_param.clone(), per_page.to_string()),
                    ],
                    data_path,
                    config_retry,
                )
                .await
                {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = writer.on_page_error(page, e.to_string()).await;
                        break;
                    }
                };

                let wrote = self
                    .write_streamed_page(page, &mut s, &*writer, &mut stats, write_mode.clone())
                    .await?;
                info!(page = page, items = wrote, source = %self.base_url, "wrote page (unknown total)");
                if wrote == 0 {
                    break;
                } // stop on empty page
                page += 1;
            }
        }

        writer.commit().await?;
        Ok(stats)
    }

    // -------------------- Private helpers ------------------------------------

    async fn write_streamed_page(
        &self,
        page: u64,
        s: &mut BoxStream<'static, Result<Value>>,
        writer: &dyn PageWriter,
        stats: &mut FetchStats,
        write_mode: WriteMode,
    ) -> Result<usize> {
        let mut buf = Vec::with_capacity(self.batch_size);
        let mut written = 0usize;

        while let Some(item) = s.next().await {
            match item {
                Ok(v) => {
                    buf.push(v);
                    if buf.len() == self.batch_size {
                        let count = buf.len();
                        let out = std::mem::take(&mut buf);
                        writer.write_page(page, out, write_mode.clone()).await?;
                        stats.add_page(page, count);
                        written += count;
                        trace!(
                            page = page,
                            batch_count = count,
                            total_written = written,
                            "wrote batch"
                        );
                    }
                }
                Err(e) => {
                    writer.on_page_error(page, e.to_string()).await?;
                }
            }
        }
        if !buf.is_empty() {
            let count = buf.len();
            let out = std::mem::take(&mut buf);
            writer.write_page(page, out, write_mode).await?;
            stats.add_page(page, count);
            written += count;
            trace!(
                page = page,
                batch_count = count,
                total_written = written,
                "wrote final batch"
            );
        }
        Ok(written)
    }

    async fn fetch_remaining_known_pages_limit_offset(
        &self,
        pages: u64, // total pages
        limit: u64,
        data_path: Option<&str>,
        limit_param: &str,
        offset_param: &str,
        writer: Arc<dyn PageWriter>,
        _stats: &mut FetchStats,
        write_mode: WriteMode,
        config_retry: &crate::pipeline::Retry,
    ) -> Result<()> {
        // We already wrote offset=0 ⇒ remaining i=1..pages-1 (offset = i*limit)
        let client = self.client.clone();
        let url = self.base_url.clone();
        let limit_param = limit_param.to_string();
        let offset_param = offset_param.to_string();
        let data_path_c = data_path.map(|s| s.to_string());
        let writer_ref = Arc::clone(&writer);
        let batch_size = self.batch_size;

        stream::iter(1..pages)
            .map(move |i| {
                let client = client.clone();
                let url = url.clone();
                let limit_param = limit_param.clone();
                let offset_param = offset_param.clone();
                let data_path = data_path_c.clone();
                let writer = Arc::clone(&writer_ref);
                let write_mode_clone = write_mode.clone();
                let offset = i * limit;

                async move {
                    let mut s = match ndjson_stream_qs(
                        &client,
                        &url,
                        &[
                            (limit_param, limit.to_string()),
                            (offset_param, offset.to_string()),
                        ],
                        data_path.as_deref(),
                        config_retry,
                    )
                    .await
                    {
                        Ok(s) => s,
                        Err(e) => {
                            let _ = writer.on_page_error(i, e.to_string()).await;
                            return;
                        }
                    };

                    let mut buf = Vec::with_capacity(batch_size);
                    while let Some(item) = s.next().await {
                        match item {
                            Ok(v) => {
                                buf.push(v);
                                if buf.len() == batch_size {
                                    let out = std::mem::take(&mut buf);
                                    if let Err(e) =
                                        writer.write_page(i, out, write_mode_clone.clone()).await
                                    {
                                        let _ = writer.on_page_error(i, e.to_string()).await;
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = writer.on_page_error(i, e.to_string()).await;
                            }
                        }
                    }
                    if !buf.is_empty() {
                        let out = std::mem::take(&mut buf);
                        if let Err(e) = writer.write_page(i, out, write_mode_clone.clone()).await {
                            let _ = writer.on_page_error(i, e.to_string()).await;
                        }
                    }
                }
            })
            .buffer_unordered(self.concurrency)
            .collect::<Vec<_>>()
            .await;

        Ok(())
    }

    async fn fetch_until_empty_limit_offset(
        &self,
        limit: u64,
        data_path: Option<&str>,
        limit_param: &str,
        offset_param: &str,
        writer: Arc<dyn PageWriter>,
        stats: &mut FetchStats,
        write_mode: WriteMode,
        config_retry: &crate::pipeline::Retry,
    ) -> Result<()> {
        let mut i = 1u64; // we already handled offset=0
        loop {
            let offset = i * limit;
            let mut s = match ndjson_stream_qs(
                &self.client,
                &self.base_url,
                &[
                    (limit_param.to_string(), limit.to_string()),
                    (offset_param.to_string(), offset.to_string()),
                ],
                data_path,
                config_retry,
            )
            .await
            {
                Ok(s) => s,
                Err(e) => {
                    writer.on_page_error(i, e.to_string()).await?;
                    break;
                }
            };

            let wrote = self
                .write_streamed_page(i, &mut s, &*writer, stats, write_mode.clone())
                .await?;
            if wrote == 0 {
                break;
            }
            i += 1;
        }
        Ok(())
    }
}

// ============================== Stats =======================================

#[derive(Debug, Clone)]
pub struct FetchStats {
    pub success_count: usize,
    pub error_count: usize,
    pub total_items: usize,
}
impl FetchStats {
    pub fn new() -> Self {
        Self {
            success_count: 0,
            error_count: 0,
            total_items: 0,
        }
    }
    fn add_page(&mut self, _page: u64, items: usize) {
        self.success_count += 1;
        self.total_items += items;
    }
    #[allow(dead_code)]
    fn add_error(&mut self, _page: u64) {
        self.error_count += 1;
    }
}

// ===================== Example Writers (unchanged in spirit) =================

pub struct DataFusionPageWriter {
    table_name: String,
    sql: String,
    final_writer: Arc<dyn DataWriter>,
}
impl DataFusionPageWriter {
    pub fn new(
        table_name: impl Into<String>,
        sql: impl Into<String>,
        final_writer: Arc<dyn DataWriter>,
    ) -> Self {
        Self {
            table_name: table_name.into(),
            sql: sql.into(),
            final_writer,
        }
    }
}

#[async_trait]
impl PageWriter for DataFusionPageWriter {
    async fn write_page(
        &self,
        page_number: u64,
        data: Vec<Value>,
        write_mode: WriteMode,
    ) -> Result<()> {
        // Span covering transform -> write for this page
        let items = data.len();
        let span = info_span!("transform.load", table = %self.table_name, page = page_number, items = items);
        let _g = span.enter();

        let json_array = Value::Array(data);
        let sdf = json_array.to_sql(&self.table_name, &self.sql).await?;
        let result_stream = sdf.inner().to_stream().await?;
        // Use structured fields for the downstream writer call
        let table_page = format!("{}_page_{}", self.table_name, page_number);
        info!(table = %self.table_name, table_page = %table_page, items = items, "transform -> load: writing page");
        self.final_writer
            .write_stream(
                QueryResultStream {
                    table_name: table_page,
                    data: result_stream,
                },
                write_mode,
            )
            .await?;
        Ok(())
    }
    async fn commit(&self) -> Result<()> {
        self.final_writer.commit().await
    }
}
