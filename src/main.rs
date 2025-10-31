use apitap::{cmd::Cli, cmd::run_pipeline, errors::Result};
use clap::Parser;
use tracing_subscriber::EnvFilter;

fn init_tracing(pretty: bool) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    if pretty {
        tracing_subscriber::fmt().with_env_filter(filter).pretty().init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing(true);
    let cli = Cli::parse();
    run_pipeline(&cli.modules, &cli.yaml_config).await
}
