use prometheus::{Histogram, HistogramOpts, IntCounter, IntCounterVec, Opts};

use once_cell::sync::Lazy;

/// Total proxy requests counter (labeled by method, path, status)
static PROXY_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    IntCounterVec::new(
        Opts::new(
            "spectre_proxy_requests_total",
            "Total number of HTTP requests handled by the proxy",
        ),
        &["method", "path", "status"],
    )
    .expect("Failed to create spectre_proxy_requests_total metric")
});

/// Request duration histogram (seconds)
static PROXY_REQUEST_DURATION: Lazy<Histogram> = Lazy::new(|| {
    Histogram::with_opts(
        HistogramOpts::new(
            "spectre_proxy_request_duration_seconds",
            "HTTP request duration in seconds",
        )
        .buckets(vec![
            0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ]),
    )
    .expect("Failed to create spectre_proxy_request_duration_seconds metric")
});

/// Events published counter
static EVENTS_PUBLISHED_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    IntCounter::new(
        "spectre_events_published_total",
        "Total number of events published to NATS",
    )
    .expect("Failed to create spectre_events_published_total metric")
});

/// Register all custom metrics with the default prometheus registry.
/// Should be called once at startup.
pub fn register_metrics() {
    let registry = prometheus::default_registry();

    // Ignore AlreadyReg errors (idempotent)
    let _ = registry.register(Box::new(PROXY_REQUESTS_TOTAL.clone()));
    let _ = registry.register(Box::new(PROXY_REQUEST_DURATION.clone()));
    let _ = registry.register(Box::new(EVENTS_PUBLISHED_TOTAL.clone()));
}

/// Record an HTTP request
pub fn record_request(method: &str, path: &str, status: u16) {
    PROXY_REQUESTS_TOTAL
        .with_label_values(&[method, path, &status.to_string()])
        .inc();
}

/// Start a request duration timer. Call `.observe_duration()` on the returned guard when done.
pub fn start_request_timer() -> prometheus::HistogramTimer {
    PROXY_REQUEST_DURATION.start_timer()
}

/// Increment the events published counter
pub fn record_event_published() {
    EVENTS_PUBLISHED_TOTAL.inc();
}
