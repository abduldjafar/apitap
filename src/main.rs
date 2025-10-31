use apitap::{cmd::Cli, cmd::run_pipeline, errors::Result};
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    run_pipeline(&cli.modules, &cli.yaml_config).await
}
