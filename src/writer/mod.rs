use async_trait::async_trait;

use crate::{
    errors::Result,
    utils::datafusion_ext::{QueryError, QueryResult, QueryResultStream},
};

pub mod postgres;

#[derive(Debug, Clone, PartialEq)]
pub enum WriteMode {
    Merge,
    Append,
}

#[async_trait]
pub trait DataWriter: Send + Sync {
    /// Write query result to destination (in-memory).
    async fn write(&self, result: QueryResult) -> Result<()>;

    /// Write query result to destination (streaming).
    async fn write_stream(&self, _result: QueryResultStream, _write_mode: WriteMode) -> Result<()> {
        Ok(())
    }

    async fn merge(&self, _result: QueryResultStream) -> Result<()> {
        Ok(())
    }

    /// Handle query errors.
    async fn on_error(&self, error: QueryError) -> Result<()> {
         tracing::error!("❌ Error in {}: {}", error.table_name, error.error);
        Ok(())
    }

    /// Lifecycle hooks.
    async fn begin(&self) -> Result<()> {
        Ok(())
    }
    async fn commit(&self) -> Result<()> {
        Ok(())
    }
    async fn rollback(&self) -> Result<()> {
        Ok(())
    }
}
