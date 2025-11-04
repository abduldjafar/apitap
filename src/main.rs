use apitap::{
    cmd::{Cli, run_pipeline},
    errors::Result,
    log,
};
use clap::Parser;
use dotenvy::dotenv;

#[tokio::main]
async fn main() -> Result<()> {
    log::init_tracing();
    // Load `.env` if present so env var-backed credentials are available.
    dotenv().ok();
    let cli = Cli::parse();
    run_pipeline(&cli.modules, &cli.yaml_config).await
}
