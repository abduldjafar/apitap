// src/utils/postgres_writer.rs

use crate::errors::{Error, Result};
use crate::utils::datafusion_ext::{DataWriter, QueryResult, QueryResultStream};
use async_trait::async_trait;
use serde_json::Value;
use sqlx::{PgPool, types::Json};
use tokio_stream::StreamExt;
use std::collections::BTreeMap;

//=============== Type Definitions ============================================//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PgType {
    Text,
    Boolean,
    BigInt,
    Double,
    Jsonb,
}

impl PgType {
    fn as_sql(&self) -> &'static str {
        match self {
            PgType::Text => "TEXT",
            PgType::Boolean => "BOOLEAN",
            PgType::BigInt => "BIGINT",
            PgType::Double => "DOUBLE PRECISION",
            PgType::Jsonb => "JSONB",
        }
    }

    fn from_json_value(value: &Value) -> Self {
        match value {
            Value::Null => PgType::Text,
            Value::Bool(_) => PgType::Boolean,
            Value::Number(n) => {
                if n.is_i64() {
                    PgType::BigInt
                } else {
                    PgType::Double
                }
            }
            Value::String(_) => PgType::Text,
            Value::Array(_) => PgType::Jsonb,
            Value::Object(_) => PgType::Jsonb,
        }
    }

    fn merge(&self, other: &Self) -> Self {
        match (self, other) {
            (PgType::Text, _) | (_, PgType::Text) => PgType::Text,
            (PgType::BigInt, PgType::Double) | (PgType::Double, PgType::BigInt) => PgType::Double,
            (PgType::BigInt, PgType::BigInt) => PgType::BigInt,
            (PgType::Double, PgType::Double) => PgType::Double,
            (a, b) if a == b => *a,
            _ => PgType::Text,
        }
    }
}

//=============== PostgreSQL Auto-Columns Writer ==============================//

pub struct PostgresAutoColumnsWriter {
    pool: PgPool,
    table_name: String,
    batch_size: usize,
    sample_size: usize,
    auto_create: bool,
    pub auto_truncate:bool,
    table_created: tokio::sync::RwLock<bool>,
    columns_cache: tokio::sync::RwLock<Option<BTreeMap<String, PgType>>>,
}

impl PostgresAutoColumnsWriter {
    pub fn new(pool: PgPool, table_name: impl Into<String>) -> Self {
        Self {
            pool,
            table_name: table_name.into(),
            batch_size: 100,
            sample_size: 10,
            auto_create: true,
            auto_truncate:false,
            table_created: tokio::sync::RwLock::new(false),
            columns_cache: tokio::sync::RwLock::new(None),
        }
    }

    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    pub fn with_sample_size(mut self, size: usize) -> Self {
        self.sample_size = size;
        self
    }

    pub fn auto_create(mut self, enabled: bool) -> Self {
        self.auto_create = enabled;
        self
    }

    pub fn auto_truncate(mut self, enabled: bool) -> Self {
        self.auto_truncate = enabled;
        self
    }

