// tracing_setup.rs
use tracing_error::ErrorLayer;
use tracing_subscriber::{EnvFilter, Registry, fmt, layer::SubscriberExt};

/// Initialize tracing subscriber.
///
/// Behavior:
/// - Log level is read from `APITAP_LOG_LEVEL` if set, otherwise falls back to `RUST_LOG` (via try_from_default_env),
///   then to `info`.
/// - Output format can be set via `APITAP_LOG_FORMAT=json` to enable JSON output. Any other value uses the default
///   human-readable formatter.
pub fn init_tracing() {
    // Allow explicit APITAP_LOG_LEVEL override, else fall back to RUST_LOG / default
    let filter = match std::env::var("APITAP_LOG_LEVEL") {
        Ok(lvl) => EnvFilter::new(lvl),
        Err(_) => EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
    };

    // JSON output opt-in (APITAP_LOG_FORMAT=json)
    let use_json = std::env::var("APITAP_LOG_FORMAT").map(|v| v.to_lowercase() == "json").unwrap_or(false);

    let fmt_layer = if use_json {
        fmt::layer()
            .json()
            .with_target(false)
            .with_file(false)
            .with_line_number(false)
    } else {
        fmt::layer()
            .with_target(false)
            .with_file(true)
            .with_line_number(true)
    };

    let subscriber = Registry::default()
        .with(filter)
        .with(fmt_layer)
        .with(ErrorLayer::default());

    tracing::subscriber::set_global_default(subscriber)
        .expect("failed to set global tracing subscriber");
}
