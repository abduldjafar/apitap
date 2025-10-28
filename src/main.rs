// src/main.rs
use apitap::{
    errors::Result,
    utils::{
        datafusion_ext::{DataFrameExt, JsonValueExt},
        http_fetcher::{DataFusionPageWriter, PaginatedFetcher, TotalHint}, // ← bring TotalHint
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
        Self { url: url.into(), params: None, headers: None, bearer_auth: None }
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
        if let Some(token) = &self.bearer_auth {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
            );
        }

        Client::builder().default_headers(headers).build().unwrap_or_else(|_| Client::new())
    }
    pub fn get_url(&self) -> String {
        if let Some(params) = &self.params {
            // keep any base params (we'll override limit/offset at call time)
            let query: Vec<String> = params
                .iter()
                .filter(|(k, _)| k.as_str() != "page") // ignore any page param
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();

            if query.is_empty() { self.url.clone() } else { format!("{}?{}", self.url, query.join("&")) }
        } else {
            self.url.clone()
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1) PG connection
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/postgres".to_string());
    let pool = PgPool::connect(&database_url).await?;
    println!("✅ Connected to PostgreSQL");

    // 2) HTTP base (Giphy Search)
    let http = Http::new("https://jsonplaceholder.typicode.com/posts");
        // no need to pre-set limit/offset here; the fetcher will inject them

    // 3) PG writer (auto-columns)
    let pg_writer_config = PostgresAutoColumnsWriter::new(pool.clone(), "jsonplaceholder_post")
        .with_batch_size(50)
        .with_sample_size(10)
        .auto_create(true)
        .auto_truncate(true);

    if pg_writer_config.auto_truncate {
        pg_writer_config.truncate().await?;
    }
    let pg_writer_all_columns = Arc::new(pg_writer_config);

    // 4) Page writer (DataFusion → Postgres)
    let page_writer_all = Arc::new(DataFusionPageWriter::new(
        "jsonplaceholder_post",
        "SELECT * FROM jsonplaceholder_post",
        pg_writer_all_columns,
    ));

    // 5) Build fetcher (limit/offset pagination)
    let client = http.build_client();
    let url = http.get_url();

    let fetcher = PaginatedFetcher::new(client.clone(), url.clone(), "page", /*concurrency=*/5)
        .with_limit_offset("_limit", "_start")
        .with_batch_size(256); // optional tuning

    println!("\n🚀 Starting data fetch...\n");

    // Giphy pointers:
    //   data_path          = /data
    //   total items pointer= /pagination/total_count
    // We’ll request limit=50 → offsets: 0,50,100,...
    let _stats = fetcher
        .fetch_limit_offset(
            50,                               // limit per page
            None,                    // where array of items lives
            None,
            page_writer_all,                  // sink
        )
        .await?;

    println!("✅ Done");
    Ok(())
}
