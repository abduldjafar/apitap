// src/main.rs
use apitap::{
    errors::{self, Result},
    http::{
        fetcher::{DataFusionPageWriter, PaginatedFetcher}, Http
    },
    pipeline::Config,
    utils::datafusion_ext::{DataFrameExt, JsonValueExt},
    writer::{postgres::PostgresWriter, WriteMode},
};

use sqlx::PgPool;
use std::fs::File;
use std::{path::Path, sync::Arc};

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = load_config_from_path("pipelines.yaml")?;
    // optional: if you mutate names later, call cfg.reindex() afterward
    if let Some(src) = cfg.source("json_place_holder") {
        println!("URL = {}", src.url);
    }
    if let Some(tgt) = cfg.target("postgres_sink") {
        println!("found target {:?}", std::mem::discriminant(tgt));
    }

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/postgres".to_string());
    let pool = PgPool::connect(&database_url).await?;
    println!("✅ Connected to PostgreSQL");

    let http = Http::new("https://jsonplaceholder.typicode.com/posts");
    let client = http.build_client();
    let url = http.get_url();

    let pg_writer_config = PostgresWriter::new(pool.clone(), "jsonplaceholder_post")
        .with_primary_key_single("id")
        .with_batch_size(50)
        .with_sample_size(10)
        .auto_create(true)
        .auto_truncate(false);

    if pg_writer_config.auto_truncate {
        pg_writer_config.truncate().await?;
    }
    let pg_writer_all_columns = Arc::new(pg_writer_config);

    let page_writer_all = Arc::new(DataFusionPageWriter::new(
        "jsonplaceholder_post",
        "SELECT * FROM jsonplaceholder_post",
        pg_writer_all_columns,
    ));

    let fetcher = PaginatedFetcher::new(client.clone(), url.clone(), /*concurrency=*/ 5)
        .with_limit_offset("_limit", "_start")
        .with_batch_size(256); // optional tuning

    println!("\n🚀 Starting data fetch...\n");

    let _stats = fetcher
        .fetch_limit_offset(
            50,   // limit per page
            None, // where array of items lives
            None,
            page_writer_all, // sink
            WriteMode::Merge,
        )
        .await?;

    println!("✅ Done");
    Ok(())
}




pub fn load_config_from_path<P: AsRef<Path>>(path: P) -> Result<Config> {
    let f = File::open(path).map_err(|e| errors::Error::SerdeYaml(e.to_string()))?;
    // This calls your custom `Deserialize` for Config (which builds the indexes)
    let cfg: Config = serde_yaml::from_reader(f)?;
    Ok(cfg)
}
