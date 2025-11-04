use apitap::{
    cmd::{Cli, run_pipeline},
    errors::Result,
    log,
};
use clap::Parser;
use dotenvy::dotenv;

#[tokio::main]
async fn main() -> Result<()> {
    // Load `.env` if present so env var-backed credentials are available.
    dotenv().ok();

    // Parse CLI early so we can set log-related env vars before initializing tracing
    let cli = Cli::parse();

    if cli.log_json {
        std::env::set_var("APITAP_LOG_FORMAT", "json");
    }
    if let Some(lvl) = cli.log_level.as_ref() {
        std::env::set_var("APITAP_LOG_LEVEL", lvl);
    }

    log::init_tracing();

    run_pipeline(&cli.modules, &cli.yaml_config).await
}