    async fn table_exists(&self) -> Result<bool> {
        let result: (bool,) = sqlx::query_as(
            "SELECT EXISTS (
                SELECT FROM information_schema.tables 
                WHERE table_schema = 'public' 
                AND table_name = $1
            )",
        )
        .bind(&self.table_name)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Datafusion(format!("Check table exists: {}", e)))?;

        Ok(result.0)
    }

    fn analyze_schema(rows: &[Value], sample_size: usize) -> Result<BTreeMap<String, PgType>> {
        let mut column_types: BTreeMap<String, Vec<PgType>> = BTreeMap::new();

        let sample = &rows[..rows.len().min(sample_size)];

        for row in sample {
            let obj = row
                .as_object()
                .ok_or_else(|| Error::Datafusion("Expected JSON object".to_string()))?;

            for (key, value) in obj {
                let pg_type = PgType::from_json_value(value);
                column_types
                    .entry(key.clone())
                    .or_insert_with(Vec::new)
                    .push(pg_type);
            }
        }

        let mut final_types = BTreeMap::new();
        for (col_name, types) in column_types {
            let final_type = types.iter().fold(types[0], |acc, t| acc.merge(t));
            final_types.insert(col_name, final_type);
        }

        Ok(final_types)
    }

    async fn create_table_from_schema(&self, schema: &BTreeMap<String, PgType>) -> Result<()> {
        if schema.is_empty() {
            return Err(Error::Datafusion("No columns detected".to_string()));
        }

        let column_defs: Vec<String> = schema
            .iter()
            .map(|(name, pg_type)| format!("{} {}", name, pg_type.as_sql()))
            .collect();

        let query = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                {}
            )",
            self.table_name,
            column_defs.join(",\n                ")
        );

        sqlx::query(&query)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Datafusion(format!("Create table: {}", e)))?;

        let column_names: Vec<String> = schema.keys().cloned().collect();

        println!(
            "✅ Created table: {} with {} columns: {}",
            self.table_name,
            column_names.len(),
            column_names.join(", ")
        );

        println!("   📋 Column types:");
        for (name, pg_type) in schema {
            println!("      - {}: {}", name, pg_type.as_sql());
        }

        Ok(())
    }

    async fn ensure_table(&self, sample_rows: &[Value]) -> Result<BTreeMap<String, PgType>> {
        if let Some(schema) = self.columns_cache.read().await.as_ref() {
            return Ok(schema.clone());
        }

        let schema = if !self.table_exists().await? {
            if self.auto_create {
                if sample_rows.is_empty() {
                    return Err(Error::Datafusion(
                        "Need sample data to create table".to_string(),
                    ));
                }
                let detected_schema = Self::analyze_schema(sample_rows, self.sample_size)?;
                self.create_table_from_schema(&detected_schema).await?;
                detected_schema
            } else {
                return Err(Error::Datafusion(format!(
                    "Table '{}' does not exist",
                    self.table_name
                )));
            }
        } else {
            if sample_rows.is_empty() {
                return Err(Error::Datafusion("Need sample data".to_string()));
            }
            Self::analyze_schema(sample_rows, self.sample_size)?
        };

        *self.columns_cache.write().await = Some(schema.clone());

        Ok(schema)
    }

    pub async fn truncate(&self) -> Result<()>{
        println!("Truncating {}...",self.table_name);
         let query = format!(
            "TRUNCATE {}",
            self.table_name
        );
         sqlx::query(&query)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Sqlx(format!("TRUNCATE: {}", e)))?;
        Ok(())
    }
    async fn insert_batch(&self, rows: &[Value], schema: &BTreeMap<String, PgType>) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }

        let columns: Vec<&String> = schema.keys().collect();
        let columns_str = columns
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let values_per_row = columns.len();

        let mut placeholders = Vec::new();
        for row_idx in 0..rows.len() {
            let row_placeholders: Vec<String> = (1..=values_per_row)
                .map(|col_idx| format!("${}", row_idx * values_per_row + col_idx))
                .collect();
            placeholders.push(format!("({})", row_placeholders.join(", ")));
        }

        let query = format!(
            "INSERT INTO {} ({}) VALUES {}",
            self.table_name,
            columns_str,
            placeholders.join(", ")
        );

        // ✅ COLLECT ALL VALUES FIRST
        let mut all_values = Vec::new();
        for row in rows {
            for col_name in columns.iter() {
                let value = row.get(*col_name).cloned().unwrap_or(Value::Null);
                all_values.push(value);
            }
        }

        // ✅ NOW BIND THEM
        let mut query_builder = sqlx::query(&query);
        for (idx, value) in all_values.iter().enumerate() {
            let col_idx = idx % values_per_row;
            let col_name = columns[col_idx];
            let expected_type = schema.get(col_name).unwrap();

            query_builder = self.bind_value(query_builder, value, expected_type)?;
        }

        query_builder
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Datafusion(format!("PostgreSQL insert: {}", e)))?;

        Ok(())
    }

    /// Bind value with proper type conversion
    fn bind_value<'q>(
        &self,
        query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
        value: &'q Value,
        expected_type: &PgType,
    ) -> Result<sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>> {
        let result = match (value, expected_type) {
            // Null
            (Value::Null, PgType::BigInt) => query.bind::<Option<i64>>(None),
            (Value::Null, PgType::Double) => query.bind::<Option<f64>>(None),
            (Value::Null, PgType::Boolean) => query.bind::<Option<bool>>(None),
            (Value::Null, PgType::Jsonb) => query.bind(Json(Value::Null)),
            (Value::Null, _) => query.bind::<Option<String>>(None),

            // Boolean
            (Value::Bool(b), PgType::Boolean) => query.bind(*b),
            (Value::Bool(b), PgType::Text) => query.bind(b.to_string()),
            (Value::Bool(b), _) => query.bind(b.to_string()),

            // Numbers
            (Value::Number(n), PgType::BigInt) => {
                if let Some(i) = n.as_i64() {
                    query.bind(i)
                } else {
                    query.bind::<Option<i64>>(None)
                }
            }
            (Value::Number(n), PgType::Double) => {
                if let Some(f) = n.as_f64() {
                    query.bind(f)
                } else {
                    query.bind::<Option<f64>>(None)
                }
            }
            (Value::Number(n), PgType::Text) => query.bind(n.to_string()),
            (Value::Number(_), PgType::Jsonb) => query.bind(Json(value)),
            (Value::Number(_), _) => query.bind::<Option<f64>>(None),

            // Strings
            (Value::String(s), PgType::Text) => query.bind(s.as_str()),
            (Value::String(s), PgType::Jsonb) => {
                let json_str = Value::String(s.clone());
                query.bind(Json(json_str))
            }
            (Value::String(s), PgType::BigInt) => {
                if let Ok(i) = s.parse::<i64>() {
                    query.bind(i)
                } else {
                    query.bind::<Option<i64>>(None)
                }
            }
            (Value::String(s), PgType::Double) => {
                if let Ok(f) = s.parse::<f64>() {
                    query.bind(f)
                } else {
                    query.bind::<Option<f64>>(None)
                }
            }
            (Value::String(s), PgType::Boolean) => {
                let b = s.to_lowercase() == "true" || s == "1";
                query.bind(b)
            }

            // Arrays / Objects
            (Value::Array(_), PgType::Jsonb) | (Value::Object(_), PgType::Jsonb) => {
                query.bind(Json(value))
            }
            (Value::Array(_), PgType::Text) | (Value::Object(_), PgType::Text) => {
                query.bind(serde_json::to_string(value).unwrap_or_default())
            }
            (Value::Array(_), _) | (Value::Object(_), _) => {
                query.bind(serde_json::to_string(value).unwrap_or_default())
            }
        };

        Ok(result)
    }
}

