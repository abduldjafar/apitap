// src/utils/http_fetcher.rs
use crate::errors::{Error, Result};
use crate::utils::datafusion_ext::{
    DataFrameExt, DataWriter, JsonValueExt, QueryResult, QueryResultStream,
};
use async_trait::async_trait;
use futures::{Stream, TryStreamExt};
use futures::stream::{self, BoxStream, StreamExt};
use reqwest::Client;
use serde_json::Value;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use tokio_util::{io::StreamReader, codec::{FramedRead, LinesCodec}};


pub async fn ndjson_stream_page(
    client: &reqwest::Client,
    url: &str,
    page_param: &str,
    page: u64,
) -> Result<BoxStream<'static, Result<Value>>> {
    let resp = client
        .get(url)
        .query(&[(page_param, &page.to_string())])
        .send()
        .await
        .map_err(|e| Error::Reqwest(e.to_string()))?
        .error_for_status()
        .map_err(|e| Error::Reqwest(e.to_string()))?;

    // reqwest stream -> AsyncRead -> lines
    let byte_stream = resp
        .bytes_stream()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e));
    let reader = StreamReader::new(byte_stream);

    // If you want to cap max line length:
    // let lines = FramedRead::new(reader, LinesCodec::new_with_max_length(1_000_000));
    let mut lines = FramedRead::new(reader, LinesCodec::new());

    // Build a streaming adapter that can yield multiple items per line.
    let s = async_stream::try_stream! {
        while let Some(line_res) = lines.next().await {
            let line = line_res.map_err(|e| Error::Io(e.to_string()))?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let v: Value = serde_json::from_str(trimmed)
                .map_err(|e| Error::SerdeJson(e.to_string()))?;

            // Prefer /data if present
            if let Some(inner) = v.pointer("/data") {
                if let Some(arr) = inner.as_array() {
                    for item in arr {
                        // clone since serde_json::Value is owned
                        yield item.clone();
                    }
                } else if !inner.is_null() {
                    yield inner.clone();
                }
                // if /data is null or missing, fall through to handle v itself
                else {
                    // nothing to yield from /data
                }
            } else {
                // No /data wrapper; if the whole line is an array, flatten it
                if let Some(arr) = v.as_array() {
                    for item in arr {
                        yield item.clone();
                    }
                } else {
                    yield v;
                }
            }
        }
    };

    Ok(s.boxed())
}

//============================== Page Writer Trait ============================//

/// Writer for streaming API pages (similar to DataWriter but for HTTP)
#[async_trait]
pub trait PageWriter: Send + Sync {
    /// Write a page of data
    async fn write_page(&self, page_number: u64, data: Vec<Value>) -> Result<()>;

    async fn write_page_stream(
        &self,
        _page_number: u64,
        _stream_data: Pin<Box<dyn Stream<Item = Result<Value>> + Send>>,
    ) -> Result<()> {
        Ok(())
    }

    /// Handle page fetch errors
    async fn on_page_error(&self, page_number: u64, error: String) -> Result<()> {
        eprintln!("❌ Error fetching page {}: {}", page_number, error);
        Ok(())
    }

    /// Called before fetching starts
    async fn begin(&self) -> Result<()> {
        Ok(())
    }

    /// Called after all pages complete
    async fn commit(&self) -> Result<()> {
        Ok(())
    }
}

//======================== Streaming Paginated Fetcher ========================//

pub struct PaginatedFetcher {
    client: Client,
    base_url: String,
    page_param_name: String,
    concurrency: usize,
}

impl PaginatedFetcher {
    pub fn new(
        client: Client,
        base_url: impl Into<String>,
        page_param_name: impl Into<String>,
        concurrency: usize,
    ) -> Self {
        Self {
            client,
            base_url: base_url.into(),
            page_param_name: page_param_name.into(),
            concurrency,
        }
    }

    /// Fetch all pages and stream to writer (NO memory accumulation!)
   pub async fn fetch_to_writer(&self, writer: Arc<dyn PageWriter + Send + Sync>) -> Result<FetchStats> {
        
        const BATCH_SIZE: usize = 256; // tune for your sink throughput

        writer.begin().await?;

        // 1) Discover total_pages from page 1 (JSON-with-metadata)
        let first: Value = self.client
            .get(&self.base_url)
            .query(&[(&self.page_param_name, "1")])
            .send().await
            .map_err(|e| Error::Reqwest(e.to_string()))?
            .error_for_status()
            .map_err(|e| Error::Reqwest(e.to_string()))?
            .json().await
            .map_err(|e| Error::Reqwest(e.to_string()))?;

        let total_pages = first
            .pointer("/metadata/pagination/pages")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);

