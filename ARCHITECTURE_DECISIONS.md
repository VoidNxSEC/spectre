# SPECTRE Architecture Decisions Record

**Project**: SPECTRE Fleet
**Date**: 2026-02-15
**Status**: Active Development (Phase 2 → Phase 3)

---

## 🎯 Critical Decisions

### ADR-001: Nix-First Kubernetes Orchestration over Helm
**Status**: ✅ Accepted & Implemented
**Classification**: Critical

**Decision**: Use Nix modules to generate Kubernetes manifests declaratively instead of Helm charts as primary deployment method.

**Context**:
- Required reproducible deployments across dev/prod environments
- Wanted to eliminate Docker daemon dependency
- Needed build-time validation of configurations
- Already using Nix flakes for development environment

**Rationale**:
- **Reproducibility**: Nix content-addressable store guarantees bit-for-bit identical builds
- **Type Safety**: Nix language catches configuration errors at build-time
- **No Docker Daemon**: OCI images built with `dockerTools.buildLayeredImage`
- **Hermetic Builds**: All dependencies pinned in `flake.lock`, zero version drift
- **Developer Isolation**: `nix develop .#kubernetes` provides isolated K8s tooling

**Trade-offs Accepted**:
- ✅ Accept: Steeper learning curve for Nix vs Helm
- ✅ Accept: Smaller community ecosystem
- ✅ Accept: JSON output instead of YAML (both valid for K8s)
- ✅ Gain: Build-time correctness guarantees
- ✅ Gain: Zero configuration drift

**Implementation**:
- `nix/lib/k8s.nix`: Helper functions (mkLabels, mkContainer, mkHttpProbe)
- `nix/kubernetes/*.nix`: Resource modules (Deployment, Service, ConfigMap, Ingress)
- `nix/images/spectre-proxy.nix`: Container image builder
- `flake.nix`: Packages (manifests) and apps (deploy scripts)

**References**:
- User request: "Vamos criar modulos nix para orquestrar as fleets de kube"
- Files: `nix/kubernetes/default.nix`, `flake.nix`, `KUBERNETES.md`
- Helm chart maintained as fallback option

---

### ADR-002: Argon2id KDF for Secret Encryption (Critical Security Fix)
**Status**: ✅ Accepted & Implemented
**Classification**: Critical

**Decision**: Replace weak XOR-based key derivation with Argon2id algorithm for AES-256 key generation.

**Context**:
- Original implementation used XOR (`password.bytes().cycle()`) to derive AES keys
- Short passwords could generate keys with zero bytes (critical vulnerability)
- Salt parameter was present but ignored
- TODO comment acknowledged the weakness

**Rationale**:
- **Security**: Argon2id is OWASP/NIST recommended for password-based key derivation
- **Resistance**: Protected against GPU/ASIC brute-force attacks
- **Memory-Hard**: Prevents parallel cracking attempts
- **Proper Salting**: Uses cryptographically random salt via `OsRng`

**Implementation**:
```rust
// Before (VULNERABLE):
let key_bytes: Vec<u8> = password.bytes().cycle().take(32).collect();

// After (SECURE):
let mut key_bytes = [0u8; 32];
let argon2 = Argon2::default();
argon2.hash_password_into(password.as_bytes(), salt, &mut key_bytes)?;
```

**Files Modified**:
- `crates/spectre-secrets/src/crypto.rs`: Argon2id implementation
- `crates/spectre-secrets/Cargo.toml`: Added `argon2`, `rand` deps
- `crates/spectre-secrets/src/lib.rs`: Wire CryptoEngine modules

**Impact**: Fixed critical security vulnerability that could lead to secret leakage

---

### ADR-003: Ingress + cert-manager Architecture over Service Mesh
**Status**: ✅ Accepted & Implemented
**Classification**: Major

**Decision**: Use nginx-ingress + cert-manager for TLS termination instead of service mesh (Istio/Linkerd).

**Context**:
- Three options evaluated: Ingress, LoadBalancer, Service Mesh
- Need TLS termination, routing, and certificate management
- Want to minimize operational complexity for initial deployment

**Alternatives Considered**:

**Option A: Service Mesh (Istio/Linkerd)**
- Pros: mTLS, advanced traffic management, observability
- Cons: High complexity, resource overhead, steep learning curve
- **Rejected**: Over-engineered for current scale

**Option B: LoadBalancer per Service**
- Pros: Simple, direct
- Cons: Multiple IPs, no path routing, manual cert management
- **Rejected**: Not cost-effective, limited routing

**Option C: Ingress + cert-manager (CHOSEN)**
- Pros: Single entry point, path-based routing, automatic TLS, battle-tested
- Cons: Less sophisticated traffic policies than mesh
- **Accepted**: Right balance of features vs complexity

