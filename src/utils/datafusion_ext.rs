// src/utils/datafusion_ext.rs

use async_trait::async_trait;
use futures::{Stream, StreamExt, stream};
use serde_arrow::schema::{SchemaLike, TracingOptions};
use std::{pin::Pin, sync::Arc};
use tokio::sync::OnceCell;

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

use crate::{
    errors::{Error, Result},
    writer::{DataWriter, WriteMode},
};

// =========================== Shared SessionContext ========================== //

static SHARED_CTX: OnceCell<Arc<SessionContext>> = OnceCell::const_new();

/// Stream of JSON rows (`Result<Value>`) boxed + pinned for dynamic dispatch.
pub type JsonStreamType = Pin<Box<dyn Stream<Item = Result<serde_json::Value>> + Send + 'static>>;

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

// ========================= RAII for temp table cleanup ====================== //

pub struct SqlDataFrame {
    df: DataFrame,
    ctx: Arc<SessionContext>,
    table_name: String,
}

impl SqlDataFrame {
    #[inline]
    pub fn inner(&self) -> &DataFrame {
        &self.df
    }
}

impl Drop for SqlDataFrame {
    fn drop(&mut self) {
        let _ = self.ctx.deregister_table(&self.table_name);
    }
}

// ============================= JSON → DF / SQL ============================== //

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

        // Best-effort cleanup of any existing table with the same name.
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

// ============================= DF → JSON / Vec<T> =========================== //

#[async_trait]
pub trait DataFrameExt {
    async fn to_vec<T>(&self) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned + Send;

    async fn to_json(&self) -> Result<serde_json::Value>;

    async fn to_stream(&self) -> Result<JsonStreamType> {
        Ok(Box::pin(stream::empty::<Result<serde_json::Value>>()))
    }
}

#[async_trait]
impl DataFrameExt for DataFrame {
    async fn to_vec<T>(&self) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned + Send,
    {
        use datafusion::physical_plan::SendableRecordBatchStream;

        let mut rb_stream: SendableRecordBatchStream = self
            .clone()
            .execute_stream()
            .await
            .map_err(|e| Error::Datafusion(format!("execute_stream: {e}")))?;

        let mut out = Vec::<T>::new();
        while let Some(item) = rb_stream.next().await {
            let batch = item.map_err(|e| Error::Datafusion(format!("stream batch: {e}")))?;
            let vals: Vec<serde_json::Value> = serde_arrow::from_record_batch(&batch)
                .map_err(|e| Error::Datafusion(format!("from_record_batch: {e}")))?;
            let chunk: Vec<T> = serde_json::from_value(serde_json::Value::Array(vals))
                .map_err(|e| Error::Datafusion(format!("json→Vec<T>: {e}")))?;
            out.extend(chunk);
        }
        Ok(out)
    }

    async fn to_stream(&self) -> Result<JsonStreamType> {
        use datafusion::physical_plan::SendableRecordBatchStream;

        let mut rb_stream: SendableRecordBatchStream = self
            .clone()
            .execute_stream()
            .await
            .map_err(|e| Error::Datafusion(format!("execute_stream: {e}")))?;

        let s = async_stream::try_stream! {
            while let Some(item) = rb_stream.next().await {
                let batch = item.map_err(|e| Error::Datafusion(format!("stream batch: {e}")))?;

                let rows: Vec<serde_json::Value> =
                    serde_arrow::from_record_batch(&batch)
                        .map_err(|e| Error::Datafusion(format!("from_record_batch: {e}")))?;

                for v in rows {
                    yield v;
                }
            }
        };

        Ok(Box::pin(s))
    }

    async fn to_json(&self) -> Result<serde_json::Value> {
        use datafusion::physical_plan::SendableRecordBatchStream;

        let mut rb_stream: SendableRecordBatchStream = self
            .clone()
            .execute_stream()
            .await
            .map_err(|e| Error::Datafusion(format!("execute_stream: {e}")))?;

        let mut rows = Vec::<serde_json::Value>::new();
        while let Some(item) = rb_stream.next().await {
            let batch = item.map_err(|e| Error::Datafusion(format!("stream batch: {e}")))?;
            let mut vals: Vec<serde_json::Value> = serde_arrow::from_record_batch(&batch)
                .map_err(|e| Error::Datafusion(format!("from_record_batch: {e}")))?;
            rows.append(&mut vals);
        }

        Ok(serde_json::Value::Array(rows))
    }
}

// ============================== Writer Types ================================ //

/// Result of a successful query execution (in-memory)
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub table_name: String,
    pub data: serde_json::Value,
    pub row_count: usize,
}

/// Result of a successful query execution (streaming)
pub struct QueryResultStream {
    pub table_name: String,
    pub data: JsonStreamType,
}

/// Query execution error
#[derive(Debug, Clone)]
pub struct QueryError {
    pub table_name: String,
    pub error: String,
}
