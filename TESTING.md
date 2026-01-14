# SPECTRE Fleet - Testing Quick Reference

## 🚀 Run All Tests (Batch Execution)

```bash
# Automated test suite (recommended)
./scripts/run-tests.sh

# Expected duration: ~5 minutes
# Tests: 10 integration + unit tests + clippy + format check
```

## 📋 Test Checklist (Execute in Order)

### ✅ Phase 1: Infrastructure Setup (1 min)
```bash
# Start services
docker-compose up -d

# Verify NATS
curl http://localhost:8222/healthz

# Verify TimescaleDB
docker-compose exec timescaledb pg_isready -U spectre

# Verify Neo4j
docker-compose exec neo4j cypher-shell -u neo4j -p spectre_dev_password "RETURN 1"
```

### ✅ Phase 2: Build Check (2 min)
```bash
# Enter Nix shell
nix develop

# Build workspace
cargo build

# Check individual crates
cargo check -p spectre-core
cargo check -p spectre-events
```

### ✅ Phase 3: Unit Tests (1 min)
```bash
# Test spectre-core
cargo test -p spectre-core --lib

# Test spectre-events
cargo test -p spectre-events --lib
```

### ✅ Phase 4: Integration Tests (2 min)
```bash
# All integration tests
cargo test --test test_event_bus -- --test-threads=1 --nocapture

# Individual test
cargo test --test test_event_bus test_03_subscribe_and_receive -- --nocapture
```

### ✅ Phase 5: Code Quality (1 min)
```bash
# Clippy (linting)
cargo clippy --all-targets --all-features -- -D warnings

# Format check
cargo fmt -- --check
```

## 📊 Expected Results

### Test Counts
- **Unit tests**: ~20 tests
- **Integration tests**: 10 tests
- **Total**: ~30 tests

### Success Criteria
```
test result: ok. 30 passed; 0 failed; 0 ignored
```

### Performance Benchmarks
- Event publish latency: < 5ms
- Event throughput: > 50 events/sec (test_10)
- Queue load balancing: 10 events distributed across 2 workers

## 🐛 Troubleshooting

### Issue: "Connection refused (os error 111)"
**Cause**: NATS not running
**Fix**:
```bash
docker-compose up -d nats
sleep 5  # Wait for startup
cargo test --test test_event_bus
```

### Issue: "Test hangs indefinitely"
**Cause**: Deadlock or blocking operation
**Fix**:
```bash
# Run with timeout
cargo test -- --test-threads=1 --timeout=30
```

### Issue: "Address already in use"
**Cause**: Port 4222 occupied
**Fix**:
```bash
# Find process
sudo lsof -i :4222

# Stop docker-compose
docker-compose down
docker-compose up -d
```

### Issue: Tests fail on CI but pass locally
**Cause**: Race condition or timing issue
**Fix**:
```bash
# Run multiple times
for i in {1..5}; do cargo test --test test_event_bus; done

# Use single thread
cargo test -- --test-threads=1
```

## 📈 Test Output Example

```
═══════════════════════════════════════
  SPECTRE Fleet Test Suite
═══════════════════════════════════════

[INFO] Starting infrastructure...
[SUCCESS] NATS is ready
[SUCCESS] TimescaleDB is ready
[SUCCESS] Neo4j is ready

═══════════════════════════════════════
  PHASE 1: Unit Tests
═══════════════════════════════════════

Running unit tests for spectre-core...
test tests::test_service_id ... ok
test tests::test_correlation_id ... ok
test tests::test_trace_id ... ok
✅ spectre-core unit tests passed

Running unit tests for spectre-events...
test tests::test_event_creation ... ok
test tests::test_event_serialization ... ok
✅ spectre-events unit tests passed

═══════════════════════════════════════
  PHASE 2: Integration Tests
═══════════════════════════════════════

test test_01_connect_to_nats ... ok
test test_02_publish_event ... ok
test test_03_subscribe_and_receive ... ok
test test_04_request_reply ... ok
test test_05_queue_group_load_balancing ... ok
✅ Integration tests passed

═══════════════════════════════════════
  TEST SUMMARY
═══════════════════════════════════════

Total Tests:   5
Passed:        5
Failed:        0
Skipped:       0

🎉 All tests passed!
```

## 🔧 Advanced Testing

### Run with logging
```bash
RUST_LOG=debug cargo test -- --nocapture
```

### Run specific test pattern
```bash
cargo test subscribe -- --nocapture
```

### Run ignored tests (manual setup required)
```bash
cargo test -- --ignored
```

### Generate coverage report
```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate report
cargo tarpaulin --out Html --output-dir coverage
```

### Run benchmarks
```bash
RUN_BENCHMARKS=1 ./scripts/run-tests.sh
```

## 📝 Test Log Locations

```
/tmp/spectre-test-spectre-core.log
/tmp/spectre-test-spectre-events.log
/tmp/spectre-test-integration.log
/tmp/spectre-clippy.log
/tmp/spectre-fmt.log
```

## 🎯 Next Steps After Tests Pass

1. ✅ Tests pass → Phase 0 complete
2. → Start Phase 1: Security Infrastructure
   - Implement `spectre-proxy`
   - Implement `spectre-secrets`
3. → Continue Phase 2: Observability
   - Implement `spectre-observability`
   - Build Tauri dashboard

## 📞 Support

If tests fail consistently:
1. Check logs in `/tmp/spectre-test-*.log`
2. Verify infrastructure is running: `docker-compose ps`
3. Review test output for specific failure
4. Consult `tests/README.md` for detailed info

---

**Quick Command Reference:**
```bash
# One-line test execution
./scripts/run-tests.sh

# Keep infrastructure running
KEEP_INFRA=1 ./scripts/run-tests.sh

# Cleanup
docker-compose down && cargo clean
```

---

**Last Updated**: 2026-01-08
**Test Suite Version**: 0.1.0 (Phase 0)
