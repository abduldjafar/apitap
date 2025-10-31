use std::sync::{Arc, Mutex};

use crate::config::load_config_from_path;
use crate::config::templating::{RenderCapture, build_env_with_captures, list_sql_templates, render_one};
use crate::errors::{self, Result};
use crate::http::Http;
use crate::pipeline::SinkConn;
use crate::pipeline::run::{FetchOpts, run_fetch};
use crate::pipeline::sink::{WriterOpts, MakeWriter};
use crate::writer::WriteMode;
use clap::Parser;

const CONCURRENCY: usize = 5;
const DEFAULT_PAGE_SIZE: usize = 50;
const FETCH_BATCH_SIZE: usize = 256;

/// Run ETL pipelines from SQL templates
#[derive(Parser, Debug)]
#[command(name = "apitap-run", version)]
pub struct Cli {
    /// Folder containing SQL templates (Minijinja)
    #[arg(long = "modules", short = 'm', default_value = "pipelines", value_name = "DIR")]
    pub modules: String,

    /// YAML config file
    #[arg(long = "yaml-config", short = 'y', default_value = "pipelines.yaml", value_name = "FILE")]
    pub yaml_config: String,
}


/// Run all templates under `root` using configuration from `cfg_path`.
pub async fn run_pipeline(root: &str, cfg_path: &str) -> Result<()> {
    // 1) Discover templates + load config
    let names = list_sql_templates(root)?;
    let cfg = load_config_from_path(cfg_path)?;

    // 2) Build templating env that captures sink()/use_source()
    let capture = Arc::new(Mutex::new(RenderCapture::default()));
    let env = build_env_with_captures(root, &capture);

    // 3) Shared fetch options
    let fetch_opts = FetchOpts {
        concurrency: CONCURRENCY,
        default_page_size: DEFAULT_PAGE_SIZE,
        fetch_batch_size: FETCH_BATCH_SIZE,
    };

    // 4) Process each template
    for name in names {
        let rendered = render_one(&env, &capture, &name)?;
        let source_name = &rendered.capture.source;
        let sink_name   = &rendered.capture.sink;

        println!("\n=== {name} ===");
        println!("source : {source_name}");
        println!("sink   : {sink_name}");

        // Resolve source/target from config
        let src = cfg.source(source_name).ok_or_else(|| {
            errors::Error::Reqwest(format!("source not found in config: {source_name}"))
        })?;
        let tgt = cfg.target(sink_name).ok_or_else(|| {
            errors::Error::Reqwest(format!("target not found in config: {sink_name}"))
        })?;

        // HTTP client
        let http   = Http::new(src.url.clone());
        let client = http.build_client();
        let url    = reqwest::Url::parse(&http.get_url())?;


        // Destination table + inject into SQL
        let dest_table = src.table_destination_name.as_deref().ok_or_else(|| {
            errors::Error::Reqwest(format!(
                "table_destination_name is required for source: {source_name}"
            ))
        })?;
        let sql = rendered.sql.replace(source_name, dest_table);

        // Target writer via factory
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

        let conn = tgt.create_conn().await?;
        let (writer, maybe_truncate) = conn.make_writer(&writer_opts)?;
        if let Some(hook) = maybe_truncate {
            hook().await?;
        }

        // Fetch → write
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
