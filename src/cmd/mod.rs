use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::config::load_config_from_path;
use crate::config::templating::{
    RenderCapture, build_env_with_captures, list_sql_templates, render_one,
};
use crate::errors::{self, Result};
use crate::http::Http;
use crate::http::fetcher::Pagination;
use crate::pipeline::SinkConn;
use crate::pipeline::run::{FetchOpts, run_fetch};
use crate::pipeline::sink::{MakeWriter, WriterOpts};
use crate::writer::WriteMode;
use clap::Parser;
use tracing::{debug, info, instrument, warn};

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
Resources:\n  • Modules: Jinja-like SQL templates that declare {{ sink(...) }} and {{ use_source(...) }}\n  • YAML config: defines sources (HTTP + pagination) and targets (warehouses)\n  • Execution: fetch JSON → DataFusion SQL → write via sink-specific writers"
)]
pub struct Cli {
    #[arg(
        long = "modules",
        short = 'm',
        value_name = "DIR",
        default_value = "pipelines"
    )]
    pub modules: String,

    #[arg(
        long = "yaml-config",
        short = 'y',
        value_name = "FILE",
        default_value = "pipelines.yaml"
    )]
    pub yaml_config: String,
}

fn _pagelabel(p: &Option<Pagination>) -> &'static str {
    match p {
        Some(Pagination::LimitOffset { .. }) => "limit_offset",
        Some(Pagination::PageNumber { .. }) => "page_number",
        Some(Pagination::PageOnly { .. }) => "page_only",
        Some(Pagination::Cursor { .. }) => "cursor",
        Some(Pagination::Default) => "default",
        None => "none",
    }
}

#[instrument(skip_all, fields(root, cfg_path))]
pub async fn run_pipeline(root: &str, cfg_path: &str) -> Result<()> {
    info!("starting apitap run");

    let t0 = Instant::now();

    // Discover + load
    let names = list_sql_templates(root)?;
    info!(count = names.len(), "discovered sql modules");

    let cfg = load_config_from_path(cfg_path)?;

    info!("loaded yaml config");

    // Build templating env
    let capture = Arc::new(Mutex::new(RenderCapture::default()));
    let env = build_env_with_captures(root, &capture);

    // Shared fetch options
    let fetch_opts = FetchOpts {
        concurrency: CONCURRENCY,
        default_page_size: DEFAULT_PAGE_SIZE,
        fetch_batch_size: FETCH_BATCH_SIZE,
    };
    debug!(?fetch_opts, "fetch options");

    // Process each template
    for (idx, name) in names.into_iter().enumerate() {
        let span = tracing::info_span!("module", idx = idx + 1, name = %name);
        let _g = span.enter();

        let m_t0 = Instant::now();
        let rendered = render_one(&env, &capture, &name)?;
        let source_name = &rendered.capture.source;
        let sink_name = &rendered.capture.sink;

        // Resolve source/target from config
        let src = match cfg.source(source_name) {
            Some(s) => s,
            None => {
                return Err(errors::ApitapError::PipelineError(format!(
                    "source not found in config: {source_name}"
                )));
            }
        };
        let tgt = match cfg.target(sink_name) {
            Some(t) => t,
            None => {
                return Err(errors::ApitapError::PipelineError(format!(
                    "target not found in config: {sink_name}"
                )));
            }
        };

        // HTTP client
        let http = Http::new(src.url.clone());
        let client = http.build_client();
        let url_s = http.get_url();
        let url = reqwest::Url::parse(&url_s)?;

        // Destination table + inject into SQL
        let dest_table = src.table_destination_name.as_deref().ok_or_else(|| {
            warn!(%source_name, "missing table_destination_name");
            errors::ApitapError::PipelineError(format!(
                "table_destination_name is required for source: {source_name}"
            ))
        })?;
        let sql = rendered.sql.replace(source_name, dest_table);

        // Target writer via factory
        let writer_opts = WriterOpts {
            dest_table,
            primary_key: Some("id".to_string()),
            batch_size: 50,
            sample_size: 10,
            auto_create: true,
            auto_truncate: false,
            truncate_first: false,
            write_mode: WriteMode::Merge,
        };
        debug!(?writer_opts, "writer opts");

        let conn = tgt.create_conn().await?;
        let (writer, maybe_truncate) = conn.make_writer(&writer_opts)?;
        if let Some(hook) = maybe_truncate {
            hook().await?;
        }

        info!("starting fetch → transform → load");
        let step_t0 = Instant::now();
        run_fetch(
            client,
            url,
            &src.pagination,
            &sql,
            dest_table,
            writer,
            writer_opts.write_mode,
            &fetch_opts,
            &src.retry,
        )
        .await?;
        info!(
            elapsed_ms = step_t0.elapsed().as_millis() as u64,
            "module completed"
        );

        info!(
            elapsed_ms = m_t0.elapsed().as_millis() as u64,
            "module finished"
        );
    }

    info!(
        total_ms = t0.elapsed().as_millis() as u64,
        "all modules finished"
    );
    Ok(())
}
