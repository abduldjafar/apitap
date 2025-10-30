// src/main.rs
use std::{fs::File, path::Path, sync::Arc};

use apitap::{
    errors::{self, Result},
    http::{
        fetcher::{DataFusionPageWriter, PaginatedFetcher, Pagination},
        Http,
    },
    pipeline::{Config, SinkConn, TargetConn},
    writer::{postgres::PostgresWriter, WriteMode},
};

#[tokio::main]
async fn main() -> Result<()> {
    Ok(())
}
