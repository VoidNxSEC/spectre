# SPECTRE Roadmap

**Project**: SPECTRE Fleet - Enterprise-Grade AI Agent Framework
**Current Phase**: Phase 2 Complete → Phase 3 Starting
**Last Updated**: 2026-02-16

---

## ✅ Phase 1: Core Infrastructure (Complete)

**Timeline**: Q4 2025
**Status**: ✅ Done

- Event-driven architecture with NATS JetStream
- 5-crate workspace (core, events, proxy, secrets, observability)
- Basic proxy with JWT authentication
- Secret management foundations
- Development environment with Nix flakes

---

## ✅ Phase 2: Production Readiness (Complete)

**Timeline**: Q1 2026 (Jan-Feb)
**Status**: ✅ Done (22/22 core tasks)

### Security
- [x] Argon2id KDF (replaced weak XOR)
- [x] RBAC (admin > service > readonly)
- [x] Rate limiting (token bucket)
- [x] Circuit breaker pattern
- [x] SBOM generation (CycloneDX)

### Reliability
- [x] Retry logic with exponential backoff
- [x] Graceful shutdown (SIGTERM/SIGINT)
- [x] Health endpoints (/health, /ready, /metrics)
- [x] NATS auto-reconnection

### Observability
- [x] Custom Prometheus metrics (3 metrics)
- [x] OTLP tracing to Tempo/Jaeger
- [x] Structured JSON logging
- [x] Request instrumentation

### Infrastructure
- [x] Nix-first Kubernetes orchestration
- [x] Helm chart (17 files, 813 lines)
- [x] CI/CD pipeline (11 jobs)
- [x] Docker optimization (<50MB target)
- [x] Load testing script
- [x] Comprehensive documentation

### Documentation
- [x] Architecture Decision Records (11 ADRs)
- [x] KUBERNETES.md deployment guide
- [x] Helm chart documentation
- [x] Phase 2 completion report

**Deliverables**: 16 commits, 4,200+ lines of production code

---

## 🔄 Phase 3: Validation & Testing (In Progress)

**Timeline**: Q1 2026 (Feb-Mar)
**Focus**: Integration testing, deployment validation, load testing

### High Priority

#### #37: Nix-native NATS Module
**Status**: ✅ Done
**Tasks**:
- [x] Create `nix/services/nats/conf.nix` (nats.conf generator)
- [x] Create `nix/services/nats/default.nix` (mkConfig, mkServerPackage, environments)
- [x] Integrate into `flake.nix` (packages, apps, devShell)
- [x] Verify build: `nix build .#nats-server-dev`
- [x] ADR: NATS over Kafka decision registered

#### #38: NATS Integration Tests
**Status**: ✅ Done
**Dependencies**: Running NATS server (`nix run .#nats`)
**Tasks**:
- [x] Setup: `nix run .#nats` (replaces docker-compose)
- [x] Run: `cargo test --test test_event_bus` (10/10 passing)
- [x] Validate: Event publish/subscribe patterns
- [x] Validate: Request-reply with timeout
- [x] Fix: `is_connected()` race condition (flush on connect)
- [x] Document: NATS failure scenarios (`crates/spectre-events/NATS_FAILURE_SCENARIOS.md`)

#### #40: Local K8s Deployment
**Status**: ✅ Done
**Dependencies**: kind
**Tasks**:
- [x] Setup cluster: `kind create cluster --name spectre-dev`
- [x] Build + load image: `nix build .#spectre-proxy-image` + `kind load`
- [x] Deploy manifests: `kubectl apply -f` (Deployment, Service, ConfigMap, Ingress)
- [x] Test /health endpoint → 200 OK
- [x] Test /metrics endpoint → Prometheus metrics (3 metrics exposed)
- [x] Fix: Image tag mismatch (nix-dev vs dev), imagePullPolicy: Never
- [x] Fix: JWT_SECRET required in K8s Secret
- [x] Deploy NATS in-cluster for /ready probe (`nix/kubernetes/nats.nix`)
- [x] Fix: Image tag alignment (nix-dev), configmap NATS_URL → in-cluster DNS
- [x] Fix: Deploy script kind load support (`flake.nix`)
- [x] Validate: Ingress routing with nginx-ingress controller (in-cluster verified)

