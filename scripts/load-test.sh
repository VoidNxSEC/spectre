#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────────────────────────
# SPECTRE Proxy Load Test — Full Stack (proxy ↔ neutron integration)
# Task #42: Production Load Test
#
# Prerequisites:
#   1. Neutron API running on :8000 (open mode, no API_SECRET_KEY)
#   2. NATS running on :4222        (nix run .#nats)
#   3. spectre-proxy running on :3000 with:
#        NEUTRON_URL=http://localhost:8000
#        NATS_URL=nats://localhost:4222
#        JWT_SECRET=dev-jwt-secret-spectre-2026
#        RATE_LIMIT_RPS=100.0
#        RATE_LIMIT_BURST=200
#        CIRCUIT_BREAKER_THRESHOLD=5
#        CIRCUIT_BREAKER_TIMEOUT_SECS=30
#   4. hey installed (nix shell nixpkgs#hey)
#
# Usage:
#   ./scripts/load-test.sh                     # Run all phases
#   ./scripts/load-test.sh --phase health      # Single phase
#   ./scripts/load-test.sh --phase proxy       # Proxy→Neutron only
#   ./scripts/load-test.sh --phase ratelimit   # Rate limiter burst
#   ./scripts/load-test.sh --phase circuit     # Circuit breaker (manual)
# ──────────────────────────────────────────────────────────────────────────────
set -euo pipefail

# ── Configuration ────────────────────────────────────────────────────────────
PROXY_URL="${PROXY_URL:-http://localhost:3000}"
NEUTRON_URL="${NEUTRON_URL:-http://localhost:8000}"
JWT_SECRET="${JWT_SECRET:-dev-jwt-secret-spectre-2026}"
PHASE="${1:-all}"  # all, --phase <name>

# Parse --phase flag
if [[ "$PHASE" == "--phase" ]]; then
  PHASE="${2:-all}"
fi

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

log()  { echo -e "${CYAN}[$(date +%H:%M:%S)]${NC} $*"; }
ok()   { echo -e "${GREEN}  ✓${NC} $*"; }
warn() { echo -e "${YELLOW}  ⚠${NC} $*"; }
fail() { echo -e "${RED}  ✗${NC} $*"; }
header() { echo -e "\n${BOLD}═══ $* ═══${NC}"; }

# ── JWT Token Generation ────────────────────────────────────────────────────
# Pure bash JWT (HS256) — no python dependency required
generate_jwt() {
  local role="${1:-service}"
  local sub="${2:-loadtest}"
  local exp=$(($(date +%s) + 3600))

  local header
  header=$(echo -n '{"alg":"HS256","typ":"JWT"}' | base64 -w0 | tr '+/' '-_' | tr -d '=')
  local payload
  payload=$(echo -n "{\"sub\":\"${sub}\",\"role\":\"${role}\",\"exp\":${exp}}" | base64 -w0 | tr '+/' '-_' | tr -d '=')
  local signature
  signature=$(echo -n "${header}.${payload}" | openssl dgst -sha256 -hmac "$JWT_SECRET" -binary | base64 -w0 | tr '+/' '-_' | tr -d '=')

  echo "${header}.${payload}.${signature}"
}

TOKEN=$(generate_jwt "service" "loadtest")
ADMIN_TOKEN=$(generate_jwt "admin" "loadtest-admin")

