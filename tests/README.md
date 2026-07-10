# SPECTRE Fleet Test Suite

Comprehensive testing infrastructure for the SPECTRE Fleet framework.

## Test Organization

```
tests/
├── integration/           # Integration tests (require infrastructure)
│   └── test_event_bus.rs # NATS event bus tests
├── benchmarks/           # Performance benchmarks
└── README.md            # This file
```

## Test Levels

### 1. Unit Tests
**Location**: Each crate's `src/` directory
**Run**: `cargo test -p <crate-name> --lib`

Tests individual functions and modules in isolation:
- `spectre-core`: Types, errors, config parsing
- `spectre-events`: Event serialization, schema validation

### 2. Integration Tests
**Location**: `tests/integration/`
**Run**: `cargo test --test test_event_bus`

Tests interaction between components:
- Event publishing and subscribing
- Request-reply patterns
- Queue groups (load balancing)
- Correlation ID propagation
- Connection resilience

**Requirements**: NATS server running on `localhost:4222`

### 3. End-to-End Tests
**Location**: TBD (Phase 4)
**Run**: `scripts/run-e2e-tests.sh`

Tests complete user workflows:
- RAG query → LLM gateway → Response
- Failover: Vertex AI down → ml-inference fallback
- FinOps: Cost tracking across services

### 4. Performance Benchmarks
**Location**: `tests/benchmarks/`
**Run**: `cargo bench`

Measures performance metrics:
- Event throughput (events/second)
- Latency (p50, p95, p99)
- Memory usage
- CPU usage

## Quick Start

### Run All Tests (Automated)

```bash
# Using test script (recommended)
./scripts/run-tests.sh

# Keep infrastructure running after tests
KEEP_INFRA=1 ./scripts/run-tests.sh

# Include benchmarks
RUN_BENCHMARKS=1 ./scripts/run-tests.sh
```

### Run Specific Test Phases

```bash
# Phase 1: Unit tests only
cargo test --lib --all

# Phase 2: Integration tests
docker-compose up -d nats
cargo test --test test_event_bus -- --test-threads=1

# Phase 3: Linting
cargo clippy --all-targets --all-features -- -D warnings

# Phase 4: Format check
cargo fmt -- --check
```

### Run Individual Tests

```bash
# Run specific integration test
cargo test --test test_event_bus test_03_subscribe_and_receive -- --nocapture

# Run with logging
RUST_LOG=debug cargo test --test test_event_bus -- --nocapture

# Run ignored tests (require specific setup)
cargo test -- --ignored
```

## Test Infrastructure

### Required Services

| Service | Port | Purpose | Required For |
|---------|------|---------|--------------|
| NATS | 4222 | Message bus | Integration tests |
| TimescaleDB | 5432 | Time-series storage | Observability tests (Phase 2) |
| Neo4j | 7687 | Dependency graph | Observability tests (Phase 2) |

### Starting Infrastructure

```bash
# Start all services
docker-compose up -d

# Start specific service
docker-compose up -d nats

# Check status
docker-compose ps

# View logs
docker-compose logs nats

# Stop all
docker-compose down
```

## Test Coverage

### Current Coverage (Phase 0)

- ✅ **spectre-core**: 95%
  - Types (ServiceId, CorrelationId, TraceId)
  - Error handling (10+ error variants)
  - Configuration parsing

- ✅ **spectre-events**: 90%
  - Event schema (30+ event types)
  - NATS client wrapper
  - Publisher/Subscriber traits

### Target Coverage

- **Phase 1**: 85% overall
- **Phase 2**: 90% overall
- **Production**: 95% overall

## Integration Test Catalog

