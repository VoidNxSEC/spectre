# Architecture Decision Records (ADR) Reference

## 📍 Location

All SPECTRE architectural decisions are documented in the centralized ADR Ledger:

**Repository**: [`adr-ledger`](https://github.com/marcosfpina/adr-ledger)
**Document**: [`docs/SPECTRE_ARCHITECTURE_DECISIONS.md`](https://github.com/marcosfpina/adr-ledger/blob/main/docs/SPECTRE_ARCHITECTURE_DECISIONS.md)

## 🎯 Formal ADRs

- **ADR-0037**: [Nix-First Kubernetes Orchestration over Helm](https://github.com/marcosfpina/adr-ledger/blob/main/adr/accepted/ADR-0037.md)
  - Status: ✅ Accepted
  - Classification: Critical
  - Date: 2026-02-15

- **ADR-0038**: [OTLP HTTP/protobuf over gRPC/tonic](https://github.com/marcosfpina/adr-ledger/blob/main/adr/accepted/ADR-0038.md)
  - Status: ✅ Accepted
  - Classification: Major
  - Date: 2026-02-15

- **ADR-0039**: [OTLP Trace Anomalies Investigation Backlog](https://github.com/marcosfpina/adr-ledger/blob/main/adr/proposed/ADR-0039.md)
  - Status: 📋 Proposed
  - Classification: Major
  - Date: 2026-02-15

- **ADR-0040**: [Service Mesh Adoption — Linkerd](https://github.com/marcosfpina/adr-ledger/blob/main/adr/accepted/ADR-0040.md)
  - Status: ✅ Accepted
  - Classification: Major
  - Date: 2026-02-16

- **ADR-0043**: [Phase 3→4 Transition — Stub Neutron to Production Backend](https://github.com/marcosfpina/adr-ledger/blob/main/adr/proposed/ADR-0043.md)
  - Status: 📋 Proposed
  - Classification: Major
  - Date: 2026-02-17

## 📋 Documented Decisions

The consolidated document includes 11 architectural decisions:

### Critical (4)
1. Nix-First Kubernetes Orchestration over Helm
2. Argon2id KDF for Secret Encryption (security fix)
3. Ingress + cert-manager Architecture
4. NATS JetStream Event-Driven Architecture

### Major (4)
5. Token Bucket Rate Limiting Strategy
6. Three-Tier RBAC Hierarchy (admin > service > readonly)
7. Prometheus + OTLP Observability Stack
8. Graceful Shutdown with SIGTERM/SIGINT Handling

### Implementation (3)
9. Shared HTTP Client with Connection Pooling
10. Health Check Endpoints (/health, /ready, /metrics)
11. Structured JSON Error Responses

## 🔗 Quick Links

- **ADR Ledger**: https://github.com/marcosfpina/adr-ledger
- **SPECTRE Decisions**: https://github.com/marcosfpina/adr-ledger/blob/main/docs/SPECTRE_ARCHITECTURE_DECISIONS.md
- **ADR-0037**: https://github.com/marcosfpina/adr-ledger/blob/main/adr/accepted/ADR-0037.md
- **ADR-0038**: https://github.com/marcosfpina/adr-ledger/blob/main/adr/accepted/ADR-0038.md
- **ADR-0040**: https://github.com/marcosfpina/adr-ledger/blob/main/adr/accepted/ADR-0040.md
- **ADR-0043**: https://github.com/marcosfpina/adr-ledger/blob/main/adr/proposed/ADR-0043.md

## 📖 Why Centralized?

Architectural decisions are maintained in the central `adr-ledger` repository for:

- **Cross-Project Visibility**: Decisions can be referenced by other projects (CEREBRO, PHANTOM, NEUTRON)
- **Governance**: Formal review and approval process
- **Traceability**: Complete audit trail of all architectural choices
- **Knowledge Management**: Centralized intelligence for the entire ecosystem

## 🚀 How to Propose New ADRs

For new SPECTRE architectural decisions:

1. Use the ADR MCP tools: `adr_new`, `adr_accept`
2. Or manually create in `adr-ledger/adr/proposed/`
3. Follow the ADR template structure
4. Submit for review and acceptance

---

*For implementation details and code, see SPECTRE's inline documentation and `KUBERNETES.md`.*
