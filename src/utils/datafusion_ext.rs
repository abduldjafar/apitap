// src/utils/datafusion_ext.rs
use async_trait::async_trait;
use futures::StreamExt;
use serde_arrow::schema::{SchemaLike, TracingOptions};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{OnceCell, RwLock, Semaphore};

use datafusion::{
    arrow::{datatypes::FieldRef, record_batch::RecordBatch},
    dataframe::DataFrame,
    execution::{
        context::SessionConfig,
        memory_pool::GreedyMemoryPool,
        runtime_env::{RuntimeConfig, RuntimeEnv},
    },
    prelude::*,
};

use crate::errors::{Error, Result};

//=========================== Shared SessionContext ===========================//

static SHARED_CTX: OnceCell<Arc<SessionContext>> = OnceCell::const_new();

async fn get_shared_context() -> Arc<SessionContext> {
    SHARED_CTX
        .get_or_init(|| async {
            let memory_pool = GreedyMemoryPool::new(256 * 1024 * 1024);
            let runtime_env =
                RuntimeEnv::new(RuntimeConfig::new().with_memory_pool(Arc::new(memory_pool)))
                    .expect("Failed to create runtime env");

            let session_config = SessionConfig::new()
                .with_target_partitions(1)
                .with_batch_size(2048);

            Arc::new(SessionContext::new_with_config_rt(
                session_config,
                Arc::new(runtime_env),
            ))
        })
        .await
        .clone()
}

//========================= RAII for temp table cleanup =======================//

pub struct SqlDataFrame {
    df: DataFrame,
    ctx: Arc<SessionContext>,
    table_name: String,
}

impl SqlDataFrame {
    pub fn inner(&self) -> &DataFrame {
        &self.df
    }
}

impl Drop for SqlDataFrame {
    fn drop(&mut self) {
        let _ = self.ctx.deregister_table(&self.table_name);
    }
}

//============================= JSON → DF / SQL ===============================//

#[async_trait]
pub trait JsonValueExt {
    async fn to_df(&self) -> Result<DataFrame>;
    async fn to_sql(&self, table_name: &str, sql: &str) -> Result<SqlDataFrame>;
}

#[async_trait]
impl JsonValueExt for serde_json::Value {
    async fn to_df(&self) -> Result<DataFrame> {
        let ctx = get_shared_context().await;

        let Self::Array(json_array) = self else {
            return Err(Error::Datafusion("Expected JSON array".into()));
        };

        if json_array.is_empty() {
            return Err(Error::Datafusion("Empty JSON array".into()));
        }

        let fields: Vec<FieldRef> = Vec::<FieldRef>::from_samples(
            json_array,
            TracingOptions::default()
                .allow_null_fields(true)
                .coerce_numbers(true),
        )
        .map_err(|e| Error::Datafusion(format!("from_samples: {e}")))?;

        let batch: RecordBatch = serde_arrow::to_record_batch(&fields, json_array)
            .map_err(|e| Error::Datafusion(format!("to_record_batch: {e}")))?;

        ctx.read_batch(batch)
            .map_err(|e| Error::Datafusion(format!("read_batch: {e}")))
    }

    async fn to_sql(&self, table_name: &str, sql: &str) -> Result<SqlDataFrame> {
        let ctx = get_shared_context().await;

        let Self::Array(json_array) = self else {
            return Err(Error::Datafusion("Expected JSON array".into()));
        };

        if json_array.is_empty() {
            return Err(Error::Datafusion("Empty JSON array".into()));
        }

        let fields: Vec<FieldRef> = Vec::<FieldRef>::from_samples(
            json_array,
            TracingOptions::default()
                .allow_null_fields(true)
                .coerce_numbers(true),
        )
        .map_err(|e| Error::Datafusion(format!("from_samples: {e}")))?;

        let batch: RecordBatch = serde_arrow::to_record_batch(&fields, json_array)
            .map_err(|e| Error::Datafusion(format!("to_record_batch: {e}")))?;

        // Cleanup any existing table with same name
        let _ = ctx.deregister_table(table_name);

        ctx.register_batch(table_name, batch)
            .map_err(|e| Error::Datafusion(format!("register_batch '{table_name}': {e}")))?;

        let df = ctx
            .sql(sql)
            .await
            .map_err(|e| Error::Datafusion(format!("sql planning: {e}")))?;

        Ok(SqlDataFrame {
            df,
            ctx,
            table_name: table_name.to_string(),
        })
    }
}

