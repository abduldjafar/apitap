use apitap::{
    cmd::{run_pipeline, Cli},
    log,
};
use clap::Parser;
use dotenvy::dotenv;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    dotenv().ok();
    let cli = Cli::parse();
    log::init_tracing_with(cli.log_level.as_deref(), cli.log_json);

    match run_pipeline(&cli.modules, &cli.yaml_config).await {
        Ok(_) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(1),
    }
}