# ── Preflight Checks ────────────────────────────────────────────────────────
preflight() {
  header "Preflight Checks"

  # Check hey is available
  if ! command -v hey &>/dev/null; then
    fail "hey not found. Install with: nix shell nixpkgs#hey"
    exit 1
  fi
  ok "hey available"

  # Check proxy
  if curl -sf "$PROXY_URL/health" >/dev/null 2>&1; then
    ok "Proxy reachable at $PROXY_URL"
  else
    fail "Proxy unreachable at $PROXY_URL"
    echo "    Start with: NEUTRON_URL=http://localhost:8000 JWT_SECRET=dev-jwt-secret-spectre-2026 cargo run -p spectre-proxy"
    exit 1
  fi

  # Check neutron
  if curl -sf "$NEUTRON_URL/health" >/dev/null 2>&1; then
    ok "Neutron reachable at $NEUTRON_URL"
  else
    warn "Neutron unreachable at $NEUTRON_URL (proxy→neutron tests will show upstream errors)"
  fi

  # Check readiness (NATS + upstream)
  local ready
  ready=$(curl -sf "$PROXY_URL/ready" 2>/dev/null || echo '{"error":true}')
  if echo "$ready" | grep -q '"upstream":true'; then
    ok "Proxy → Neutron connectivity confirmed"
  else
    warn "Proxy readiness check: $ready"
  fi

  # Validate JWT works
  local auth_status
  auth_status=$(curl -sf -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer $TOKEN" \
    "$PROXY_URL/api/v1/ingest" -X POST \
    -H "Content-Type: application/json" \
    -d '{"event":"preflight"}' 2>/dev/null || echo "000")
  if [[ "$auth_status" == "200" ]]; then
    ok "JWT authentication working (HTTP $auth_status)"
  else
    fail "JWT auth returned HTTP $auth_status (expected 200)"
    exit 1
  fi

  echo ""
}

# ── Phase 1: Health Endpoint Baseline ────────────────────────────────────────
phase_health() {
  header "Phase 1: Health Endpoint Baseline (no auth, no upstream)"
  log "10,000 requests, 50 concurrent connections"
  echo ""

  hey -n 10000 -c 50 "$PROXY_URL/health"

  echo ""
  log "Metrics endpoint (10s, 10 connections)"
  echo ""
  hey -z 10s -c 10 "$PROXY_URL/metrics"
}

# ── Phase 2: Authenticated Ingest ────────────────────────────────────────────
phase_ingest() {
  header "Phase 2: Authenticated Ingest (auth + rate limit, no upstream)"
  log "5,000 requests, 30 concurrent connections"
  echo ""

  hey -n 5000 -c 30 \
    -m POST \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"event":"load_test","data":{"ts":"2026-02-15T00:00:00Z"}}' \
    "$PROXY_URL/api/v1/ingest"
}

# ── Phase 3: Proxy → Neutron ────────────────────────────────────────────────
phase_proxy() {
  header "Phase 3: Proxy → Neutron (full path: auth → rate limit → circuit breaker → HTTP forward)"

  # Note: proxy always forwards as POST. Neutron's /api/v1/agents is GET-only,
  # so this returns 405 from neutron. The proxy still exercises the full path.
  # We test both the agents endpoint and the execute endpoint.

  log "Testing proxy→neutron agents (1000 req, 20 conns)"
  log "Note: Expect non-200 if neutron only serves GET /agents (proxy forwards as POST)"
  echo ""

  hey -n 1000 -c 20 \
    -m GET \
    -H "Authorization: Bearer $TOKEN" \
    "$PROXY_URL/api/v1/neutron/agents"

  echo ""
  log "Testing proxy→neutron execute (500 req, 10 conns)"
  echo ""

  hey -n 500 -c 10 \
    -m POST \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"agent_id":"compliance_analyst","task_type":"analyze","input":{"query":"test"}}' \
    "$PROXY_URL/api/v1/neutron/agents/execute"
}

# ── Phase 4: Rate Limiter Burst Test ─────────────────────────────────────────
phase_ratelimit() {
  header "Phase 4: Rate Limiter Burst Test"
  log "Sending 300 simultaneous requests (burst limit = 200)"
  log "Expected: ~200 pass (2xx), ~100 rejected (429)"
  echo ""

  # Use hey with high concurrency to saturate the token bucket
  hey -n 300 -c 300 \
    -m POST \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"event":"burst_test"}' \
    "$PROXY_URL/api/v1/ingest" 2>&1 | tee /tmp/spectre-ratelimit.txt

  echo ""
  # Parse 429 count from hey output
  local total_429
  total_429=$(grep -oP '\[429\]\s+\K\d+' /tmp/spectre-ratelimit.txt 2>/dev/null || echo "0")
  local total_200
  total_200=$(grep -oP '\[200\]\s+\K\d+' /tmp/spectre-ratelimit.txt 2>/dev/null || echo "0")

  log "Results: 200 OK = $total_200 | 429 Rate Limited = $total_429"

  if [[ "$total_429" -gt 0 ]]; then
    ok "Rate limiter is working — rejected $total_429 requests"
  else
    warn "No 429 responses detected. Rate limit may be higher than burst size, or requests didn't arrive fast enough."
  fi

  # Wait for token bucket to refill before next phase
  log "Waiting 3s for rate limiter to refill..."
  sleep 3
}

