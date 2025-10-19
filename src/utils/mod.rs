#[macro_export]
macro_rules! impl_from_error {
    ($($type:ty => $variant:ident),* $(,)?) => {
        $(impl From<$type> for Error {
            fn from(error: $type) -> Self {
                tracing::error!("{}", error);
                Error::$variant(error.to_string())
            }
        })*
    };
}
pub mod datafusion_ext;
pub mod http_fetcher;
pub mod postgres_writer;