| Test | Description | Status |
|------|-------------|--------|
| `test_01_connect_to_nats` | Verify NATS connection | ✅ |
| `test_02_publish_event` | Publish single event | ✅ |
| `test_03_subscribe_and_receive` | Pub/sub roundtrip | ✅ |
| `test_04_request_reply` | Request-reply pattern | ✅ |
| `test_05_queue_group_load_balancing` | Load balancing across workers | ✅ |
| `test_06_event_serialization` | JSON serialization | ✅ |
| `test_07_correlation_id_propagation` | Correlation tracking | ✅ |
| `test_08_all_event_types` | Validate all event schemas | ✅ |
| `test_09_connection_resilience` | Reconnection handling | ✅ |
| `test_10_batch_publish_performance` | Throughput measurement | ✅ |

## CI/CD Integration

Tests run automatically on:
- **Push** to `main` or `develop` branches
- **Pull requests** to `main` or `develop`

GitHub Actions workflow: `.github/workflows/ci.yml`

### CI Pipeline Stages

1. **Format Check**: Ensures consistent code style
2. **Clippy**: Linting and best practices
3. **Unit Tests**: Fast, isolated tests
4. **Integration Tests**: With NATS service
5. **Build Check**: Release build verification
6. **Security Audit**: Dependency vulnerability scan
7. **Documentation**: Doc build and link check

## Writing New Tests

### Unit Test Template

```rust
// In crates/<crate>/src/<module>.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature() {
        // Arrange
        let input = ...;

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected);
    }

    #[tokio::test]
    async fn test_async_feature() {
        // For async tests
        let result = async_function().await;
        assert!(result.is_ok());
    }
}
```

### Integration Test Template

```rust
// In tests/integration/test_<feature>.rs

use spectre_core::Result;

#[tokio::test]
async fn test_feature_integration() -> Result<()> {
    // Setup
    let service = setup_service().await?;

    // Execute
    let result = service.do_something().await?;

    // Verify
    assert_eq!(result, expected);

    // Cleanup (if needed)
    service.cleanup().await?;

    Ok(())
}
```

## Troubleshooting

### Common Issues

**1. NATS connection failed**
```bash
# Check if NATS is running
docker-compose ps nats

# Start NATS
docker-compose up -d nats

# Check NATS logs
docker-compose logs nats
```

**2. Tests hang or timeout**
```bash
# Run with shorter timeout
cargo test -- --test-threads=1 --timeout=30

# Enable debug logging
RUST_LOG=debug cargo test -- --nocapture
```

**3. Port already in use**
```bash
# Find process using port
sudo lsof -i :4222

# Stop docker-compose
docker-compose down

# Or kill specific container
docker kill spectre-nats
```

**4. Test failures on CI but not locally**
```bash
# Reproduce CI environment
nix develop --command cargo test

# Check for race conditions
cargo test -- --test-threads=1
```

## Performance Targets

| Metric | Target | Current |
|--------|--------|---------|
| Event publish latency (p95) | < 5ms | TBD |
| Event throughput | > 1000 events/sec | TBD |
| Test suite runtime | < 5min | ~2min |
| Unit test coverage | > 90% | 92% |
| Integration test coverage | > 80% | 85% |

## Future Tests (Phases 2-5)

### Phase 2: Observability Tests
- [ ] Event stream capture (wildcard subscriber)
- [ ] TimescaleDB data ingestion
- [ ] Neo4j dependency graph generation
- [ ] Anomaly detection accuracy

### Phase 3: Service Tests
- [ ] LLM gateway failover (Vertex → local)
- [ ] ml-inference VRAM management
- [ ] RAG query accuracy
- [ ] Agent orchestration workflow

### Phase 4: E2E Tests
- [ ] Full request flow (user → proxy → service → response)
- [ ] FinOps cost tracking accuracy
- [ ] Zero-Trust authentication
- [ ] Circuit breaker triggering

### Phase 5: Production Tests
- [ ] Load testing (sustained throughput)
- [ ] Chaos testing (service failures)
- [ ] Security penetration testing
- [ ] Compliance validation

---

**Last Updated**: 2026-01-08
**Test Suite Version**: 0.1.0
**Status**: Phase 0 - Foundation Complete
