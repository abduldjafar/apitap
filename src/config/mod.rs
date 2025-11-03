use std::{fs::File, path::Path};

use tracing_subscriber::EnvFilter;

use crate::errors::Result;
use crate::pipeline::Config as PipelineConfig;

pub mod templating;

pub fn load_config_from_path<P: AsRef<Path>>(path: P) -> Result<PipelineConfig> {
    let f = File::open(path)?;
    Ok(serde_yaml::from_reader(f)?)
}

pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
