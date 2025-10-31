use std::sync::{Arc, Mutex};

use crate::config::load_config_from_path;
use crate::config::templating::{
    RenderCapture, build_env_with_captures, list_sql_templates, render_one,
};
use crate::errors::{self, Result};
use crate::http::Http;
use crate::pipeline::SinkConn;
use crate::pipeline::run::{FetchOpts, run_fetch};
use crate::pipeline::sink::{MakeWriter, WriterOpts};
use crate::writer::WriteMode;
use clap::Parser;

const CONCURRENCY: usize = 5;
const DEFAULT_PAGE_SIZE: usize = 50;
const FETCH_BATCH_SIZE: usize = 256;

/// CLI
#[derive(Parser, Debug)]
#[command(
    name = "apitap-run",
    version,
    about = "Extract from REST APIs, transform with SQL, load to warehouses.",
    long_about = "Extract from REST APIs, transform with SQL, load to warehouses.\n\
HTTP-to-warehouse ETL powered by DataFusion.\n\n\
Resources:\n  • Modules: Jinja-like SQL templates that declare {{ sink(...) }} and {{ use_source(...) }}\n  • YAML config: defines sources (HTTP + pagination) and targets (warehouses)\n  • Execution: fetch JSON → DataFusion SQL → write via sink-specific writers",
)]
pub struct Cli {
    #[arg(long = "modules", short = 'm', value_name = "DIR", default_value = "pipelines")]
    pub modules: String,

    #[arg(long = "yaml-config", short = 'y', value_name = "FILE", default_value = "pipelines.yaml")]
    pub yaml_config: String,
}

pub async fn run_pipeline(root: &str, cfg_path: &str) -> Result<()> {
    let names = list_sql_templates(root)?;
    let cfg = load_config_from_path(cfg_path)?;

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
        let sink_name = &rendered.capture.sink;


        let src = cfg.source(source_name).ok_or_else(|| {
            errors::Error::Reqwest(format!("source not found in config: {source_name}"))
        })?;
        let tgt = cfg.target(sink_name).ok_or_else(|| {
            errors::Error::Reqwest(format!("target not found in config: {sink_name}"))
        })?;

        let http = Http::new(src.url.clone());
        let client = http.build_client();
        let url = reqwest::Url::parse(&http.get_url())?;

        let dest_table = src.table_destination_name.as_deref().ok_or_else(|| {
            errors::Error::Reqwest(format!(
                "table_destination_name is required for source: {source_name}"
            ))
        })?;
        let sql = rendered.sql.replace(source_name, dest_table);

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

        run_fetch(
            client,
            url,
            &src.pagination,
            &sql,
            dest_table,
            writer,
            writer_opts.write_mode,
            &fetch_opts,
        )
        .await?;

    }

    Ok(())
}
