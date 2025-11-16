use apitap::{
    cmd::{run_pipeline, Cli},
    log,
};
use clap::Parser;
use dotenvy::dotenv;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    // Load `.env` if present so env var-backed credentials are available.
    dotenv().ok();

    // Parse CLI early so we can set log-related env vars before initializing tracing
    let cli = Cli::parse();

    // Initialize tracing from CLI flags without mutating global env vars
    log::init_tracing_with(cli.log_level.as_deref(), cli.log_json);

    let res = run_pipeline(&cli.modules, &cli.yaml_config).await;

    if res.is_err() {
        // do NOT print the error again; the #[instrument(err)] already logged it
        return ExitCode::from(1);
    }

    ExitCode::SUCCESS
}
