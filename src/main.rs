// src/main.rs

use apitap::{
    errors::Result,
    utils::{
        datafusion_ext::{DataFrameExt, JsonValueExt},
        http_fetcher::{DataFusionPageWriter, PaginatedFetcher},
        postgres_writer::PostgresAutoColumnsWriter,
    },
};
use datafusion::common::HashMap;
use reqwest::Client;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct Http {
    url: String,
    params: Option<HashMap<String, String>>,
    headers: Option<HashMap<String, String>>,
    bearer_auth: Option<String>,
}

impl Http {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            params: None,
            headers: None,
            bearer_auth: None,
        }
    }

    pub fn param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let map = self.params.get_or_insert_with(HashMap::new);
        map.insert(key.into(), value.into());
        self
    }

    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let map = self.headers.get_or_insert_with(HashMap::new);
        map.insert(key.into(), value.into());
        self
    }

    pub fn bearer_auth(mut self, token: impl Into<String>) -> Self {
        self.bearer_auth = Some(token.into());
        self
    }

    pub fn build_client(&self) -> Client {
        let mut headers = reqwest::header::HeaderMap::new();

        if let Some(header_map) = &self.headers {
            for (key, value) in header_map {
                if let (Ok(name), Ok(val)) = (
                    reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                    reqwest::header::HeaderValue::from_str(value),
                ) {
                    headers.insert(name, val);
                }
            }
        }

        Client::builder()
            .default_headers(headers)
            .build()
            .unwrap_or_else(|_| Client::new())
    }

    pub fn get_url(&self) -> String {
        if let Some(params) = &self.params {
            let query: Vec<String> = params
                .iter()
                .filter(|(k, _)| k.as_str() != "page")
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();

            if query.is_empty() {
                self.url.clone()
            } else {
                format!("{}?{}", self.url, query.join("&"))
            }
        } else {
            self.url.clone()
        }
    }
}

//============================== Main =========================================//

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Connect to PostgreSQL
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/postgres".to_string());

    let pool = PgPool::connect(&database_url).await?;
    println!("✅ Connected to PostgreSQL");

    // 2. Setup HTTP request
    let http = Http::new("https://peopleforce.io/api/public/v2/employees")
        .param("status", "active")
        .param("per_page", "50")
        .header(
            "X-API-KEY",
            "",
        )
        .header("Content-Type", "application/json");

    // 3. Setup PostgreSQL writers

    // ✅ Auto-detect ALL columns from data (recommended!)

    let pg_writer_config = PostgresAutoColumnsWriter::new(pool.clone(), "employees_3")
            .with_batch_size(50)
            .with_sample_size(10)
            .auto_create(true)
            .auto_truncate(true);
    
    if pg_writer_config.auto_truncate{
        pg_writer_config.truncate().await?;

    }
    
    
    let pg_writer_all_columns = Arc::new(
       pg_writer_config
    );

    // 4. Setup page writers

    // All columns as structured fields
    let page_writer_all = Arc::new(DataFusionPageWriter::new(
        "employees_3",
        "SELECT * FROM employees_3", // ✅ All columns auto-detected!
        pg_writer_all_columns,
    ));

    // 5. Fetch and stream to PostgreSQL
    println!("\n🚀 Starting data fetch...\n");

    let client = http.build_client();
    let url = http.get_url();

    // Option 1: All columns as structured fields (RECOMMENDED)
    println!("📥 Fetching all employee data (auto-detect columns)...");
    let fetcher = PaginatedFetcher::new(client.clone(), url.clone(), "page", 5);
    let stats = fetcher.fetch_to_writer(page_writer_all).await?;
    println!("✅ Completed: {} items\n", stats.total_items);

    Ok(())
}
