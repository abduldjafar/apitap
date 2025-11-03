// src/error.rs
use thiserror::Error;
use tokio_util::codec::LinesCodecError;

/// Main error type for apitap operations
#[derive(Error, Debug)]
pub enum ApitapError {
    //LinesCodecError
    #[error("LinesCodecError error: {0}")]
    LinesCodecError(#[from] LinesCodecError),

    #[error("DataFusion error: {0}")]
    Datafusion(#[from] datafusion::error::DataFusionError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP request failed: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Invalid header name: {0}")]
    HeaderName(#[from] reqwest::header::InvalidHeaderName),

    #[error("Invalid header value: {0}")]
    HeaderValue(#[from] reqwest::header::InvalidHeaderValue),

    #[error("JSON serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Task join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("Arrow error: {0}")]
    Arrow(#[from] datafusion::arrow::error::ArrowError),

    #[error("Parquet error: {0}")]
    Parquet(#[from] datafusion::parquet::errors::ParquetError),

    #[error("Serde Arrow error: {0}")]
    SerdeArrow(#[from] serde_arrow::Error),

    #[error("YAML error: {0}")]
    SerdeYaml(#[from] serde_yaml::Error),

    #[error("Directory walk error: {0}")]
    WalkDir(#[from] walkdir::Error),

    #[error("Template error: {0}")]
    Minijinja(#[from] minijinja::Error),

    #[error("URL parse error: {0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Pagination error: {0}")]
    PaginationError(String),

    #[error("Writer error: {0}")]
    WriterError(String),

    #[error("Writer error: {0}")]
    PipelineError(String),

    #[error("Unsupported sink: {0}")]
    UnsupportedSink(String),

    #[error("Merge Error: {0}")]
    MergeError(String),
}

/// Convenience Result type that uses ApitapError
pub type Result<T> = std::result::Result<T, ApitapError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ApitapError::ConfigError("missing url".to_string());
        assert_eq!(err.to_string(), "Configuration error: missing url");
    }

    #[test]
    fn test_writer_error() {
        let err = ApitapError::WriterError("connection failed".to_string());
        assert!(err.to_string().contains("Writer error"));
    }
}
