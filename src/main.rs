use std::sync::{Arc, Mutex};

use apitap::config::load_config_from_path;
use apitap::errors::{self, Result};
use apitap::http::Http;
use apitap::config::templating::{RenderCapture, build_env_with_captures, list_sql_templates, render_one};
use apitap::pipeline::SinkConn;
use apitap::pipeline::run::{FetchOpts, run_fetch};
use apitap::pipeline::sink::{WriterOpts,MakeWriter};
use apitap::writer::WriteMode;
use reqwest::Url;


const CONCURRENCY: usize = 5;
const DEFAULT_PAGE_SIZE: usize = 50;
const FETCH_BATCH_SIZE: usize = 256;

#[tokio::main]
async fn main() -> Result<()> {
    let root = "pipelines";
    let cfg = load_config_from_path("pipelines.yaml")?;

    let names = list_sql_templates(root)?;
    let capture = Arc::new(Mutex::new(RenderCapture::default()));
    let env = build_env_with_captures(root, &capture);

    let fetch_opts = FetchOpts {
        concurrency: CONCURRENCY,
        default_page_size: DEFAULT_PAGE_SIZE,
        fetch_batch_size: FETCH_BATCH_SIZE,
    };

    for name in names {
        let rendered = render_one(&env, &capture, &name)?;

        let source_name = &rendered.capture.source;
        let sink_name   = &rendered.capture.sink;

        println!("\n=== {name} ===");
        println!("source : {source_name}");
        println!("sink   : {sink_name}");

        let src = cfg.source(source_name).ok_or_else(|| {
            errors::Error::Reqwest(format!("source not found in config: {source_name}"))
        })?;
        let tgt = cfg.target(sink_name).ok_or_else(|| {
            errors::Error::Reqwest(format!("target not found in config: {sink_name}"))
        })?;

        // Build HTTP client from source
        let http   = Http::new(src.url.clone());
        let client = http.build_client();
        let url = Url::parse(&http.get_url())?;          // returns Result<Url, url::ParseError>


        // Resolve destination table and substitute in SQL
        let dest_table = src.table_destination_name.as_deref().ok_or_else(|| {
            errors::Error::Reqwest(format!(
                "table_destination_name is required for source: {source_name}"
            ))
        })?;
        let sql = rendered.sql.replace(source_name, dest_table);

        // 🔧 Single factory call: TargetConn -> Arc<dyn DataWriter>
        let writer_opts = WriterOpts {
            dest_table,
            primary_key: "id",
            batch_size: 50,
            sample_size: 10,
            auto_create: true,
            auto_truncate: false,
            truncate_first: false,
            write_mode: WriteMode::Merge,
        };

        // Create a concrete connection first
        let conn = tgt.create_conn().await?;

        let (writer, maybe_truncate) = conn.make_writer(&writer_opts)?;

        // Optional pre-hook (e.g., truncate)
        if let Some(hook) = maybe_truncate {
            hook().await?;
        }

        // One generic runner drives pagination → page writer → sink
        run_fetch(
            client,
            url,
            &src.pagination,
            &sql,
            dest_table,
            writer,
            writer_opts.write_mode,
            &fetch_opts,
        ).await?;

        println!("✅ Done: {name}");
    }

    Ok(())
}
