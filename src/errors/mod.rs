pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone, Debug)]
pub enum Error {
    Datafusion(String),
    Io(String),
    Reqwest(String),
    HeaderName(String),
    HeaderValue(String),
    SerdeJson(String),
    Sqlx(String),
    JoinError(String),
    Arrow(String),
    Parquet(String),
    SerdeArrow(String),
    SerdeYaml(String),
}

impl core::fmt::Display for Error {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::result::Result<(), core::fmt::Error> {
        write!(fmt, "{self:?}")
    }
}

impl std::error::Error for Error {}

crate::impl_from_error!(
    std::io::Error => Io,
    reqwest::Error => Reqwest,
    reqwest::header::InvalidHeaderValue => HeaderValue,
    reqwest::header::InvalidHeaderName => HeaderName,
    serde_json::Error => SerdeJson,
    sqlx::Error => Sqlx,
    tokio::task::JoinError => JoinError,
    datafusion::error::DataFusionError => Datafusion,
    datafusion::arrow::error::ArrowError => Arrow,
    datafusion::parquet::errors::ParquetError => Parquet,
    serde_arrow::Error=>SerdeArrow,
    serde_yaml::Error=>SerdeYaml
);
