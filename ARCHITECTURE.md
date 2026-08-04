# Architecture

## System Purpose

SPECTRE is the event mesh for the VoidNX Labs platform. It turns a set of polyglot services
into a coherent system by making the **event schema** the contract rather than the service
interface. Services are interchangeable behind a topic; the topic taxonomy is strict and
versioned so that schema drift cannot happen silently.

It also carries the platform's zero-trust proxy, secret rotation and observability plumbing.

## High-Level Overview

```
┌──────────────────────────────────────────────────────────┐
│ spectre-proxy      Axum gateway                          │
│   JWT + RBAC · rate limiting · circuit breaker · OTel    │
└─────────────────────────┬────────────────────────────────┘
                          ▼
┌──────────────────────────────────────────────────────────┐
│ NATS JetStream :4222                                     │
│   at-least-once delivery · replay                        │
│   llm.request.v1 · system.metrics.v1                     │
│   cost.incurred.v1 · governance.vote.v1                  │
└───┬──────────────────┬───────────────────┬───────────────┘
    ▼                  ▼                   ▼
spectre-events   spectre-observability  spectre-ai-reactor
 typed schemas    TimescaleDB (series)   reactive handlers
                  Neo4j (dep graph)
                          ▲
                 spectre-secrets
                 AES-GCM + Argon2id rotation
```

## Components

Cargo workspace, 6 crates under `crates/`:

| Crate | Responsibility |
|---|---|
| `spectre-core` | Shared domain types and traits |
| `spectre-events` | Typed event schemas and the topic taxonomy |
| `spectre-proxy` | Axum gateway: JWT, RBAC, rate limiting, circuit breaker, OpenTelemetry |
| `spectre-observability` | Metrics and trace pipeline; TimescaleDB and Neo4j sinks |
| `spectre-secrets` | AES-GCM encryption with Argon2id key derivation, rotation |
| `spectre-ai-reactor` | Reactive handlers for AI-driven events |

Supporting: `apps/`, `config/`, `charts/` (Kubernetes), `docker-compose.yml`.

## Data Flow

1. A producer publishes to a versioned subject — for example `llm.request.v1`.
2. JetStream persists it, guaranteeing at-least-once delivery and enabling replay.
3. Consumers subscribe by subject; `spectre-events` enforces the schema on both ends.
4. `spectre-observability` fans metrics into TimescaleDB for time-series and Neo4j for the
   service-dependency graph.
5. `spectre-proxy` mediates any external access, applying JWT authentication and RBAC.

## Trust Boundaries

| Boundary | Control |
|---|---|
| External → proxy | JWT, RBAC, rate limiting, circuit breaker |
| Producer → subject | typed schema in `spectre-events`; publication is rejected on mismatch |
| Service → secrets | AES-GCM + Argon2id; rotation handled by `spectre-secrets` |
| Mesh → observability | one-way; sinks cannot inject into the mesh |

## Runtime Model

Async Rust on Tokio. Each crate can run as an independent process; `docker-compose.yml`
composes them. JetStream provides durability, so a consumer restart replays rather than loses.

## Configuration

`config/`, per-crate. Subjects are declared in `spectre-events` and versioned in the name
(`.v1`), so a breaking change becomes `.v2` rather than a silent reinterpretation.

## Storage

| Store | Purpose |
|---|---|
| NATS JetStream | event log, replay |
| TimescaleDB | metrics time-series |
| Neo4j | service dependency graph |

## External Integrations

Consumed by `adr-ledger`, `neoland` and `sentinel`. Publishes governance and cost events used
for platform-wide accounting.

## Security Model

- JWT with RBAC at the proxy; no unauthenticated path into the mesh.
- Secrets encrypted with AES-GCM, keys derived with Argon2id, rotation built in.
- Circuit breaker prevents a failing consumer from amplifying load.
- OpenTelemetry traces carry no payload content by design.

## Testing Model

9 test files. `cargo test` across the workspace. `CHAOS_ENGINEERING.md` documents fault
injection. The flake exposes `packages`, `devShells`, `checks`, `apps` and `overlays`.

## Operational Notes

- Highest operability score in the ecosystem (84/100): health probes, container restart
  policy, structured logging, metrics endpoint, tracing and graceful shutdown are all present.
- `charts/` provides Kubernetes deployment.
- `ADR.md` and `ADR_REFERENCE.md` record architectural decisions in-repo.
- Build: `nix develop` then `cargo build --release`.

## Known Architectural Risks

1. **Release discipline scores 33/100** — 1 tag against 84 commits and 6 crates. The mesh
   other services depend on has essentially no release history, so consumers have no stable
   version to pin.
2. **Only 9 test files for a 6-crate workspace** carrying at-least-once delivery guarantees.
   Delivery semantics are the kind of property that wants property-based testing.
3. **No operational runbook**, despite having the best operational instrumentation in the
   ecosystem — the telemetry exists but the response procedure is not written down.
4. **No `SECURITY.md`** for the component that holds the secret-rotation implementation.
5. **Topic taxonomy is enforced in code, not in a schema registry.** A consumer built outside
   this workspace has nothing to validate against.
