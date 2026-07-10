//! Logging and tracing setup for SPECTRE Fleet
//!
//! Provides structured logging with tracing integration.

use crate::{Config, ServiceId};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Initialize logging for a SPECTRE service
///
/// Sets up structured logging with:
/// - JSON formatting (for production)
/// - Pretty formatting (for development)
/// - Environment-based log level filtering
/// - Service context injection
///
/// # Examples
///
/// ```no_run
/// use spectre_core::{Config, init_logging};
///
/// # fn main() -> spectre_core::Result<()> {
/// let config = Config::from_env()?;
/// init_logging(&config)?;
///
/// tracing::info!("Service started");
/// # Ok(())
/// # }
/// ```
pub fn init_logging(config: &Config) -> crate::Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&config.observability.log_level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let service_id = ServiceId::new(&config.service.id);
    let environment = config.service.environment.clone();

    // JSON formatter for production
    let json_layer = if config.service.environment == "prod" {
        Some(
            fmt::layer()
                .json()
                .with_target(true)
                .with_level(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .with_filter(env_filter.clone()),
        )
    } else {
        None
    };

    // Pretty formatter for development
    let pretty_layer = if config.service.environment != "prod" {
        Some(
            fmt::layer()
                .pretty()
                .with_target(true)
                .with_level(true)
                .with_thread_ids(false)
                .with_file(true)
                .with_line_number(true)
                .with_filter(env_filter),
        )
    } else {
        None
    };

    // Build subscriber
    tracing_subscriber::registry()
        .with(json_layer)
        .with(pretty_layer)
        .try_init()
        .map_err(|e| {
            crate::error::SpectreError::internal(format!("Failed to init logging: {}", e))
        })?;

    tracing::info!(
        service = %service_id,
        environment = %environment,
        version = %config.service.version,
        "SPECTRE service logging initialized"
    );

    Ok(())
}

/// Create a span for a correlation context
///
/// Utility to create a tracing span with correlation and trace IDs.
///
/// # Examples
///
/// ```
/// use spectre_core::{CorrelationId, TraceId};
/// use spectre_core::logging::correlation_span;
///
/// let correlation_id = CorrelationId::generate();
/// let trace_id = TraceId::generate();
///
/// let _span = correlation_span("handle_request", correlation_id, trace_id);
/// tracing::info!("Processing request"); // Will include correlation/trace IDs
/// ```
#[macro_export]
macro_rules! correlation_span {
    ($name:expr, $correlation_id:expr, $trace_id:expr) => {
        tracing::info_span!(
            $name,
            correlation_id = %$correlation_id,
            trace_id = %$trace_id
        )
    };
}

pub use correlation_span;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logging_init() {
        std::env::set_var("SPECTRE_SERVICE_ID", "test-service");
        let config = Config::from_env().unwrap();
        assert!(init_logging(&config).is_ok());
    }
}