//============================= DF → JSON / Vec<T> ============================//

#[async_trait]
pub trait DataFrameExt {
    async fn to_vec<T>(&self) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned + Send;

    async fn to_json(&self) -> Result<serde_json::Value>;
}

#[async_trait]
impl DataFrameExt for DataFrame {
    async fn to_vec<T>(&self) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned + Send,
    {
        use datafusion::physical_plan::SendableRecordBatchStream;

        let mut stream: SendableRecordBatchStream = self
            .clone()
            .execute_stream()
            .await
            .map_err(|e| Error::Datafusion(format!("execute_stream: {e}")))?;

        let mut out = Vec::<T>::new();

        while let Some(item) = stream.next().await {
            let batch = item.map_err(|e| Error::Datafusion(format!("stream batch: {e}")))?;

            let vals: Vec<serde_json::Value> = serde_arrow::from_record_batch(&batch)
                .map_err(|e| Error::Datafusion(format!("from_record_batch: {e}")))?;

            let chunk: Vec<T> = serde_json::from_value(serde_json::Value::Array(vals))
                .map_err(|e| Error::Datafusion(format!("json→Vec<T>: {e}")))?;

            out.extend(chunk);
        }

        Ok(out)
    }

    async fn to_json(&self) -> Result<serde_json::Value> {
        use datafusion::physical_plan::SendableRecordBatchStream;

        let mut stream: SendableRecordBatchStream = self
            .clone()
            .execute_stream()
            .await
            .map_err(|e| Error::Datafusion(format!("execute_stream: {e}")))?;

        let mut rows = Vec::<serde_json::Value>::new();

        while let Some(item) = stream.next().await {
            let batch = item.map_err(|e| Error::Datafusion(format!("stream batch: {e}")))?;

            let mut vals: Vec<serde_json::Value> = serde_arrow::from_record_batch(&batch)
                .map_err(|e| Error::Datafusion(format!("from_record_batch: {e}")))?;

            rows.append(&mut vals);
        }

        Ok(serde_json::Value::Array(rows))
    }
}

//============================== Writer Trait =================================//

/// Result of a successful query execution
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub table_name: String,
    pub data: serde_json::Value,
    pub row_count: usize,
}

/// Query execution error
#[derive(Debug, Clone)]
pub struct QueryError {
    pub table_name: String,
    pub error: String,
}

/// Writer trait - implement for PostgreSQL, ClickHouse, Files, etc.
#[async_trait]
pub trait DataWriter: Send + Sync {
    /// Write query result to destination
    async fn write(&self, result: QueryResult) -> Result<()>;

    /// Handle query errors (optional override)
    async fn on_error(&self, error: QueryError) -> Result<()> {
        eprintln!("❌ Error in {}: {}", error.table_name, error.error);
        Ok(())
    }

    /// Called before processing starts (optional override)
    async fn begin(&self) -> Result<()> {
        Ok(())
    }

    /// Called after all queries complete successfully (optional override)
    async fn commit(&self) -> Result<()> {
        Ok(())
    }

    /// Called on failure (optional override)
    async fn rollback(&self) -> Result<()> {
        Ok(())
    }
}

//======================== Query with Per-Table Writer ========================//

/// A query with its data source, SQL, and destination writer
pub struct TableQuery {
    pub table_name: String,
    pub data: serde_json::Value,
    pub sql: String,
    pub writer: Arc<dyn DataWriter>,
}

impl TableQuery {
    pub fn new(
        table_name: impl Into<String>,
        data: serde_json::Value,
        sql: impl Into<String>,
        writer: Arc<dyn DataWriter>,
    ) -> Self {
        Self {
            table_name: table_name.into(),
            data,
            sql: sql.into(),
            writer,
        }
    }
}

//============================== Pipeline Config ==============================//

/// Configuration for batch query processing
pub struct BatchQueryConfig {
    /// Number of concurrent queries to execute
    pub concurrency: usize,

    /// Show progress logs
    pub show_progress: bool,

