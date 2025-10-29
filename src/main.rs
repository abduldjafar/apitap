// src/main.rs
use apitap::{
    errors::Result,
    http::{
        Http,
        fetcher::{DataFusionPageWriter, PaginatedFetcher},
    },
    pipeline::Config,
    utils::datafusion_ext::{DataFrameExt, JsonValueExt},
    writer::postgres::PostgresWriter,
};

use sqlx::PgPool;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    Ok(())
}
