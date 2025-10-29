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
    Ok(())
}