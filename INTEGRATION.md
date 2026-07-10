# SPECTRE Fleet Integration Specification

**Protocol Version**: 1.0
**Enforcement**: Mandatory for all Domain Services (`ai-agent-os`, `securellm-bridge`, etc.)

This document defines the strict architectural contract required for a service to join the SPECTRE Fleet.

---

## 1. The Service Lifecycle Contract

Every service participating in the fleet MUST adhere to the following lifecycle phases to ensure resilience and observability.

### Phase A: Bootstrapping & Identity

Services do not "just start". They must establish identity and observability context before processing any logic.

```rust
use spectre_core::{Result, Config, ServiceId};
use spectre_observability;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Strict Config Validation
    // Fail-fast if SPECTRE_SERVICE_ID or NATS_URL are missing.
    let config = Config::from_env().expect("CRITICAL: Invalid Service Configuration");
    
    // 2. Telemetry Injection
    // Must happen before ANY logic to capture startup traces.
    // Exports to: stdout (json/pretty) + OTLP Collector (if configured)
    spectre_observability::init(&config.service.id);
    
    tracing::info!(
        service_id = %config.service.id,
        version = env!("CARGO_PKG_VERSION"),
        "Service bootstrapping..."
    );

    // 3. Runtime Handover
    if let Err(e) = run_service(config).await {
        tracing::error!(error = %e, "Service crashed fatally");
        std::process::exit(1);
    }
    
    Ok(())
}
```

### Phase B: The Event Loop (Actor Pattern)

Do NOT write procedural scripts. Services must implement an asynchronous actor loop that handles:
1.  **NATS Connection State** (Auto-reconnect is handled by the crate, but logic must be idempotent).
2.  **Graceful Shutdown** (Listen for SIGTERM/SIGINT).

```rust
async fn run_service(config: Config) -> Result<()> {
    // Connection must use the resilience settings from spectre-core
    let bus = spectre_events::EventBus::connect(&config.nats.url).await?;
    
    tracing::info!("Connected to NATS JetStream backbone");

    // Service logic blocks here until shutdown signal
    tokio::select! {
        _ = start_consumers(&bus) => {
            tracing::warn!("Consumers exited unexpectedly");
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Shutdown signal received");
        }
    }

    spectre_observability::shutdown();
    Ok(())
}
```

---

## 2. Event Communication Protocol

### Topic Topology (Taxonomy)

Services MUST NOT invent arbitrary topics. Use the strict hierarchy:

| Pattern | Description | Example |
|---------|-------------|---------|
| `system.metrics.v1` | Hardware telemetry (readonly) | CPU, VRAM usage |
| `llm.request.v1` | RPC-style request to LLM Gateway | Prompt submission |
| `governance.vote.v1` | Critical immutable audit logs | Agent DAO decisions |
| `analysis.report.v1` | High-volume data streams | Arch Analyzer outputs |

### Publishing (With Context Propagation)

Never fire-and-forget without context. Every event implies a causation chain.

```rust
use spectre_events::{Event, EventType};
use spectre_core::CorrelationId;

async fn publish_inference_request(bus: &EventBus, input: String, parent_ctx: Option<CorrelationId>) -> Result<()> {
    // 1. Inherit or Generate Correlation ID
    let correlation_id = parent_ctx.unwrap_or_else(CorrelationId::generate);

    // 2. Construct Canonical Event
    let event = Event::new(
        EventType::InferenceRequest,
        ServiceId::new("ml-offload-api"),
        serde_json::json!({
            "model": "deepseek-r1",
            "input": input,
            "params": { "temperature": 0.7 }
        })
    ).with_correlation_id(correlation_id); // CRITICAL for distributed tracing

    // 3. Publish with Resilience
    // The EventBus client handles retry logic automatically for transient NATS errors
    bus.publish(&event).await?;
    
    tracing::debug!(
        event_id = %event.event_id,
        correlation_id = %event.correlation_id,
        "Dispatched inference request"
    );
    
    Ok(())
}
```

---

## 3. Observability Standards

### Distributed Tracing (OTLP)

`spectre-observability` automatically configures the OTLP exporter if `OTEL_EXPORTER_OTLP_ENDPOINT` is present.
Your responsibility is to **instrument critical paths**.

```rust
// Use the `tracing` macros to create spans. 
// These are automatically converted to OTLP spans.

#[tracing::instrument(skip(data))]
async fn process_heavy_computation(data: Vec<u8>) {
    tracing::info!("Starting vector embedding generation");
    
    // ... logic ...
    
    tracing::info!("Embedding complete");
}
```

### Metrics (Prometheus)

Services MUST expose a `/metrics` HTTP endpoint if they process heavy workloads.
The `spectre-observability` crate maintains the registry.

```rust
// In your Axum/Hyper router:
use axum::{routing::get, Router};

pub fn metrics_routes() -> Router {
    Router::new().route("/metrics", get(handle_metrics))
}

async fn handle_metrics() -> String {
    // Scrapes the global Prometheus registry managed by spectre-observability
    spectre_observability::gather_metrics()
}
```

---

## 4. Error Handling & Recovery

Errors fall into two categories. Handle them explicitly:

1.  **Transient (Network/Timeout):**
    *   **Strategy:** Exponential Backoff.
    *   **Implementation:** Handled by `spectre-events` client for NATS. For HTTP (Proxy), use `tower::retry`.

2.  **Poison Pills (Malformed Data):**
    *   **Strategy:** Dead Letter Queue (DLQ).
    *   **Implementation:** If `EventHandler::handle()` returns `Err`, the subscriber logs the error.
    *   **Requirement:** Do NOT crash the service on malformed JSON. Return a `SpectreError::Deserialization`.

---

## 5. Security & Secrets

Services MUST NOT store secrets in environment variables in plain text if possible.
Use `spectre-secrets` to retrieve rotated credentials at runtime.

*Future enforcement: Services will authenticate via mTLS certificates issued by the Vault.*