#### #42: Production Load Test
**Status**: ✅ Done
**Dependencies**: Full stack (NATS + proxy + neutron)
**Tasks**:
- [x] Create load test script: `./scripts/load-test.sh` (6 phases, per-phase execution)
- [x] Run: full stack load test (NATS + proxy + neutron, 2026-02-15)
- [x] Validate: Circuit breaker triggers (neutron killed → 503 circuit open → 30s → recovery → 200)
- [x] Validate: Rate limiting under burst (300 req burst, burst=200 → 204 passed, 96 rejected)
- [x] Profile: CPU/memory post-load
- [x] Document: Performance baseline

**Performance Baseline — Debug Build** (2026-02-15, localhost):
| Metric | Value |
|--------|-------|
| /health RPS | 27,693 |
| /health p50 / p95 / p99 | 1.6ms / 3.4ms / 5.0ms |
| /ingest (auth+rate limit) RPS | 14,713 |
| /ingest p50 / p95 / p99 | 1.8ms / 4.0ms / 5.9ms |
| Proxy → Neutron p50 / p95 / p99 | 0.8ms / 1.4ms / 2.6ms |
| Rate limiter accuracy (burst=200) | 204 passed / 96 rejected (300 burst) |
| Circuit breaker: open → recovery | 503 while open → 200 after 30s timeout |
| VmRSS (post-load) | 23.4 MB |
| Thread count | 3 (tokio runtime) |

**Performance Baseline — Release Build** (2026-02-16, localhost, 50 connections):
| Metric | Value |
|--------|-------|
| /health RPS | 58,130 |
| /health p50 / p95 / p99 | 0.5ms / 2.8ms / 4.6ms |
| /metrics (auth) RPS | 59,692 |
| /metrics p50 / p95 / p99 | 0.4ms / 2.6ms / 5.2ms |
| /ingest (auth+NATS) RPS | 68,903 |
| /ingest p50 / p95 / p99 | 0.4ms / 2.1ms / 4.0ms |
| /health (200 conns) RPS | 100,733 |
| /health (200 conns) p50 / p95 / p99 | 1.0ms / 6.3ms / 18.1ms |
| VmRSS (post-load) | 25.8 MB |
| Thread count | 13 (tokio runtime) |

**Notes**:
- Release build 2-4x faster than debug build across all endpoints
- Rate limiter correctly enforces per-IP with configurable burst
- Circuit breaker full lifecycle validated: closed → open (503) → half-open → closed (200)
- 100K+ RPS at high concurrency with sub-millisecond p50

### Medium Priority

#### #39: Property-Based Testing
**Status**: ✅ Done
**Dependencies**: proptest crate
**Tasks**:
- [x] Add proptest to spectre-secrets
- [x] Test: KDF determinism (same input → same output)
- [x] Test: Encryption roundtrip properties
- [x] Test: Salt uniqueness guarantees
- [x] Test: Key derivation edge cases
- [x] Test: Ciphertext overhead invariant (nonce + tag = 28 bytes)
- [x] Test: Non-deterministic encryption (random nonce)
- [x] Test: Tamper detection (bit-flip → decryption failure)
- [x] Test: Truncated ciphertext rejection
- [x] Fix: Salt minimum length validation (8 bytes, Argon2 requirement)

#### #41: E2E Trace Propagation
**Status**: ✅ Done
**Dependencies**: Jaeger or Tempo
**Tasks**:
- [x] Setup: `docker run jaegertracing/all-in-one:1.53` (ports 16686, 4317, 4318)
- [ ] Send request: proxy → neutron (deferred — neutron service not yet implemented)
- [x] Verify: Trace spans in Jaeger UI (spectre-proxy service visible, method/uri/duration tags)
- [x] Validate: Trace context propagation (W3C `traceparent` header → `CHILD_OF` refs in Jaeger)
- [x] Test: Sampling rate configuration (10% prod via `OTEL_TRACES_SAMPLER_ARG=0.1`, 100% dev)
- [x] Fix: OTLP gRPC/tonic silent failure → switched to HTTP/protobuf (ADR-0038)
- [x] Implement: `OtelMakeSpan` for W3C trace context extraction in tower-http TraceLayer

---

## 🚀 Phase 4: Enterprise Features (Planned)

