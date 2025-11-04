use apitap::{
    cmd::{Cli, run_pipeline},
    errors::Result,
    log,
};
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    log::init_tracing();
    let cli = Cli::parse();
    run_pipeline(&cli.modules, &cli.yaml_config).await
}