**Implementation**:
- nginx-ingress controller for L7 routing
- cert-manager for Let's Encrypt certificate automation
- Ingress resource in `nix/kubernetes/ingress.nix`
- TLS disabled in proxy (Ingress handles it)

**Configuration**:
```nix
ingress = {
  enabled = true;
  className = "nginx";
  host = "spectre.production.com";
  tls = {
    enabled = true;
    issuer = "letsencrypt-prod";
  };
};
```

---

### ADR-004: NATS JetStream Event-Driven Architecture
**Status**: ✅ Accepted & Implemented (Inherited from Phase 1)
**Classification**: Critical

**Decision**: Use NATS JetStream as the event bus for asynchronous communication between services.

**Context**:
- SPECTRE is event-driven microservices architecture
- Need reliable message delivery, at-least-once semantics
- Require pub/sub patterns and stream persistence

**Rationale**:
- **Performance**: 11M+ msgs/sec throughput
- **Persistence**: JetStream provides durable streams
- **Simplicity**: Simpler than Kafka, lighter than RabbitMQ
- **Cloud-Native**: CNCF project, K8s native operators

**Implementation**:
- `crates/spectre-events/src/client.rs`: NATS client wrapper
- Automatic reconnection enabled
- Connection health checks via `is_connected()`
- Event publishing for proxy requests

**Fixes Applied**:
- Enabled `retry_on_initial_connect()` for resilience
- Fixed `is_connected()` to check `!client.is_closed()`
- Added connection state logging

---

## ⚙️ Major Decisions

### ADR-005: Token Bucket Rate Limiting Strategy
**Status**: ✅ Accepted & Implemented
**Classification**: Major

**Decision**: Implement token bucket algorithm for request rate limiting with configurable RPS and burst.

**Rationale**:
- **Fair**: Allows burst traffic while maintaining average rate
- **Configurable**: `RATE_LIMIT_RPS` and `RATE_LIMIT_BURST` env vars
- **Standard**: Industry-standard algorithm
- **Efficient**: O(1) token check per request

**Implementation**:
- Tower-governor middleware integration
- Per-IP rate limiting
- 429 responses with `Retry-After` header
- Default: 100 RPS, 200 burst (prod), 1000 RPS (dev)

**Files**: `crates/spectre-proxy/src/main.rs`, `Cargo.toml` (tower-governor)

---

### ADR-006: Three-Tier RBAC Hierarchy (admin > service > readonly)
**Status**: ✅ Accepted & Implemented
**Classification**: Major

**Decision**: Enforce role-based access control with three levels: admin, service, readonly.

**Hierarchy**:
```
admin    → Full access (all endpoints)
service  → Write access (ingest, proxy, events)
readonly → Read-only (health, metrics, status)
```

**Rationale**:
- **Principle of Least Privilege**: Services only get necessary permissions
- **Defense in Depth**: JWT validation + role verification
- **Auditability**: Role logged in every request

**Implementation**:
```rust
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Role { Readonly, Service, Admin }

fn required_role_for_path(path: &str) -> Role {
    match path {
        p if p.starts_with("/admin") => Role::Admin,
        p if p.starts_with("/ingest") || p.starts_with("/proxy") => Role::Service,
        _ => Role::Readonly,
    }
}
```

**Files**: `crates/spectre-proxy/src/main.rs` (auth middleware)

---

### ADR-007: Prometheus + OTLP Observability Stack
**Status**: ✅ Accepted & Implemented
**Classification**: Major

**Decision**: Use Prometheus for metrics and OTLP for distributed tracing.

**Components**:
- **Prometheus**: Scrapes `/metrics` endpoint for custom metrics
- **OTLP Exporter**: Sends traces to Tempo/Jaeger
- **Custom Metrics**:
  - `spectre_proxy_requests_total` (counter)
  - `spectre_proxy_request_duration_seconds` (histogram)
  - `spectre_events_published_total` (counter)

**Rationale**:
- **Standard**: OpenTelemetry is industry standard
- **Vendor Neutral**: Can switch backends easily
- **Comprehensive**: Metrics + Traces + Logs
- **Configurable Sampling**: 10% default, 100% in dev

**Implementation**:
- `crates/spectre-observability/src/lib.rs`: OTLP setup
- `crates/spectre-observability/src/metrics.rs`: Prometheus metrics
- `prometheus.yml`: Scrape configuration
- Configurable via `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_TRACES_SAMPLER_ARG`

**Fixes Applied**:
- Removed `.unwrap()` that could panic
- Separated json/pretty formatter branches for type safety
- Added error handling for tracer initialization

