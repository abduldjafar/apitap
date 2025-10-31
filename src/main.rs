use apitap::{cmd::{Cli, run_pipeline}, config::init_tracing, errors::Result};
use clap::Parser;


#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();
    run_pipeline(&cli.modules, &cli.yaml_config).await
}
