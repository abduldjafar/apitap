// tracing_setup.rs
use tracing_error::ErrorLayer;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter, Registry};

pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let subscriber = Registry::default()
        .with(filter)
        .with(
            fmt::layer()
                .with_target(false)
                .with_file(true)
                .with_line_number(true),
        )
        .with(ErrorLayer::default());

    tracing::subscriber::set_global_default(subscriber)
        .expect("failed to set global tracing subscriber");
}