        println!("📄 Total pages: {total_pages}");

        let stats = Arc::new(tokio::sync::RwLock::new(FetchStats::new()));

        // If first page also exposes a data array (non-NDJSON), write it now (optional)
        if let Some(arr) = first.pointer("/data").and_then(|v| v.as_array()).cloned() {
            let n = arr.len();
            writer.write_page(1, arr).await?;
            stats.write().await.add_page(1, n);
        } else {
            // Or stream page 1 as NDJSON if the endpoint supports it:
            let mut s = ndjson_stream_page(&self.client, &self.base_url, &self.page_param_name, 1)
                .await
                .map_err(|e| Error::Reqwest(e.to_string()))?;
            let mut buf = Vec::with_capacity(BATCH_SIZE);
            while let Some(item) = s.next().await {
                match item {
                    Ok(v) => {
                        buf.push(v);
                        if buf.len() == BATCH_SIZE {
                            let count = buf.len();
                            let out = std::mem::take(&mut buf);
                            writer.write_page(1, out).await?;
                            stats.write().await.add_page(1, count);
                        }
                    }
                    Err(e) => {
                        stats.write().await.add_error(1);
                        let _ = writer.on_page_error(1, e.to_string()).await;
                    }
                }
            }
            if !buf.is_empty() {
                let count = buf.len();
                let out = std::mem::take(&mut buf);
                writer.write_page(1, out).await?;
                stats.write().await.add_page(1, count);
            }
        }

        if total_pages <= 1 {
            writer.commit().await?;
            return Ok(stats.read().await.clone());
        }

        // 2) Stream remaining pages concurrently (true pagination: 2..=total_pages)
        let client = self.client.clone();
        let url = self.base_url.clone();
        let page_param = self.page_param_name.clone();
        let stats_ref = Arc::clone(&stats);
        let writer_tasks = Arc::clone(&writer);

        stream::iter(2..=total_pages)
            .map(move |page| {
                let client = client.clone();
                let url = url.clone();
                let page_param = page_param.clone();
                let writer = Arc::clone(&writer_tasks);
                let stats = Arc::clone(&stats_ref);

                async move {
                    let stream_res = ndjson_stream_page(&client, &url, &page_param, page).await;
                    let mut s = match stream_res {
                        Ok(s) => s,
                        Err(e) => {
                            stats.write().await.add_error(page);
                            let _ = writer.on_page_error(page, e.to_string()).await;
                            return;
                        }
                    };

                    let mut buf = Vec::with_capacity(BATCH_SIZE);
                    while let Some(item) = s.next().await {
                        match item {
                            Ok(v) => {
                                buf.push(v);
                                if buf.len() == 50 {
                                    let count = buf.len();
                                    let out = std::mem::take(&mut buf);
                                    
                                    match writer.write_page(page, out).await {
                                        Ok(_) => stats.write().await.add_page(page, count),
                                        Err(e) => {
                                            stats.write().await.add_error(page);
                                            let _ = writer.on_page_error(page, e.to_string()).await;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                stats.write().await.add_error(page);
                                let _ = writer.on_page_error(page, e.to_string()).await;
                            }
                        }
                    }

                    if !buf.is_empty() {
                        let count = buf.len();
                        let out = std::mem::take(&mut buf);
                        match writer.write_page(page, out).await {
                            Ok(_) => stats.write().await.add_page(page, count),
                            Err(e) => {
                                stats.write().await.add_error(page);
                                let _ = writer.on_page_error(page, e.to_string()).await;
                            }
                        }
                    }

                    //println!("✅ Page {page}/{total_pages} streamed");
                }
            })
            .buffer_unordered(self.concurrency)
            .collect::<Vec<_>>()
            .await;

        // 3) Finish
        writer.commit().await?;

        Ok(stats.read().await.clone())
    }
}

//=============================== Fetch Stats =================================//

#[derive(Debug, Clone)]
pub struct FetchStats {
    pub success_count: usize,
    pub error_count: usize,
    pub total_items: usize,
}

impl FetchStats {
    fn new() -> Self {
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

    fn add_error(&mut self, _page: u64) {
        self.error_count += 1;
    }
}

//========================== Example: DataFusion Writer =======================//

/// Writer that takes API pages → DataFusion → Final destination
pub struct DataFusionPageWriter {
    table_name: String,
    sql: String,
    final_writer: Arc<dyn DataWriter>,
    accumulated: tokio::sync::RwLock<Vec<Value>>,
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
            accumulated: tokio::sync::RwLock::new(Vec::new()),
        }
    }
}

#[async_trait]
impl PageWriter for DataFusionPageWriter {
    async fn write_page_stream(
        &self,
        page_number: u64,
        stream_data: Pin<Box<dyn Stream<Item = Result<Value>> + Send>>,
    ) -> Result<()> {
        Ok(())
    }

