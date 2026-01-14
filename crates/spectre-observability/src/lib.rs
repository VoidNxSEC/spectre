//! SPECTRE Observability - Metrics and Tracing
//!
//! Provides centralized logging, tracing, and metrics setup for all SPECTRE services.

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize tracing with JSON output and environment filter
pub fn init(service_name: &str) {
    // Default to info, allowing overrides via RUST_LOG
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,spectre_proxy=debug,spectre_core=debug"));

    // Check if we want pretty logs (dev) or JSON logs (prod)
    // For now, we default to pretty logs for better DX during development
    // In production, we would check an env var like SPECTRE_LOG_FORMAT=json
    let format = std::env::var("SPECTRE_LOG_FORMAT").unwrap_or_else(|_| "text".to_string());

    if format == "json" {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().pretty())
            .init();
    }

    tracing::info!("Observability initialized for service: {}", service_name);
}
