// src/main.rs
use apitap::{
    errors::Result,
    http::{
        Http,
        fetcher::{DataFusionPageWriter, PaginatedFetcher},
    },
    utils::datafusion_ext::{DataFrameExt, JsonValueExt},
    writer::postgres::PostgresWriter,
};

use sqlx::PgPool;
use std::sync::Arc;

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
    let pg_writer_config = PostgresWriter::new(pool.clone(), "jsonplaceholder_post")
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

    let fetcher =
        PaginatedFetcher::new(client.clone(), url.clone(), "page", /*concurrency=*/ 5)
            .with_limit_offset("_limit", "_start")
            .with_batch_size(256); // optional tuning

    println!("\n🚀 Starting data fetch...\n");

    // Giphy pointers:
    //   data_path          = /data
    //   total items pointer= /pagination/total_count
    // We’ll request limit=50 → offsets: 0,50,100,...
    let _stats = fetcher
        .fetch_limit_offset(
            50,   // limit per page
            None, // where array of items lives
            None,
            page_writer_all, // sink
        )
        .await?;

    println!("✅ Done");
    Ok(())
}
