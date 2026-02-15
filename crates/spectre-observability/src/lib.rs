//! SPECTRE Observability - Metrics and Tracing
//!
//! Provides centralized logging, tracing, and metrics setup for all SPECTRE services.
//! Integrates OpenTelemetry (OTLP) for distributed tracing and Prometheus for metrics.

pub mod metrics;

use anyhow::Result;
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::{Protocol, WithExportConfig};
use opentelemetry_sdk::{
    propagation::TraceContextPropagator, resource::Resource, trace as sdktrace,
};
use opentelemetry_semantic_conventions::resource;
use prometheus::{Encoder, TextEncoder};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

/// Initialize observability (Tracing + Logging + Metrics)
///
/// Returns `Result` instead of panicking on initialization failures.
pub fn init(service_name: &str) -> Result<()> {
    // 1. Set global propagator and error handler
    global::set_text_map_propagator(TraceContextPropagator::new());
    global::set_error_handler(|err| {
        eprintln!("[OTEL ERROR] {}", err);
    })
    .ok();

    // 2. Register custom Prometheus metrics
    metrics::register_metrics();

    // 3. Configure Logging Layer
    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let env_filter = EnvFilter::new(log_level);
    let log_format = std::env::var("SPECTRE_LOG_FORMAT").unwrap_or_else(|_| "pretty".to_string());

    // 4. Helper to create tracer (returns Result instead of panicking)
    let create_tracer = || -> Option<Result<opentelemetry_sdk::trace::Tracer>> {
        let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok()?;

        let resource = Resource::new(vec![
            KeyValue::new(resource::SERVICE_NAME, service_name.to_string()),
            KeyValue::new(resource::SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
        ]);

        // Configurable sampler: OTEL_TRACES_SAMPLER_ARG (0.0 - 1.0, default 0.1 = 10%)
        let sample_ratio: f64 = std::env::var("OTEL_TRACES_SAMPLER_ARG")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.1);

        let sampler = sdktrace::Sampler::TraceIdRatioBased(sample_ratio);

        // Use OTEL_EXPORTER_OTLP_PROTOCOL to select transport (default: http/protobuf)
        let protocol = std::env::var("OTEL_EXPORTER_OTLP_PROTOCOL")
            .unwrap_or_else(|_| "http/protobuf".to_string());

        let tracer_result = if protocol == "grpc" {
            opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(
                    opentelemetry_otlp::new_exporter()
                        .tonic()
                        .with_endpoint(&otlp_endpoint),
                )
                .with_trace_config(
                    sdktrace::config()
                        .with_resource(resource)
                        .with_sampler(sampler),
                )
                .install_batch(opentelemetry_sdk::runtime::Tokio)
        } else {
            // HTTP/protobuf (more reliable, works with Jaeger OTLP HTTP endpoint)
            opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(
                    opentelemetry_otlp::new_exporter()
                        .http()
                        .with_endpoint(&otlp_endpoint)
                        .with_protocol(Protocol::HttpBinary),
                )
                .with_trace_config(
                    sdktrace::config()
                        .with_resource(resource)
                        .with_sampler(sampler),
                )
                .install_batch(opentelemetry_sdk::runtime::Tokio)
        }
        .map_err(|e| anyhow::anyhow!("Failed to initialize OTLP tracer: {}", e));

        Some(tracer_result)
    };

    // 5. Compose Subscriber
    let registry = Registry::default().with(env_filter);

    if log_format == "json" {
        let fmt_layer = tracing_subscriber::fmt::layer()
            .json()
            .with_target(true)
            .with_level(true);

        match create_tracer() {
            Some(Ok(tracer)) => {
                let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
                registry.with(fmt_layer).with(otel_layer).init();
            }
            Some(Err(e)) => {
                eprintln!("Warning: OTLP tracer init failed: {}. Continuing without tracing.", e);
                registry.with(fmt_layer).init();
            }
            None => {
                registry.with(fmt_layer).init();
            }
        }
    } else {
        let fmt_layer = tracing_subscriber::fmt::layer()
            .pretty()
            .with_target(true)
            .with_level(true);

        match create_tracer() {
            Some(Ok(tracer)) => {
                let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
                registry.with(fmt_layer).with(otel_layer).init();
            }
            Some(Err(e)) => {
                eprintln!("Warning: OTLP tracer init failed: {}. Continuing without tracing.", e);
                registry.with(fmt_layer).init();
            }
            None => {
                registry.with(fmt_layer).init();
            }
        }
    }

    tracing::info!("Observability initialized for service: {}", service_name);
    Ok(())
}

/// Shutdown observability (flush traces)
pub fn shutdown() {
    global::shutdown_tracer_provider();
}

/// Gather metrics for export (e.g. via /metrics endpoint)
pub fn gather_metrics() -> String {
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        tracing::error!("Failed to encode metrics: {}", e);
        return String::new();
    }
    String::from_utf8(buffer).unwrap_or_default()
}
