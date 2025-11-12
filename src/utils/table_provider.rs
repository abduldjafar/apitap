use datafusion::{
    arrow::datatypes::{Schema, SchemaRef},
    catalog::Session,
    datasource::{TableProvider, TableType},
    logical_expr::{Expr, TableProviderFilterPushDown},
    physical_plan::ExecutionPlan,
};
use futures::Stream;
use serde_json::Value;
use std::{any::Any, pin::Pin, sync::Arc};

use crate::errors;
use crate::utils::execution::Exec;

/// Table provider for streaming JSON data
pub type JsonStreamFactory =
    Arc<dyn Fn() -> Pin<Box<dyn Stream<Item = errors::Result<Value>> + Send>> + Send + Sync>;
pub struct JsonStreamTableProvider {
    stream_factory: JsonStreamFactory,
    schema: Arc<tokio::sync::Mutex<Option<SchemaRef>>>,
}

impl JsonStreamTableProvider {
    pub fn new<F>(stream_factory: F) -> Self
    where
        F: Fn() -> Pin<Box<dyn Stream<Item = errors::Result<Value>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        Self {
            stream_factory: Arc::new(stream_factory),
            schema: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Get or infer the schema
    async fn _get_schema(&self) -> Result<SchemaRef, Box<dyn std::error::Error>> {
        // Check if schema is already cached
        if let Some(cached) = self.schema.lock().await.as_ref() {
            return Ok(cached.clone());
        }

        // Infer schema from stream
        let stream = (self.stream_factory)();
        let inferred = crate::utils::schema::infer_schema_streaming(stream).await?;

        // Cache it
        *self.schema.lock().await = Some(inferred.clone());

        Ok(inferred)
    }
}

impl std::fmt::Debug for JsonStreamTableProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsonStreamTableProvider")
            .field("schema", &self.schema)
            .finish()
    }
}

#[async_trait::async_trait]
impl TableProvider for JsonStreamTableProvider {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> SchemaRef {
        Arc::new(Schema::empty())
    }

    fn table_type(&self) -> TableType {
        TableType::View
    }

    async fn scan(
        &self,
        _state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> datafusion::error::Result<Arc<dyn ExecutionPlan>> {
        // Log what filters/limits are being pushed down
        if !filters.is_empty() {
            println!("Filters: {:?}", filters);
        }
        if let Some(l) = limit {
            println!("Limit: {}", l);
        }

        let exec = Exec::new(projection, {
            let factory = self.stream_factory.clone();
            move || factory()
        });

        Ok(Arc::new(exec.await))
    }

    fn supports_filters_pushdown(
        &self,
        _filters: &[&Expr],
    ) -> datafusion::error::Result<Vec<TableProviderFilterPushDown>> {
        // Return empty to indicate no filters are pushed down
        // Or return SupportedFilters to indicate which filters you support
        Ok(vec![])
    }
}