# ── Phase 5: Circuit Breaker Validation ──────────────────────────────────────
phase_circuit() {
  header "Phase 5: Circuit Breaker Validation"
  echo ""
  log "This phase requires manual intervention:"
  echo ""
  echo "  1. Ensure neutron is running and proxy→neutron returns 200"
  echo "  2. While the test runs, kill neutron:  kill \$(pgrep -f uvicorn)"
  echo "  3. Watch proxy logs for:"
  echo "     - First 5 failures → 502 Bad Gateway (retries exhausted)"
  echo "     - After 5 failures → 503 Service Unavailable (circuit OPEN)"
  echo "  4. Restart neutron, wait 30s → circuit breaker recovers → 200"
  echo ""

  read -rp "Press Enter to start circuit breaker test (60s sustained load)... " </dev/tty || true

  log "Sending sustained traffic for 60s (10 concurrent connections)"
  echo ""

  hey -z 60s -c 10 \
    -m POST \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"agent_id":"compliance_analyst","task_type":"analyze","input":{}}' \
    "$PROXY_URL/api/v1/neutron/agents/execute" 2>&1 | tee /tmp/spectre-circuit.txt

  echo ""
  local count_502
  count_502=$(grep -oP '\[502\]\s+\K\d+' /tmp/spectre-circuit.txt 2>/dev/null || echo "0")
  local count_503
  count_503=$(grep -oP '\[503\]\s+\K\d+' /tmp/spectre-circuit.txt 2>/dev/null || echo "0")
  local count_200
  count_200=$(grep -oP '\[200\]\s+\K\d+' /tmp/spectre-circuit.txt 2>/dev/null || echo "0")

  log "Results: 200 OK = $count_200 | 502 Bad Gateway = $count_502 | 503 Circuit Open = $count_503"

  if [[ "$count_503" -gt 0 ]]; then
    ok "Circuit breaker triggered — $count_503 requests rejected with 503"
  else
    warn "No 503 responses. Did you kill neutron during the test?"
  fi
}

# ── Phase 6: Memory & Metrics Snapshot ───────────────────────────────────────
phase_profile() {
  header "Phase 6: Resource Profiling"

  # Memory usage
  local proxy_pid
  proxy_pid=$(pgrep -f "spectre-proxy" 2>/dev/null | head -1 || echo "")
  if [[ -n "$proxy_pid" ]]; then
    local vmrss
    vmrss=$(grep VmRSS "/proc/$proxy_pid/status" 2>/dev/null | awk '{print $2, $3}' || echo "N/A")
    local vmsize
    vmsize=$(grep VmSize "/proc/$proxy_pid/status" 2>/dev/null | awk '{print $2, $3}' || echo "N/A")
    local threads
    threads=$(grep Threads "/proc/$proxy_pid/status" 2>/dev/null | awk '{print $2}' || echo "N/A")

    log "spectre-proxy (PID $proxy_pid):"
    echo "    VmRSS (resident):  $vmrss"
    echo "    VmSize (virtual):  $vmsize"
    echo "    Threads:           $threads"
  else
    warn "spectre-proxy process not found (cannot read /proc stats)"
  fi

  echo ""

  # Prometheus metrics
  log "Prometheus metrics snapshot:"
  local metrics
  metrics=$(curl -sf "$PROXY_URL/metrics" 2>/dev/null || echo "")
  if [[ -n "$metrics" ]]; then
    echo "$metrics" | grep -E "^spectre_" || echo "    (no spectre_ prefixed metrics found)"
  else
    warn "Could not fetch metrics from $PROXY_URL/metrics"
  fi
}

# ── Summary ──────────────────────────────────────────────────────────────────
summary() {
  header "Load Test Complete"
  echo ""
  log "Review results above for:"
  echo "    - Requests/sec and latency (p50/p95/p99) from hey output"
  echo "    - Rate limiter accuracy (Phase 4)"
  echo "    - Circuit breaker behavior (Phase 5)"
  echo "    - Memory footprint (Phase 6)"
  echo ""
  log "Next steps:"
  echo "    - Record baseline in ROADMAP.md"
  echo "    - Check Jaeger traces: http://localhost:16686"
  echo "    - Generate flamegraph: cargo flamegraph -p spectre-proxy"
  echo ""
}

# ── Main ─────────────────────────────────────────────────────────────────────
main() {
  echo -e "${BOLD}"
  echo "╔══════════════════════════════════════════════════════════════╗"
  echo "║       SPECTRE Proxy Load Test — Task #42                   ║"
  echo "║       Proxy ↔ Neutron Integration                          ║"
  echo "╚══════════════════════════════════════════════════════════════╝"
  echo -e "${NC}"
  echo "  Proxy:   $PROXY_URL"
  echo "  Neutron: $NEUTRON_URL"
  echo "  JWT:     ${JWT_SECRET:0:8}..."
  echo ""

  preflight

  case "$PHASE" in
    all)
      phase_health
      phase_ingest
      phase_proxy
      phase_ratelimit
      phase_circuit
      phase_profile
      summary
      ;;
    health)    phase_health ;;
    ingest)    phase_ingest ;;
    proxy)     phase_proxy ;;
    ratelimit) phase_ratelimit ;;
    circuit)   phase_circuit ;;
    profile)   phase_profile ;;
    *)
      echo "Unknown phase: $PHASE"
      echo "Available: all, health, ingest, proxy, ratelimit, circuit, profile"
      exit 1
      ;;
  esac
}

main "$@"