**Timeline**: Q2 2026 (Apr-Jun)
**Focus**: Security hardening, multi-region, advanced reliability

### Security & Compliance

#### #43: Security Audit
**Status**: ✅ Done
**Priority**: High
**Results**:
- [x] Dependency audit: `cargo audit` - **0 vulnerabilities, 2 warnings**
  - Fixed: protobuf DoS (prometheus 0.13→0.14)
  - Fixed: time DoS (jsonwebtoken 9.2→10.3, async-nats 0.33→0.46)
  - Removed: bincode, dotenv (unmaintained, unused)
  - Warning: rustls-pemfile unmaintained (deferred to #44 TLS)
- [x] JWT validation edge cases - **9/9 tests passed**
  - ✓ Expired tokens rejected
  - ✓ Invalid signatures rejected
  - ✓ Missing claims rejected
  - ✓ Algorithm confusion (none) blocked
  - ✓ Malformed tokens rejected
- [x] RBAC bypass attempt testing - **7/7 tests passed**
  - ✓ Role hierarchy enforced (readonly < service < admin)
  - ✓ Invalid roles rejected
  - ✓ Case manipulation blocked
- [x] Rate limiting bypass testing - **5/5 tests passed**
  - ✓ 100 RPS limit enforced (226/250 passed, 24 rate-limited)
  - ✓ Bucket refill working
  - ✓ IP-based rate limiting
- [x] Secret exposure audit - **7/7 tests passed**
  - ✓ No secrets in git
  - ✓ No hardcoded credentials
  - ✓ .env files excluded
- [x] DoS resistance testing - **6/6 tests passed**
  - ✓ Large payloads handled
  - ✓ Connection exhaustion resistance
  - ✓ Slowloris resistance
  - ✓ Malformed input handling

### Optional Features

#### #44: TLS Implementation (Low Priority)
**Priority**: Low (Ingress handles TLS)
**Trigger**: Only if direct-to-pod TLS needed
**Tasks**:
- [ ] Implement: axum-server with rustls
- [ ] Load certs from K8s Secret
- [ ] Test with self-signed cert
- [ ] Document: When to use proxy TLS vs Ingress TLS

#### #45: Service Mesh Evaluation
**Status**: 🔄 In Progress
**Priority**: Medium
**Decision**: Linkerd (lightweight, low overhead, Rust-based proxy)
**Tasks**:
- [x] Research: Istio vs Linkerd vs Cilium → Linkerd chosen (simplicity, performance)
- [x] Install: Linkerd control plane on kind cluster (stable-2.14.9, nft iptables mode)
- [x] Mesh: spectre-proxy with automatic sidecar injection (2/2 containers)
- [x] Fix: NATS protocol detection skip (`config.linkerd.io/skip-outbound-ports: 4222`)
- [x] Benchmark: Release build baseline (58K-100K RPS, p50 < 1ms)
- [ ] Test: mTLS between proxy ↔ neutron (requires neutron container image)
- [ ] Benchmark: Mesh overhead (with vs without sidecar, p50/p95/p99 delta)
- [ ] Test: Linkerd traffic policies (retries, timeouts, traffic splitting)
- [ ] Create ADR: Service mesh adoption decision

### Scalability & Resilience

#### #46: Multi-Region Strategy
**Priority**: Medium
**Timeline**: Q2 2026
**Tasks**:
- [ ] Design: NATS geo-distribution (leafnodes)
- [ ] Design: K8s multi-cluster federation
- [ ] Design: DNS-based traffic routing
- [ ] Document: Data sovereignty considerations
- [ ] Document: Disaster recovery procedures
- [ ] POC: 2-region deployment

#### #47: Chaos Engineering
**Priority**: High
**Timeline**: Q2 2026
**Tasks**:
- [ ] Test: Pod random termination
- [ ] Test: Network latency injection (toxiproxy)
- [ ] Test: NATS broker restart under load
- [ ] Test: Database connection loss
- [ ] Test: Upstream timeout simulation
- [ ] Validate: Circuit breaker, retry, graceful degradation
- [ ] Document: Resilience test suite

---

## 🔮 Phase 5: Advanced Features (Future)

**Timeline**: Q3 2026+
**Status**: Planning

### Potential Features
- **Auto-scaling based on custom metrics** (HPA with Prometheus adapter)
- **Blue-green deployments** (Flagger + Istio)
- **A/B testing framework** (Traffic splitting)
- **Multi-tenancy** (Namespace isolation, resource quotas)
- **Cost optimization** (Spot instances, vertical pod autoscaling)
- **Advanced observability** (Distributed profiling, eBPF tracing)
- **ML-based anomaly detection** (Prometheus + custom models)

---

## 📊 Current Status Summary

### Completed
- **Phase 1**: Core infrastructure ✅
- **Phase 2**: Production readiness ✅ (22 tasks)

### In Progress
- **Phase 3**: Validation & testing ✅ (6 tasks, 6 done)

### Planned
- **Phase 4**: Enterprise features 📅 (5 tasks)
- **Phase 5**: Advanced features 💭 (Future)

### Task Breakdown
- ✅ **Completed**: 22 tasks
- 🔄 **In Progress**: 0 tasks (ready to start Phase 3)
- 📅 **Planned**: 10 tasks (Phase 3 + 4)
- 💭 **Future**: 7+ features (Phase 5)

---

## 🎯 Success Criteria

### Phase 3 (Validation)
- [ ] All integration tests passing with NATS
- [ ] Successful deployment to local K8s cluster
- [ ] Load test baseline established (RPS, latency p50/p95/p99)
- [ ] E2E tracing validated in Jaeger
- [ ] Property-based crypto tests passing

### Phase 4 (Enterprise)
- [ ] Security audit clean (no critical/high vulnerabilities)
- [ ] Chaos tests demonstrating 99.9% uptime
- [ ] Multi-region deployment documented
- [ ] Service mesh decision documented (ADR)

### Phase 5 (Advanced)
- [ ] Auto-scaling responding to traffic spikes
- [ ] Blue-green deployments automated
- [ ] Multi-tenant isolation validated
- [ ] Cost per request optimized

---

## 📚 Resources

### Documentation
- `PHASE_2_COMPLETE.md` - Phase 2 achievements
- `KUBERNETES.md` - Deployment guide
- `ADR_REFERENCE.md` - Architecture decisions
- `adr-ledger/docs/SPECTRE_ARCHITECTURE_DECISIONS.md` - Full ADR catalog

### Code Locations
- Core: `crates/spectre-{core,events,proxy,secrets,observability}/`
- Nix: `nix/kubernetes/`, `nix/services/nats/`, `flake.nix`
- Helm: `charts/spectre-proxy/`
- CI/CD: `.github/workflows/ci.yml`

### Quick Commands
```bash
# Development
nix develop                    # Enter dev shell
cargo build --release          # Build all crates
cargo test --workspace --lib   # Run unit tests

# Infrastructure (local dev)
nix run .#nats                 # Start NATS server (Nix-native)
docker-compose up -d           # Start Jaeger, Prometheus, etc.
docker-compose down            # Stop docker services

# Testing (Phase 3)
cargo test --test test_event_bus  # Integration tests (requires NATS)
./scripts/load-test.sh         # Load testing

# Container Images (Nix-only, no Docker build)
nix build .#spectre-proxy-image        # Build OCI image
docker load < result                   # Load to Docker daemon
skopeo copy docker-archive:result docker://registry/spectre:tag  # Push

# Deployment
nix build .#kubernetes-manifests-dev   # Generate manifests
nix run .#deploy-dev                   # Deploy to K8s
helm install spectre charts/spectre-proxy  # Or use Helm

# CI/CD
git push origin main           # Triggers 10-job pipeline (no Docker build)
```

---

## 🎓 Lessons Learned (Continuous)

### Phase 2 Key Insights
1. **Nix reproducibility** > Community size for infrastructure
2. **Circuit breakers first** - Fail-fast prevents cascades
3. **Build-time validation** - Catch errors before deployment
4. **SBOM automation** - Supply chain security from day 1

### Next Phase Focus
- **Integration testing is critical** - Unit tests alone insufficient
- **Real load testing matters** - Synthetic benchmarks miss edge cases
- **Observability debt compounds** - Add metrics/tracing early
- **Documentation is code** - ADRs prevent re-learning decisions

---

**Note**: This roadmap is living document. Tasks may be reprioritized based on production feedback and business needs.

Last reviewed: 2026-02-15
