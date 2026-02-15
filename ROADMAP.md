# SPECTRE Roadmap

**Project**: SPECTRE Fleet - Enterprise-Grade AI Agent Framework
**Current Phase**: Phase 2 Complete → Phase 3 Starting
**Last Updated**: 2026-02-15

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
- [ ] Document: NATS failure scenarios

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
- [ ] Deploy NATS in-cluster for /ready probe
- [ ] Validate: Ingress routing with nginx-ingress controller

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

**Performance Baseline** (2026-02-15, debug build, localhost):
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
| VmSize | 156 MB |
| Thread count | 3 (tokio runtime) |

**Notes**:
- All measurements on debug build (release will be faster)
- Rate limiter correctly enforces 100 RPS per-IP with 200 burst
- Circuit breaker full lifecycle validated: closed → open (503) → half-open → closed (200)
- Proxy always forwards as POST to neutron (GET /agents → 405); future fix needed

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
**Priority**: High
**Tasks**:
- [ ] Dependency audit: `cargo audit`
- [ ] JWT validation edge cases
- [ ] RBAC bypass attempt testing
- [ ] Rate limiting bypass testing
- [ ] Secret exposure audit (logs, env)
- [ ] DoS resistance testing
- [ ] Generate security report

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
**Priority**: Medium
**Decision Point**: When inter-service communication grows
**Tasks**:
- [ ] Research: Istio vs Linkerd vs Cilium
- [ ] Document: Service mesh adoption criteria
- [ ] POC: Deploy proxy with Linkerd
- [ ] Test: mTLS between proxy ↔ neutron
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
- **Phase 3**: Validation & testing 🔄 (6 tasks, 5 done)

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
