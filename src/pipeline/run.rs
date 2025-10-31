// src/pipeline/run.rs
use reqwest::Client;
use std::sync::Arc;
use url::Url;

use crate::{
    errors::{Error, Result},
    http::fetcher::{DataFusionPageWriter, PaginatedFetcher, Pagination},
    writer::{DataWriter, WriteMode},
};

#[derive(Debug, Clone)]
pub struct FetchOpts {
    pub concurrency: usize,
    pub default_page_size: usize,
    pub fetch_batch_size: usize, // internal http batch size
}

pub async fn run_fetch(
    client: Client,
    url: Url,
    pagination: &Option<Pagination>,
    sql: &str,
    dest_table: &str,
    writer: Arc<dyn DataWriter>,
    write_mode: WriteMode,
    opts: &FetchOpts,
) -> Result<()> {
    let page_writer = Arc::new(DataFusionPageWriter::new(dest_table, sql, writer));

    match pagination {
        Some(Pagination::LimitOffset {
            limit_param,
            offset_param,
        }) => {
            let fetcher = PaginatedFetcher::new(client, url, opts.concurrency)
                .with_limit_offset(limit_param, offset_param)
                .with_batch_size(opts.fetch_batch_size);

            fetcher
                .fetch_limit_offset(
                    opts.default_page_size.try_into().unwrap(),
                    None,
                    None,
                    page_writer,
                    write_mode,
                )
                .await?;
        }

        Some(Pagination::PageNumber {
            page_param,
            per_page_param,
        }) => {
            let fetcher = PaginatedFetcher::new(client, url, opts.concurrency)
                .with_batch_size(opts.fetch_batch_size);
        }

        Some(Pagination::PageOnly { page_param }) => {
            let fetcher = PaginatedFetcher::new(client, url, opts.concurrency)
                .with_batch_size(opts.fetch_batch_size);
        }

        Some(Pagination::Cursor {
            cursor_param,
            page_size_param,
        }) => {
            let fetcher = PaginatedFetcher::new(client, url, opts.concurrency)
                .with_batch_size(opts.fetch_batch_size);
        }

        Some(Pagination::Default) | None => {
            return Err(Error::Reqwest("no supported pagination configured".into()));
        }
    }

    Ok(())
}