    async fn write_page(&self, page_number: u64, data: Vec<Value>) -> Result<()> {
        // Option 1: Process each page immediately
        // (Use this if you want true streaming)
        use crate::utils::datafusion_ext::JsonValueExt;

        let json_array = Value::Array(data);
        let sdf = json_array.to_sql(&self.table_name, &self.sql).await?;
        let mut result_json = sdf.inner().to_stream().await?;


        self.final_writer
            .write_stream(QueryResultStream {
                table_name: format!("{}_page_{}", self.table_name, page_number),
                data: result_json,
            })
            .await?;


        Ok(())
    }

    async fn commit(&self) -> Result<()> {
        self.final_writer.commit().await
    }
}

//======================= Example: Batched Writer =============================//

/// Accumulate N pages, then process batch
pub struct BatchedPageWriter {
    batch_size: usize,
    table_name: String,
    sql: String,
    final_writer: Arc<dyn DataWriter>,
    buffer: tokio::sync::RwLock<Vec<Value>>,
}

impl BatchedPageWriter {
    pub fn new(
        batch_size: usize,
        table_name: impl Into<String>,
        sql: impl Into<String>,
        final_writer: Arc<dyn DataWriter>,
    ) -> Self {
        Self {
            batch_size,
            table_name: table_name.into(),
            sql: sql.into(),
            final_writer,
            buffer: tokio::sync::RwLock::new(Vec::new()),
        }
    }

    async fn flush(&self) -> Result<()> {
        use crate::utils::datafusion_ext::JsonValueExt;

        let mut buffer = self.buffer.write().await;
        if buffer.is_empty() {
            return Ok(());
        }

        let json_array = Value::Array(buffer.drain(..).collect());
        let sdf = json_array.to_sql(&self.table_name, &self.sql).await?;
        let result_json = sdf.inner().to_json().await?;

        let row_count = result_json.as_array().map(|a| a.len()).unwrap_or(0);

        self.final_writer
            .write(QueryResult {
                table_name: self.table_name.clone(),
                data: result_json,
                row_count,
            })
            .await?;

        Ok(())
    }
}

#[async_trait]
impl PageWriter for BatchedPageWriter {
    async fn write_page(&self, _page_number: u64, data: Vec<Value>) -> Result<()> {
        let mut buffer = self.buffer.write().await;
        buffer.extend(data);

        // Flush if batch size reached
        if buffer.len() >= self.batch_size {
            drop(buffer); // Release lock before flush
            self.flush().await?;
        }

        Ok(())
    }

    async fn commit(&self) -> Result<()> {
        // Flush any remaining data
        self.flush().await?;
        self.final_writer.commit().await
    }
}

//======================== Example: Simple Memory Writer ======================//

/// Collects all data in memory (for small datasets or testing)
pub struct MemoryPageWriter {
    data: tokio::sync::RwLock<Vec<Value>>,
}

impl MemoryPageWriter {
    pub fn new() -> Self {
        Self {
            data: tokio::sync::RwLock::new(Vec::new()),
        }
    }

    pub async fn into_data(self) -> Vec<Value> {
        self.data.into_inner()
    }

    pub async fn get_data(&self) -> Vec<Value> {
        self.data.read().await.clone()
    }
}

#[async_trait]
impl PageWriter for MemoryPageWriter {
    async fn write_page(&self, _page_number: u64, data: Vec<Value>) -> Result<()> {
        self.data.write().await.extend(data);
        Ok(())
    }
}