---

### ADR-008: Graceful Shutdown with SIGTERM/SIGINT Handling
**Status**: ✅ Accepted & Implemented
**Classification**: Major

**Decision**: Implement graceful shutdown to prevent dropped requests during rolling updates.

**Context**:
- Kubernetes sends SIGTERM before killing pods
- Need to drain in-flight requests before exit
- Observability shutdown must happen cleanly

**Implementation**:
```rust
// crates/spectre-core/src/shutdown.rs
pub async fn shutdown_signal() {
    let ctrl_c = signal::ctrl_c();
    let terminate = signal::unix::signal(SignalKind::terminate());

    tokio::select! {
        _ = ctrl_c => info!("Received SIGINT"),
        _ = terminate => info!("Received SIGTERM"),
    }
}

// In proxy main.rs
axum::serve(listener, app)
    .with_graceful_shutdown(shutdown_signal())
    .await?;
```

**Benefits**:
- Zero-downtime deployments
- Proper cleanup of connections
- Observability flush before exit

**Files**:
- `crates/spectre-core/src/shutdown.rs` (new)
- `crates/spectre-proxy/src/main.rs` (integration)

---

## 🔧 Implementation Decisions

### Shared HTTP Client with Connection Pooling
**Status**: ✅ Implemented
**Impact**: Performance, Resource Efficiency

**Decision**: Create single `reqwest::Client` in `AppState` instead of per-request allocation.

**Before**:
```rust
async fn proxy_handler() -> Result<Response, StatusCode> {
    let client = reqwest::Client::new(); // ❌ New client per request!
    let upstream_url = "http://localhost:8000"; // ❌ Hardcoded!
}
```

**After**:
```rust
struct AppState {
    http_client: reqwest::Client,  // ✅ Shared, pooled
    neutron_url: String,           // ✅ Configurable
}

// Client configured with timeouts and connection limits
reqwest::Client::builder()
    .timeout(Duration::from_secs(30))
    .connect_timeout(Duration::from_secs(5))
    .pool_max_idle_per_host(20)
    .build()?
```

---

### Health Check Endpoints Pattern
**Status**: ✅ Implemented
**Impact**: Kubernetes Integration

**Endpoints**:
- `/health` → Liveness probe (process alive)
- `/ready` → Readiness probe (NATS connected, upstream reachable)
- `/metrics` → Prometheus scraping

**Implementation**:
- All bypass authentication middleware
- `/ready` returns 503 if dependencies unavailable
- Used in K8s deployment probes:
  - `livenessProbe`: `/health` (10s interval)
  - `readinessProbe`: `/ready` (5s interval)
  - `startupProbe`: `/health` (2s interval, 30 retries)

---

### Structured Error Responses
**Status**: ✅ Implemented
**Impact**: API Consistency

**Decision**: Return JSON error responses instead of bare status codes.

**Format**:
```json
{
  "error": "Forbidden",
  "message": "Insufficient permissions: requires service role",
  "status": 403
}
```

**Implementation**:
```rust
struct ApiError {
    status: StatusCode,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({
            "error": self.status.canonical_reason().unwrap_or("Error"),
            "message": self.message,
            "status": self.status.as_u16()
        }))).into_response()
    }
}
```

---

## 📊 Summary

### By Phase

**Phase 1 (Complete)**: Event infrastructure, secret management foundations
**Phase 2 (Complete)**: Security hardening, observability, Kubernetes deployment
**Phase 3 (In Progress)**: Production operationalization, testing, optimization

### By Classification

- **Critical Decisions**: 4 (Nix K8s, Argon2id, Ingress, NATS)
- **Major Decisions**: 4 (Rate limiting, RBAC, Observability, Graceful shutdown)
- **Implementation Decisions**: 3 (HTTP pooling, Health checks, Error responses)

### Status

- ✅ **Accepted & Implemented**: 11 decisions
- 🔄 **In Progress**: None (ADR infrastructure itself)
- ❌ **Rejected**: Service mesh, LoadBalancer, XOR KDF

---

## 🔗 References

- **ADR System**: `/home/kernelcore/master/adr-ledger/`
- **Primary ADR**: ADR-0037 (Nix-First Kubernetes)
- **Source Code**: `/home/kernelcore/master/spectre/`
- **Documentation**: `KUBERNETES.md`, `HELM_CHART_SUMMARY.md`, `IMPLEMENTATION_REPORT.md`
- **Git Commits**: 10 commits on 2026-02-15 covering all implementations

---

*This document consolidates architectural decisions made during SPECTRE Phase 2 development.*
*For formal ADR tracking, see the adr-ledger repository.*
