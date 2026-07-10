# SPECTRE Observability

**Unified Telemetry Library for SPECTRE Fleet**

This crate provides a "one-line" initialization for:
- **Structured Logging**: via `tracing` (JSON or Pretty format).
- **Distributed Tracing**: via `opentelemetry` and `opentelemetry-otlp` (OpenTelemetry Protocol).
- **Metrics**: via `prometheus`.

## Usage

Add to `Cargo.toml`:
```toml
[dependencies]
spectre-observability = { path = "../spectre-observability" }
```

Initialize in `main.rs`:
```rust
fn main() {
    // Initializes Logs, Traces (if configured), and Metrics registry
    spectre_observability::init("my-service-id");
    
    tracing::info!("Service started");
}
```

## Configuration

Controlled via Environment Variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Log level filter | `info` |
| `SPECTRE_LOG_FORMAT` | Log output format (`json` or `pretty`) | `pretty` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OTLP Collector URL (e.g. `http://localhost:4317`) | `None` (Tracing disabled) |

## Metrics

To expose Prometheus metrics, you must create an HTTP endpoint in your service that calls:

```rust
let encoder = spectre_observability::gather_metrics();
// return encoder as HTTP response body
```
