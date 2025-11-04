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

    // Initialize tracing from CLI flags without mutating global env vars
    log::init_tracing_with(cli.log_level.as_deref(), cli.log_json);

    run_pipeline(&cli.modules, &cli.yaml_config).await
}
