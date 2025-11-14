use datafusion::{
    arrow::datatypes::SchemaRef,
    catalog::Session,
    datasource::{TableProvider, TableType},
    logical_expr::{Expr, TableProviderFilterPushDown},
    physical_plan::ExecutionPlan,
};
use std::{any::Any, sync::Arc};

use crate::utils::execution::{Exec, JsonStreamFactory};

/// Table provider for streaming JSON data
pub struct JsonStreamTableProvider {
    stream_factory: JsonStreamFactory,
    schema: SchemaRef,
}

impl JsonStreamTableProvider {
    pub fn new(stream_factory: JsonStreamFactory, schema: SchemaRef) -> Self
    {
        Self {
            stream_factory,
            schema,
        }
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
        self.schema.clone()
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
        // Log what filters/limits are being pushed down at debug level
        if !filters.is_empty() {
            tracing::debug!(filters = ?filters, "filters pushed down");
        }
        if let Some(l) = limit {
            tracing::debug!(limit = l, "limit pushed down");
        }

        let exec = Exec::new(self.schema.clone(), projection, {
            let factory = self.stream_factory.clone();
            move || factory()
        })?;

        Ok(Arc::new(exec))
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