    /// Continue processing on errors or stop
    pub continue_on_error: bool,
}

impl Default for BatchQueryConfig {
    fn default() -> Self {
        Self {
            concurrency: 4,
            show_progress: true,
            continue_on_error: true,
        }
    }
}

//============================== Pipeline Stats ===============================//

/// Statistics from pipeline execution
#[derive(Debug, Clone)]
pub struct PipelineStats {
    pub success_count: usize,
    pub error_count: usize,
    pub total_rows: usize,
}

impl PipelineStats {
    fn new() -> Self {
        Self {
            success_count: 0,
            error_count: 0,
            total_rows: 0,
        }
    }

    fn success(&mut self, rows: usize) {
        self.success_count += 1;
        self.total_rows += rows;
    }

    fn error(&mut self) {
        self.error_count += 1;
    }

    fn completed(&self) -> usize {
        self.success_count + self.error_count
    }
}

//============================= Pipeline Execution ============================//

/// Data processing pipeline
pub struct DataPipeline {
    config: BatchQueryConfig,
}

impl DataPipeline {
    pub fn new(config: BatchQueryConfig) -> Self {
        Self { config }
    }

    /// Execute queries with per-table writers (streaming, minimal memory)
    pub async fn execute(&self, queries: Vec<TableQuery>) -> Result<PipelineStats> {
        let total = queries.len();
        let sem = Arc::new(Semaphore::new(self.config.concurrency));
        let stats = Arc::new(RwLock::new(PipelineStats::new()));

        if self.config.show_progress {
            println!(
                "🚀 Processing {} queries (concurrency: {})",
                total, self.config.concurrency
            );
        }

        // Get unique writers and call begin()
        let unique_writers = Self::get_unique_writers(&queries);
        for writer in &unique_writers {
            writer.begin().await?;
        }

        // Process queries in chunks to avoid spawning too many tasks
        let chunk_size = self.config.concurrency * 2;
        let mut query_iter = queries.into_iter();

        loop {
            let chunk: Vec<_> = query_iter.by_ref().take(chunk_size).collect();
            if chunk.is_empty() {
                break;
            }

            let mut handles = Vec::with_capacity(chunk.len());

            for q in chunk {
                let sem = sem.clone();
                let stats = stats.clone();
                let show = self.config.show_progress;
                let continue_on_error = self.config.continue_on_error;

                handles.push(tokio::spawn(async move {
                    let _permit = sem.acquire().await.unwrap();

                    match q.data.to_sql(&q.table_name, &q.sql).await {
                        Ok(sdf) => {
                            match sdf.inner().to_json().await {
                                Ok(json) => {
                                    let n = json.as_array().map(|a| a.len()).unwrap_or(0);

                                    let result = QueryResult {
                                        table_name: q.table_name.clone(),
                                        data: json,
                                        row_count: n,
                                    };

                                    // Write immediately to destination
                                    match q.writer.write(result).await {
                                        Ok(_) => {
                                            stats.write().await.success(n);
                                            if show {
                                                let s = stats.read().await;
                                                println!(
                                                    "✅ [{}/{}] {} - {} rows",
                                                    s.completed(),
                                                    total,
                                                    q.table_name,
                                                    n
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            stats.write().await.error();
                                            if show {
                                                eprintln!(
                                                    "❌ Write failed for {}: {}",
                                                    q.table_name, e
                                                );
                                            }
                                            if !continue_on_error {
                                                return Err(e);
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    stats.write().await.error();
                                    let _ = q
                                        .writer
                                        .on_error(QueryError {
                                            table_name: q.table_name.clone(),
                                            error: format!("to_json: {e}"),
                                        })
                                        .await;
                                    if show {
                                        eprintln!("❌ Query failed for {}: {}", q.table_name, e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            stats.write().await.error();
                            let _ = q
                                .writer
                                .on_error(QueryError {
                                    table_name: q.table_name.clone(),
                                    error: format!("query: {e}"),
                                })
                                .await;
                            if show {
                                eprintln!("❌ Query failed for {}: {}", q.table_name, e);
                            }
                        }
                    }

                    Ok::<_, Error>(())
                }));
            }

            // Wait for chunk to complete
            for h in handles {
                let _ = h.await;
            }

            // Small delay between chunks
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        // Commit all writers
        for writer in &unique_writers {
            writer.commit().await?;
        }

        let final_stats = stats.read().await.clone();

        if self.config.show_progress {
            println!("\n🎉 Pipeline completed:");
            println!("   ✅ Success: {}", final_stats.success_count);
            println!("   ❌ Errors: {}", final_stats.error_count);
            println!("   📊 Total rows: {}", final_stats.total_rows);
        }

        Ok(final_stats)
    }

    /// Get unique writers from queries (for begin/commit)
    fn get_unique_writers(queries: &[TableQuery]) -> Vec<Arc<dyn DataWriter>> {
        let mut unique = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for q in queries {
            let ptr = Arc::as_ptr(&q.writer) as *const () as usize;
            if seen.insert(ptr) {
                unique.push(q.writer.clone());
            }
        }

        unique
    }
}

//=============================== Query Builder ===============================//

/// Builder for constructing query pipelines
pub struct QueryBuilder {
    queries: Vec<TableQuery>,
}

impl QueryBuilder {
    pub fn new() -> Self {
        Self {
            queries: Vec::new(),
        }
    }

    /// Add a single query with its writer
    pub fn add_query(
        mut self,
        table_name: impl Into<String>,
        data: serde_json::Value,
        sql: impl Into<String>,
        writer: Arc<dyn DataWriter>,
    ) -> Self {
        self.queries
            .push(TableQuery::new(table_name, data, sql, writer));
        self
    }

    /// Add multiple queries with SQL template and shared writer
    pub fn add_queries_with_template(
        mut self,
        tables: Vec<(String, serde_json::Value)>,
        sql_template: impl Fn(&str) -> String,
        writer: Arc<dyn DataWriter>,
    ) -> Self {
        for (name, data) in tables {
            let sql = sql_template(&name);
            self.queries
                .push(TableQuery::new(name, data, sql, writer.clone()));
        }
        self
    }

    /// Add multiple queries with individual SQL and shared writer
    pub fn add_queries(
        mut self,
        queries: Vec<(String, serde_json::Value, String)>,
        writer: Arc<dyn DataWriter>,
    ) -> Self {
        for (name, data, sql) in queries {
            self.queries
                .push(TableQuery::new(name, data, sql, writer.clone()));
        }
        self
    }

    /// Execute the pipeline
    pub async fn execute(self, config: BatchQueryConfig) -> Result<PipelineStats> {
        DataPipeline::new(config).execute(self.queries).await
    }
}

impl Default for QueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct JsonFileWriter {
    output_dir: std::path::PathBuf,
}

#[async_trait]
impl DataWriter for JsonFileWriter {
    async fn write(&self, result: QueryResult) -> Result<()> {
        let path = self.output_dir.join(format!("{}.json", result.table_name));
        tokio::fs::write(path, serde_json::to_string_pretty(&result.data)?).await?;
        Ok(())
    }
}

impl JsonFileWriter {
    pub fn new(output_dir: impl Into<std::path::PathBuf>) -> Result<Self> {
        let output_dir = output_dir.into();
        std::fs::create_dir_all(&output_dir)
            .map_err(|e| Error::Datafusion(format!("Create dir: {e}")))?;
        Ok(Self { output_dir })
    }
}

use sqlx::PgPool;

pub struct PostgresWriter {
    pool: PgPool,
}

impl PostgresWriter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DataWriter for PostgresWriter {
    async fn write(&self, result: QueryResult) -> Result<()> {
        let rows = result
            .data
            .as_array()
            .ok_or_else(|| Error::Datafusion("Expected array".into()))?;

        for row in rows {
            // Your insert logic
            sqlx::query("INSERT INTO my_table (data) VALUES ($1)")
                .bind(row.to_string())
                .execute(&self.pool)
                .await
                .map_err(|e| Error::Datafusion(format!("PG write: {e}")))?;
        }

        Ok(())
    }

    async fn begin(&self) -> Result<()> {
        sqlx::query("BEGIN")
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Datafusion(format!("BEGIN: {e}")))?;
        Ok(())
    }

    async fn commit(&self) -> Result<()> {
        sqlx::query("COMMIT")
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Datafusion(format!("COMMIT: {e}")))?;
        Ok(())
    }
}
