// src/utils/http_fetcher.rs
use crate::errors::{Error, Result};
use crate::utils::datafusion_ext::{DataFrameExt, DataWriter, JsonValueExt, QueryResult};
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use futures::Stream;
use reqwest::Client;
use serde_json::Value;
use std::pin::Pin;
use std::sync::Arc;

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
    pub async fn fetch_to_writer<W: PageWriter>(&self, writer: Arc<W>) -> Result<FetchStats> {
        writer.begin().await?;

        // 1. Fetch first page to get total pages
        let first: Value = self
            .client
            .get(&self.base_url)
            .query(&[(&self.page_param_name, "1")])
            .send()
            .await
            .map_err(|e| Error::Reqwest(e.to_string()))?
            .error_for_status()
            .map_err(|e| Error::Reqwest(e.to_string()))?
            .json()
            .await
            .map_err(|e| Error::Reqwest(e.to_string()))?;

        let first_data = first
            .pointer("/data")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let total_pages = first
            .pointer("/metadata/pagination/pages")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);

        println!("📄 Total pages: {}", total_pages);

        let stats = Arc::new(tokio::sync::RwLock::new(FetchStats::new()));

        // Write first page immediately
        let first_count = first_data.len();
        writer.write_page(1, first_data).await?;
        stats.write().await.add_page(1, first_count);

        if total_pages <= 1 {
            writer.commit().await?;
            return Ok(stats.read().await.clone());
        }

        // 2. Fetch remaining pages concurrently and stream to writer
        stream::iter(2..=total_pages)
            .map(|page| {
                let client = self.client.clone();
                let url = self.base_url.clone();
                let param_name = self.page_param_name.clone();
                let writer = writer.clone();
                let stats = stats.clone();

                async move {
                    match client
                        .get(&url)
                        .query(&[(param_name.as_str(), &page.to_string())])
                        .send()
                        .await
                    {
                        Ok(resp) => match resp.json::<Value>().await {
                            Ok(json) => {
                                if let Some(data) =
                                    json.pointer("/data").and_then(|v| v.as_array()).cloned()
                                {
                                    let count = data.len();

                                    // Write immediately - NO accumulation!
                                    match writer.write_page(page, data).await {
                                        Ok(_) => {
                                            stats.write().await.add_page(page, count);
                                            println!(
                                                "✅ Page {}/{} - {} items",
                                                page, total_pages, count
                                            );
                                        }
                                        Err(e) => {
                                            stats.write().await.add_error(page);
                                            eprintln!("❌ Write failed for page {}: {}", page, e);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                stats.write().await.add_error(page);
                                let _ = writer.on_page_error(page, e.to_string()).await;
                            }
                        },
                        Err(e) => {
                            stats.write().await.add_error(page);
                            let _ = writer.on_page_error(page, e.to_string()).await;
                        }
                    }
                }
            })
            .buffer_unordered(self.concurrency) // Max N concurrent requests
            .collect::<Vec<_>>()
            .await;

        writer.commit().await?;

        let final_stats = stats.read().await.clone();
        println!("\n🎉 Fetch completed:");
        println!("   ✅ Success: {} pages", final_stats.success_count);
        println!("   ❌ Errors: {} pages", final_stats.error_count);
        println!("   📊 Total items: {}", final_stats.total_items);

        Ok(final_stats)
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
        let result_json = sdf.inner().to_json().await?;

        let row_count = result_json.as_array().map(|a| a.len()).unwrap_or(0);

        self.final_writer
            .write(QueryResult {
                table_name: format!("{}_page_{}", self.table_name, page_number),
                data: result_json,
                row_count,
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
