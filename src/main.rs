use apitap::{cmd::run_pipeline, errors::Result,cmd::Cli};
use clap::Parser;


#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    run_pipeline(&cli.modules, &cli.yaml_config).await
}