#[async_trait]
impl DataWriter for PostgresAutoColumnsWriter {
    async fn write_stream(&self, mut result: QueryResultStream) -> Result<()> {

        let mut buf: Vec<serde_json::Value> = Vec::with_capacity(self.batch_size);
        let mut schema: Option<BTreeMap<String, PgType>> = None; // whatever type your ensure_table returns


        while let Some(item) = result.data.next().await {
            let v = item?; // serde_json::Value

            buf.push(v);

            if buf.len() >= self.batch_size {
                // Initialize schema/table once, using the first batch
                if schema.is_none() {
                    schema = Some(self.ensure_table(&buf).await?);
                }

                // Insert this batch
                self.insert_batch(&buf, schema.as_ref().unwrap()).await?;
                buf.clear();
            }
        }

        // Flush remaining
        if !buf.is_empty() {
            if schema.is_none() {
                schema = Some(self.ensure_table(&buf).await?);
            }
            self.insert_batch(&buf, schema.as_ref().unwrap()).await?;
        }

        Ok(())
    }

    async fn write(&self, result: QueryResult) -> Result<()> {
        let rows = result
            .data
            .as_array()
            .ok_or_else(|| Error::Datafusion("Expected JSON array".to_string()))?;

        if rows.is_empty() {
            return Ok(());
        }

        let schema = self.ensure_table(rows).await?;

        for chunk in rows.chunks(self.batch_size) {
            self.insert_batch(chunk, &schema).await?;
        }

        Ok(())
    }

    async fn begin(&self) -> Result<()> {
        sqlx::query("BEGIN")
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Datafusion(format!("BEGIN: {}", e)))?;
        Ok(())
    }

    async fn commit(&self) -> Result<()> {
        sqlx::query("COMMIT")
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Datafusion(format!("COMMIT: {}", e)))?;
        Ok(())
    }

    async fn rollback(&self) -> Result<()> {
        sqlx::query("ROLLBACK")
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Datafusion(format!("ROLLBACK: {}", e)))?;
        Ok(())
    }
}
