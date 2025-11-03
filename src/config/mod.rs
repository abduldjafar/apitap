use std::{fs::File, path::Path};
use crate::errors::Result;
use crate::pipeline::Config as PipelineConfig;

pub mod templating;

pub fn load_config_from_path<P: AsRef<Path>>(path: P) -> Result<PipelineConfig> {
    let f = File::open(path)?;
    Ok(serde_yaml::from_reader(f)?)
}