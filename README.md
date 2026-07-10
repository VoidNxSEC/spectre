# SPECTRE Fleet

**AI Agent Framework with Event-Driven Architecture**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)](https://www.rust-lang.org)
[![Nix](https://img.shields.io/badge/nix-2.18+-blue.svg)](https://nixos.org)

---

## 🎯 Vision

SPECTRE is a **Domain-Driven Microservices** framework with:

- 🎯 **Event-Driven Architecture** - All services communicate via NATS message bus
- 🔒 **Zero-Trust Governance** - Mandatory authentication via Spectre Proxy
- 🏆 **Observability Intelligence** - ML-based anomaly detection, FinOps tracking
- 💰 **Hybrid Cloud + Local AI** - Vertex AI for complex tasks, local models for routine work
- 🛡️ **Resilience by Design** - Circuit breakers, automatic failover, degraded operation

**SPECTRE is a FLEET, not a MONOLITH**

---

## 🏗️ Architecture

### 🎯 Hybrid Architecture

**SPECTRE** is the **core infrastructure framework** that orchestrates independent domain services via event-driven communication.

**Repository Organization**:

- **This repository** (`/home/kernelcore/dev/low-level/spectre`): Core infrastructure only
- **Domain services**: Separate repositories in `~/dev/low-level/` (open source contributions)
- **Integration**: All services connect via NATS event bus

```
spectre/                          # Core Infrastructure (this repo)
├── crates/spectre-core           # Types, errors, config
├── crates/spectre-events         # NATS client & event schemas
├── crates/spectre-proxy          # Zero-Trust gateway
├── crates/spectre-secrets        # Secret rotation
└── crates/spectre-observability  # Intelligence & monitoring

~/dev/low-level/                  # Domain Services (separate repos)
├── ai-agent-os/                  # System monitoring
├── intelagent/                   # Agent orchestration
├── securellm-bridge/             # LLM proxy
├── ml-offload-api/               # ML inference
├── cognitive-vault/              # Credential manager
├── ragtex/                       # RAG system
└── arch-analyzer/                # NixOS analysis

          All communicate via NATS ↓
    [NATS Message Bus - localhost:4222]
```

### Core Infrastructure Crates (Phase 0-2)

1. **spectre-core** ✅ Phase 0
   - Common types: `ServiceId`, `CorrelationId`, `TraceId`
   - Error handling: `SpectreError` with context
   - Configuration: Unified TOML-based config
   - Logging: Structured logging with `tracing`

2. **spectre-events** ✅ Phase 0
   - NATS client wrapper
   - Event schema definitions (30+ event types)
   - Publisher/Subscriber abstractions
   - Request/Reply patterns

3. **spectre-proxy** 🚧 Phase 1
   - Zero-Trust API Gateway
   - TLS termination, rate limiting
   - Authentication via spectre-secrets

4. **spectre-secrets** 🚧 Phase 1
   - Secret storage & rotation
   - Integration with cognitive-vault crypto

5. **spectre-observability** 📅 Phase 2
   - Event stream processing (wildcard NATS subscriber)
   - TimescaleDB for time-series storage
   - Neo4j for dependency graphs
   - ML-based anomaly detection
   - FinOps dashboard

### Domain Services (Separate Repositories)

These services live in **separate repositories** and integrate with SPECTRE via NATS:

6. **ai-agent-os** → `~/dev/low-level/ai-agent-os/`
   - System monitoring (CPU, memory, disk, thermal)
   - Publishes: `system.metrics.v1`, `system.log.v1`

7. **intelagent** → `~/dev/low-level/intelagent/`
   - Agent orchestration with DAO governance
   - Publishes: `task.assigned.v1`, `governance.vote.v1`

8. **securellm-bridge** → `~/dev/low-level/securellm-bridge/`
   - Production LLM proxy with TLS, rate limiting
   - Publishes: `llm.request.v1`, `llm.response.v1`

9. **ml-offload-api** → `~/dev/low-level/ml-offload-api/`
   - ML inference with VRAM management
   - Publishes: `inference.request.v1`, `vram.status.v1`

10. **cognitive-vault** → `~/dev/low-level/cognitive-vault/`
    - Credential manager (Rust+Go)
    - Crypto primitives used by spectre-secrets

11. **ragtex** → `~/dev/low-level/ragtex/`
    - RAG system with Vertex AI + Chroma
    - Publishes: `rag.query.v1`, `document.indexed.v1`

12. **arch-analyzer** → `~/dev/low-level/arch-analyzer/`
    - NixOS architecture analysis
    - Publishes: `analysis.report.v1`

---

## 🚀 Quick Start

**Note**: This repository contains only the **core infrastructure**. Domain services live in separate repositories and integrate via NATS events.

### Prerequisites

- **Nix** with flakes enabled
- **Docker** and Docker Compose (for dev environment)

### Development Setup

```bash
# Clone this repository (core infrastructure)
cd /home/kernelcore/dev/low-level/spectre

# Enter Nix development shell
nix develop

# Start infrastructure (NATS, TimescaleDB, Neo4j)
docker-compose up -d

# Build core infrastructure crates
cargo build

# Run tests (validates event bus integration)
./scripts/run-tests.sh

# Or run tests manually
cargo test

# Check specific crate
cargo check -p spectre-core
cargo check -p spectre-events
```

### Integrating Domain Services

Domain services (e.g., `securellm-bridge`, `ml-offload-api`) integrate by:

1. **Adding SPECTRE dependencies** to their `Cargo.toml`:

   ```toml
   [dependencies]
   spectre-core = { git = "https://github.com/kernelcore/spectre", branch = "main" }
   spectre-events = { git = "https://github.com/kernelcore/spectre", branch = "main" }
   ```

2. **Connecting to NATS** and publishing/subscribing to events:

   ```rust
   use spectre_events::EventBus;

   let bus = EventBus::connect("nats://localhost:4222").await?;
   bus.subscribe("llm.request.v1").await?;
   ```

3. **See** `INTEGRATION.md` for detailed integration guide (created below)

### Environment Variables

```bash
# NATS
export NATS_URL=nats://localhost:4222

# Databases
export TIMESCALEDB_URL=postgresql://spectre:spectre_dev_password@localhost:5432/spectre_observability
export NEO4J_URI=neo4j://localhost:7687
export NEO4J_USER=neo4j
export NEO4J_PASSWORD=spectre_dev_password

# Logging
export RUST_LOG=debug
export RUST_BACKTRACE=1
```

---

## 📊 Project Status

### Phase 0: Foundation (Weeks 1-2) - **IN PROGRESS**

- [x] Monorepo structure (Cargo workspace, flake.nix)
- [x] Docker Compose (NATS, TimescaleDB, Neo4j)
- [x] spectre-core crate (types, errors, config, logging)
- [x] spectre-events crate (NATS client, event schemas)
- [ ] Integration tests (event pub/sub roundtrip)
- [ ] Validate dev environment

**Next**: Complete Phase 0 testing, then move to Phase 1 (Security Infrastructure)

---

## 🎯 Event Types

All events follow the pattern: `<category>.<action>.v<version>`

### Implemented Event Types

**LLM Gateway**:

- `llm.request.v1` / `llm.response.v1`

**ML Inference**:

- `inference.request.v1` / `inference.response.v1`
- `vram.status.v1`

**Analysis**:

- `analysis.request.v1` / `analysis.response.v1`
- `analysis.report.v1`

**RAG**:

- `rag.index.v1` / `rag.query.v1`
- `document.indexed.v1`

**System**:

- `system.metrics.v1` / `system.log.v1`
- `hyprland.window.v1` / `hyprland.workspace.v1`

**FinOps**:

- `cost.incurred.v1`

**Orchestration**:

- `task.assigned.v1` / `task.result.v1`

**Governance**:

- `governance.proposal.v1` / `governance.vote.v1`
- `quality.report.v1`

---

## 🧪 Example Usage

### Publishing an Event

```rust
use spectre_events::{EventBus, Event, EventType};
use spectre_core::ServiceId;

#[tokio::main]
async fn main() -> spectre_core::Result<()> {
    // Connect to NATS
    let bus = EventBus::connect("nats://localhost:4222").await?;

    // Create event
    let event = Event::new(
        EventType::SystemMetrics,
        ServiceId::new("agent-os"),
        serde_json::json!({
            "cpu_percent": 45.2,
            "memory_mb": 2048,
            "disk_gb": 128
        }),
    );

    // Publish
    bus.publish(&event).await?;

    println!("Event published: {}", event.event_id);
    Ok(())
}
```

### Subscribing to Events

```rust
use spectre_events::{EventBus, EventHandler, Subscriber, Event};

struct MyHandler;

#[async_trait::async_trait]
impl EventHandler for MyHandler {
    async fn handle(&self, event: Event) -> spectre_core::Result<()> {
        println!("Received event: {:?}", event);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> spectre_core::Result<()> {
    let bus = EventBus::connect("nats://localhost:4222").await?;
    let nats_sub = bus.subscribe("system.metrics.v1").await?;

    let mut subscriber = Subscriber::new(nats_sub, "system.metrics.v1");

    // This blocks and listens for events
    subscriber.listen(MyHandler).await?;

    Ok(())
}
```

---

## 📈 Roadmap

### Phase 1: Security Infrastructure (Weeks 3-4)

- spectre-proxy (Zero-Trust gateway)
- spectre-secrets (Secret rotation engine)

### Phase 2: Observability (Weeks 5-6)

- spectre-observability (Intelligence engine)
- Tauri dashboard (Real-time monitoring)

### Phase 3: Service Adaptation (Weeks 7-10)

- Migrate existing services to event-driven architecture
- Add NATS integration layers

### Phase 4: Integration & Testing (Weeks 11-12)

- End-to-end tests
- Performance benchmarks
- Failover testing

### Phase 5: Production Hardening (Weeks 13-14)

- NixOS module
- Prometheus exporters
- Security audit

---

## 📝 License

MIT License

---

## 🤝 Contributing

This is a personal professional framework project. Contributions are welcome after Phase 5 completion.

---

**Status**: 🚧 Phase 0 Foundation - Active Development
**Last Updated**: 2026-01-08
**Architects**: kernelcore + Claude Sonnet 4.5
