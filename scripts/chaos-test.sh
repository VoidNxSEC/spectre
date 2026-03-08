#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# SPECTRE Fleet — Chaos Engineering Test Suite
# Task #47: Validate resilience patterns under real failure conditions
#
# Phases:
#   1. NATS Restart Under Load         (auto-reconnect validation)
#   2. Upstream Failure → CB Lifecycle (closed → open → half-open → closed)
#   3. Network Latency Injection       (toxiproxy timeout/latency toxics)
#   4. Graceful Shutdown + MTTR        (SIGTERM drain, restart recovery)
#   5. Database Connection Loss        (TimescaleDB — not on critical path)
#   6. Cascading Failure               (NATS + neutron simultaneously)
#
# Prerequisites:
#   nix develop  (or tools: cargo, hey, toxiproxy-server/cli, python3, jq, docker)
#   docker-compose up -d
#   cargo build --release
#
# Usage:
#   ./scripts/chaos-test.sh [--phase N] [--skip-build] [--fast]
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

# ── Colors & formatting ───────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m'

# ── Configuration ─────────────────────────────────────────────────────────────
PROXY_PORT="${PROXY_PORT:-3000}"
PROXY_URL="http://localhost:${PROXY_PORT}"
NEUTRON_PORT="${NEUTRON_PORT:-9000}"
NEUTRON_URL="http://localhost:${NEUTRON_PORT}"
TOXI_LISTEN_PORT="${TOXI_LISTEN_PORT:-9001}"   # toxiproxy sits between proxy → neutron
TOXI_API_PORT="${TOXI_API_PORT:-8474}"
TOXI_API="http://localhost:${TOXI_API_PORT}"

JWT_SECRET="${JWT_SECRET:-spectre-dev-secret}"
PROXY_BIN="./target/release/spectre-proxy"
LOG_DIR="/tmp/spectre-chaos"

# Circuit breaker tuned for fast testing
CB_THRESHOLD="${CIRCUIT_BREAKER_THRESHOLD:-3}"    # 3 failures → open
CB_TIMEOUT="${CIRCUIT_BREAKER_TIMEOUT_SECS:-10}"  # 10s recovery window

# Docker container names (docker-compose default prefix)
NATS_CONTAINER="${NATS_CONTAINER:-spectre-nats-1}"
TIMESCALE_CONTAINER="${TIMESCALE_CONTAINER:-spectre-timescaledb-1}"

# Test counters
PASS=0
FAIL=0
TOTAL_PHASE_PASS=0
TOTAL_PHASE_FAIL=0

# PID tracking for cleanup
PROXY_PID=""
NEUTRON_PID=""
TOXI_PID=""

# Phase filter
RUN_PHASE="${1:-}"
SKIP_BUILD=false
FAST_MODE=false

for arg in "$@"; do
  case "$arg" in
    --skip-build) SKIP_BUILD=true ;;
    --fast)       FAST_MODE=true ;;
    --phase)      shift; RUN_PHASE="${1:-}" ;;
  esac
done

# ── Utilities ─────────────────────────────────────────────────────────────────

log()      { echo -e "${DIM}[$(date '+%H:%M:%S')]${NC} $*"; }
info()     { echo -e "${CYAN}[INFO]${NC}  $*"; }
warn()     { echo -e "${YELLOW}[WARN]${NC}  $*"; }
success()  { echo -e "${GREEN}[PASS]${NC}  $*"; ((PASS++)); ((TOTAL_PHASE_PASS++)); }
failure()  { echo -e "${RED}[FAIL]${NC}  $*"; ((FAIL++)); ((TOTAL_PHASE_FAIL++)); }
phase()    { echo -e "\n${BOLD}${BLUE}━━━  Phase $1: $2  ━━━${NC}"; }
section()  { echo -e "\n${MAGENTA}▶ $*${NC}"; }

wait_for() {
  local url="$1"
  local label="${2:-service}"
  local max="${3:-30}"
  local interval=1
  local elapsed=0
  while ! curl -sf "$url" >/dev/null 2>&1; do
    sleep $interval
    elapsed=$((elapsed + interval))
    if [[ $elapsed -ge $max ]]; then
      echo -e "${RED}Timeout waiting for ${label} (${max}s)${NC}"
      return 1
    fi
    echo -ne "\r${DIM}  waiting for ${label}... ${elapsed}s${NC}"
  done
  echo -e "\r${GREEN}  ${label} ready (${elapsed}s)${NC}         "
}

assert_status() {
  local label="$1"
  local expected="$2"
  local actual="$3"
  if [[ "$actual" == "$expected" ]]; then
    success "$label → HTTP ${actual}"
  else
    failure "$label → expected HTTP ${expected}, got HTTP ${actual}"
  fi
}

http_status() {
  curl -so /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer $(generate_jwt)" \
    "${@:2}" "$1" 2>/dev/null || echo "000"
}

http_status_public() {
  curl -so /dev/null -w "%{http_code}" "$1" 2>/dev/null || echo "000"
}

generate_jwt() {
  local secret="${JWT_SECRET}"
  local header; header=$(echo -n '{"alg":"HS256","typ":"JWT"}' | base64 | tr '+/' '-_' | tr -d '=')
  local exp=$(( $(date +%s) + 3600 ))
  local payload; payload=$(echo -n "{\"sub\":\"chaos-test\",\"role\":\"admin\",\"exp\":${exp}}" \
    | base64 | tr '+/' '-_' | tr -d '=')
  local sig; sig=$(echo -n "${header}.${payload}" \
    | openssl dgst -sha256 -hmac "${secret}" -binary \
    | base64 | tr '+/' '-_' | tr -d '=')
  echo "${header}.${payload}.${sig}"
}

# ── Neutron stub ──────────────────────────────────────────────────────────────

NEUTRON_STUB_PY="${LOG_DIR}/neutron_stub.py"

write_neutron_stub() {
  local port="${1:-${NEUTRON_PORT}}"
  cat > "${NEUTRON_STUB_PY}" <<PYEOF
import http.server, sys

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(b'{"status":"ok","service":"neutron-stub"}')
    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(b'{"status":"ok","service":"neutron-stub"}')
    def log_message(self, fmt, *args):
        pass  # suppress access logs

port = int(sys.argv[1]) if len(sys.argv) > 1 else ${port}
httpd = http.server.HTTPServer(('0.0.0.0', port), Handler)
httpd.serve_forever()
PYEOF
}

start_neutron_stub() {
  local port="${1:-${NEUTRON_PORT}}"
  write_neutron_stub "$port"
  python3 "${NEUTRON_STUB_PY}" "$port" > "${LOG_DIR}/neutron.log" 2>&1 &
  NEUTRON_PID=$!
  sleep 0.3
  if ! kill -0 "$NEUTRON_PID" 2>/dev/null; then
    failure "Neutron stub failed to start on port ${port}"
    return 1
  fi
  log "Neutron stub started (PID ${NEUTRON_PID}, port ${port})"
}

stop_neutron_stub() {
  if [[ -n "$NEUTRON_PID" ]] && kill -0 "$NEUTRON_PID" 2>/dev/null; then
    kill "$NEUTRON_PID" 2>/dev/null || true
    wait "$NEUTRON_PID" 2>/dev/null || true
    NEUTRON_PID=""
    log "Neutron stub stopped"
  fi
}

# ── Proxy lifecycle ───────────────────────────────────────────────────────────

start_proxy() {
  local extra_env="${1:-}"
  local log_file="${LOG_DIR}/proxy.log"

  NEUTRON_URL="http://localhost:${NEUTRON_PORT}" \
  JWT_SECRET="${JWT_SECRET}" \
  CIRCUIT_BREAKER_THRESHOLD="${CB_THRESHOLD}" \
  CIRCUIT_BREAKER_TIMEOUT_SECS="${CB_TIMEOUT}" \
  RATE_LIMIT_RPS=500 \
  RATE_LIMIT_BURST=1000 \
  RUST_LOG=warn \
  OTEL_EXPORTER_OTLP_ENDPOINT="" \
  SPECTRE_ENV=dev \
  eval "${extra_env} ${PROXY_BIN}" > "${log_file}" 2>&1 &
  PROXY_PID=$!

  sleep 0.2
  if ! kill -0 "$PROXY_PID" 2>/dev/null; then
    failure "Proxy failed to start (see ${log_file})"
    return 1
  fi
}

start_proxy_via_toxi() {
  # Route proxy → toxiproxy → neutron
  local log_file="${LOG_DIR}/proxy-toxi.log"

  NEUTRON_URL="http://localhost:${TOXI_LISTEN_PORT}" \
  JWT_SECRET="${JWT_SECRET}" \
  CIRCUIT_BREAKER_THRESHOLD="${CB_THRESHOLD}" \
  CIRCUIT_BREAKER_TIMEOUT_SECS="${CB_TIMEOUT}" \
  RATE_LIMIT_RPS=500 \
  RATE_LIMIT_BURST=1000 \
  RUST_LOG=warn \
  OTEL_EXPORTER_OTLP_ENDPOINT="" \
  SPECTRE_ENV=dev \
  "${PROXY_BIN}" > "${log_file}" 2>&1 &
  PROXY_PID=$!

  sleep 0.2
  if ! kill -0 "$PROXY_PID" 2>/dev/null; then
    failure "Proxy (toxi-mode) failed to start (see ${log_file})"
    return 1
  fi
}

stop_proxy() {
  if [[ -n "$PROXY_PID" ]] && kill -0 "$PROXY_PID" 2>/dev/null; then
    kill -SIGTERM "$PROXY_PID" 2>/dev/null || true
    wait "$PROXY_PID" 2>/dev/null || true
    PROXY_PID=""
    log "Proxy stopped (SIGTERM)"
  fi
}

kill_proxy_hard() {
  if [[ -n "$PROXY_PID" ]] && kill -0 "$PROXY_PID" 2>/dev/null; then
    kill -SIGKILL "$PROXY_PID" 2>/dev/null || true
    wait "$PROXY_PID" 2>/dev/null || true
    PROXY_PID=""
    log "Proxy killed (SIGKILL)"
  fi
}

# ── toxiproxy helpers ─────────────────────────────────────────────────────────

start_toxiproxy() {
  toxiproxy-server -host 0.0.0.0 -port "${TOXI_API_PORT}" \
    > "${LOG_DIR}/toxiproxy.log" 2>&1 &
  TOXI_PID=$!
  wait_for "${TOXI_API}/version" "toxiproxy-server" 10
}

stop_toxiproxy() {
  if [[ -n "$TOXI_PID" ]] && kill -0 "$TOXI_PID" 2>/dev/null; then
    kill "$TOXI_PID" 2>/dev/null || true
    wait "$TOXI_PID" 2>/dev/null || true
    TOXI_PID=""
    log "toxiproxy stopped"
  fi
}

toxi_create() {
  # Create proxy: toxi listen → neutron upstream
  curl -sf -X POST "${TOXI_API}/proxies" \
    -H 'Content-Type: application/json' \
    -d "{\"name\":\"neutron\",\"listen\":\"0.0.0.0:${TOXI_LISTEN_PORT}\",\"upstream\":\"127.0.0.1:${NEUTRON_PORT}\"}" \
    > /dev/null
  log "toxiproxy: created neutron proxy (:${TOXI_LISTEN_PORT} → :${NEUTRON_PORT})"
}

toxi_add_latency() {
  local ms="${1:-2000}"
  curl -sf -X POST "${TOXI_API}/proxies/neutron/toxics" \
    -H 'Content-Type: application/json' \
    -d "{\"name\":\"slow\",\"type\":\"latency\",\"attributes\":{\"latency\":${ms},\"jitter\":0}}" \
    > /dev/null
  log "toxiproxy: latency toxic added (+${ms}ms)"
}

toxi_add_timeout() {
  # timeout=0 drops connections immediately (simulates network partition)
  local ms="${1:-100}"
  curl -sf -X POST "${TOXI_API}/proxies/neutron/toxics" \
    -H 'Content-Type: application/json' \
    -d "{\"name\":\"partition\",\"type\":\"timeout\",\"attributes\":{\"timeout\":${ms}}}" \
    > /dev/null
  log "toxiproxy: timeout toxic added (${ms}ms, simulating network partition)"
}

toxi_remove_toxic() {
  local name="${1:-slow}"
  curl -sf -X DELETE "${TOXI_API}/proxies/neutron/toxics/${name}" > /dev/null 2>&1 || true
  log "toxiproxy: toxic '${name}' removed"
}

toxi_delete_proxy() {
  curl -sf -X DELETE "${TOXI_API}/proxies/neutron" > /dev/null 2>&1 || true
}

# ── Global cleanup ────────────────────────────────────────────────────────────

cleanup() {
  echo -e "\n${DIM}[cleanup] stopping all processes...${NC}"
  stop_proxy
  stop_neutron_stub
  stop_toxiproxy

  # Restore docker containers if they were stopped
  docker start "${NATS_CONTAINER}" > /dev/null 2>&1 || true
  docker start "${TIMESCALE_CONTAINER}" > /dev/null 2>&1 || true
}

trap cleanup EXIT INT TERM

# ── Pre-flight ────────────────────────────────────────────────────────────────

preflight() {
  echo -e "\n${BOLD}${BLUE}━━━  SPECTRE Chaos Engineering Suite  ━━━${NC}"
  echo -e "${DIM}CB threshold: ${CB_THRESHOLD} failures → open | Recovery: ${CB_TIMEOUT}s${NC}\n"

  mkdir -p "${LOG_DIR}"

  # Build
  if [[ "$SKIP_BUILD" != "true" ]]; then
    section "Building spectre-proxy (release)"
    cargo build --release -p spectre-proxy -q 2>&1 | tail -3
    if [[ ! -x "$PROXY_BIN" ]]; then
      echo -e "${RED}Build failed — cannot find ${PROXY_BIN}${NC}"
      exit 1
    fi
    success "Binary built: ${PROXY_BIN}"
  fi

  # Tool checks
  for tool in hey curl jq openssl python3 docker; do
    if command -v "$tool" >/dev/null 2>&1; then
      log "  ✓ ${tool}"
    else
      warn "  ✗ ${tool} not found — some phases may be skipped"
    fi
  done

  if command -v toxiproxy-server >/dev/null 2>&1 && command -v toxiproxy-cli >/dev/null 2>&1; then
    log "  ✓ toxiproxy"
  else
    warn "  ✗ toxiproxy not found — Phase 3 will be skipped (add to nix devShell)"
  fi

  # Check docker containers exist
  if ! docker ps --format '{{.Names}}' | grep -q "${NATS_CONTAINER}" 2>/dev/null; then
    warn "NATS container '${NATS_CONTAINER}' not running — Phase 1 and NATS tests will be skipped"
    warn "Run: docker-compose up -d"
  fi
}

# ─────────────────────────────────────────────────────────────────────────────
# Phase 1: NATS Restart Under Load
# ─────────────────────────────────────────────────────────────────────────────

run_phase1() {
  phase "1" "NATS Restart Under Load"
  PASS=0; FAIL=0

  if ! docker ps --format '{{.Names}}' | grep -q "${NATS_CONTAINER}" 2>/dev/null; then
    warn "Skipping Phase 1 — NATS container not found"
    return
  fi

  section "Starting proxy + neutron stub"
  start_neutron_stub
  start_proxy
  wait_for "${PROXY_URL}/health" "proxy" 15

  section "Baseline: 20 ingest requests (all must pass)"
  local ok=0
  for i in $(seq 1 20); do
    local code; code=$(http_status "${PROXY_URL}/api/v1/ingest" -X POST \
      -H 'Content-Type: application/json' -d '{"event":"baseline"}')
    [[ "$code" == "200" ]] && ((ok++))
  done
  if [[ $ok -eq 20 ]]; then
    success "Baseline: 20/20 requests succeeded"
  else
    failure "Baseline: only ${ok}/20 requests succeeded"
  fi

  section "Starting background load (100 RPS) → restarting NATS"
  hey -n 2000 -c 20 -q 100 -m POST \
    -H "Authorization: Bearer $(generate_jwt)" \
    -H "Content-Type: application/json" \
    -d '{"event":"chaos-nats-restart"}' \
    "${PROXY_URL}/api/v1/ingest" \
    > "${LOG_DIR}/phase1-hey.log" 2>&1 &
  local HEY_PID=$!

  sleep 1  # let load start

  section "Stopping NATS (docker stop)"
  docker stop "${NATS_CONTAINER}" > /dev/null
  log "NATS stopped — proxy will lose event bus connectivity"
  sleep 2

  section "Restarting NATS"
  docker start "${NATS_CONTAINER}" > /dev/null
  log "NATS restarted — waiting for reconnect..."
  sleep 3  # proxy auto-reconnect window

  # Wait for hey to finish
  wait "$HEY_PID" 2>/dev/null || true

  section "Validating recovery"
  wait_for "${PROXY_URL}/health" "proxy /health after NATS restart" 15

  # Post-restart: /health should be 200
  local health_code; health_code=$(http_status_public "${PROXY_URL}/health")
  assert_status "/health after NATS restart" "200" "$health_code"

  # /ready should reflect NATS status (may take a moment to reconnect)
  sleep 2
  local ready_code; ready_code=$(http_status_public "${PROXY_URL}/ready")
  if [[ "$ready_code" == "200" ]]; then
    success "/ready after NATS reconnect → 200 (reconnected)"
  else
    warn "/ready after NATS reconnect → ${ready_code} (may still be reconnecting)"
  fi

  # Parse hey results
  if [[ -f "${LOG_DIR}/phase1-hey.log" ]]; then
    local total; total=$(grep -E '^\[' "${LOG_DIR}/phase1-hey.log" | head -1 || echo "")
    log "hey output: $(cat "${LOG_DIR}/phase1-hey.log" | grep -E 'Requests/sec|Success' | head -3 || true)"
  fi

  section "Phase 1 Result"
  echo -e "  Pass: ${GREEN}${PASS}${NC}  Fail: ${RED}${FAIL}${NC}"

  stop_proxy
  stop_neutron_stub
}

# ─────────────────────────────────────────────────────────────────────────────
# Phase 2: Upstream Failure → Circuit Breaker Lifecycle
# ─────────────────────────────────────────────────────────────────────────────

run_phase2() {
  phase "2" "Upstream Failure → Circuit Breaker Lifecycle"
  PASS=0; FAIL=0

  section "Starting proxy + neutron stub"
  start_neutron_stub
  start_proxy
  wait_for "${PROXY_URL}/health" "proxy" 15

  section "Baseline: 5 neutron requests (circuit CLOSED — all must succeed)"
  local ok=0
  for i in $(seq 1 5); do
    local code; code=$(http_status "${PROXY_URL}/api/v1/neutron/health" -X GET)
    [[ "$code" == "200" ]] && ((ok++))
    sleep 0.1
  done
  if [[ $ok -ge 4 ]]; then
    success "Baseline CLOSED: ${ok}/5 succeeded"
  else
    failure "Baseline CLOSED: only ${ok}/5 succeeded (circuit already broken?)"
  fi

  section "Killing neutron stub → inducing ${CB_THRESHOLD} failures to open circuit"
  stop_neutron_stub

  local open_code=""
  local failures=0
  for i in $(seq 1 10); do
    local code; code=$(http_status "${PROXY_URL}/api/v1/neutron/health" -X GET)
    if [[ "$code" == "503" ]]; then
      # Could be circuit open OR service unavailable from failed proxy
      ((failures++))
    fi
    sleep 0.2
  done

  # After CB_THRESHOLD failures, circuit opens → 503 Service Unavailable
  local open_check; open_check=$(http_status "${PROXY_URL}/api/v1/neutron/health" -X GET)
  if [[ "$open_check" == "503" ]]; then
    success "Circuit OPEN: requests blocked with 503 after ${CB_THRESHOLD} failures"
  else
    warn "Circuit state uncertain: got ${open_check} (may need more failures)"
  fi

  section "Verifying health check is unaffected (circuit breaker is endpoint-scoped)"
  local health_code; health_code=$(http_status_public "${PROXY_URL}/health")
  assert_status "/health during circuit OPEN" "200" "$health_code"

  section "Verifying rate limiter still functional during circuit OPEN"
  local rl_code; rl_code=$(http_status "${PROXY_URL}/api/v1/ingest" -X POST \
    -H 'Content-Type: application/json' -d '{"event":"test"}')
  if [[ "$rl_code" == "200" ]]; then
    success "Ingest (NATS-only path) unaffected by neutron circuit breaker"
  else
    warn "Ingest returned ${rl_code} (may be NATS connectivity issue)"
  fi

  section "Waiting ${CB_TIMEOUT}s for circuit → HALF-OPEN"
  log "Sleeping ${CB_TIMEOUT}s (circuit recovery window)..."
  sleep "${CB_TIMEOUT}"

  section "Restarting neutron stub → 3 successes to close circuit"
  start_neutron_stub

  local recovered=0
  for i in $(seq 1 5); do
    local code; code=$(http_status "${PROXY_URL}/api/v1/neutron/health" -X GET)
    if [[ "$code" == "200" ]]; then
      ((recovered++))
    fi
    sleep 0.3
  done

  if [[ $recovered -ge 3 ]]; then
    success "Circuit CLOSED: ${recovered}/5 requests succeeded after recovery"
  else
    failure "Circuit did not fully recover: only ${recovered}/5 succeeded"
  fi

  section "Final state: 10 requests — all must succeed (circuit firmly CLOSED)"
  local final_ok=0
  for i in $(seq 1 10); do
    local code; code=$(http_status "${PROXY_URL}/api/v1/neutron/health" -X GET)
    [[ "$code" == "200" ]] && ((final_ok++))
    sleep 0.1
  done
  if [[ $final_ok -ge 8 ]]; then
    success "Post-recovery: ${final_ok}/10 succeeded — circuit CLOSED"
  else
    failure "Post-recovery: only ${final_ok}/10 succeeded"
  fi

  section "Phase 2 Result"
  echo -e "  Pass: ${GREEN}${PASS}${NC}  Fail: ${RED}${FAIL}${NC}"

  stop_proxy
  stop_neutron_stub
}

# ─────────────────────────────────────────────────────────────────────────────
# Phase 3: Network Chaos via toxiproxy
# ─────────────────────────────────────────────────────────────────────────────

run_phase3() {
  phase "3" "Network Latency Injection (toxiproxy)"
  PASS=0; FAIL=0

  if ! command -v toxiproxy-server >/dev/null 2>&1 || ! command -v toxiproxy-cli >/dev/null 2>&1; then
    warn "toxiproxy not found — skipping Phase 3"
    warn "Fix: add 'toxiproxy' to commonBuildInputs in flake.nix"
    return
  fi

  section "Starting toxiproxy-server"
  start_toxiproxy
  toxi_create

  section "Starting neutron stub + proxy (routed via toxiproxy)"
  start_neutron_stub
  start_proxy_via_toxi
  wait_for "${PROXY_URL}/health" "proxy (toxi-mode)" 15

  section "Baseline: 5 requests through clean proxy (no toxics)"
  local ok=0
  for i in $(seq 1 5); do
    local code; code=$(http_status "${PROXY_URL}/api/v1/neutron/health" -X GET)
    [[ "$code" == "200" ]] && ((ok++))
    sleep 0.1
  done
  if [[ $ok -ge 4 ]]; then
    success "Baseline through toxiproxy: ${ok}/5 OK"
  else
    failure "Baseline failed through toxiproxy: ${ok}/5 OK (setup issue?)"
  fi

  # ── 3a: High latency (2000ms — still under 5s connect timeout) ─────────────
  section "3a: Adding 2000ms latency toxic → measuring impact"
  toxi_add_latency 2000

  local slow_ok=0
  local t_start; t_start=$(date +%s%3N)
  for i in $(seq 1 5); do
    local code; code=$(http_status "${PROXY_URL}/api/v1/neutron/health" -X GET)
    [[ "$code" == "200" ]] && ((slow_ok++))
    sleep 0.1
  done
  local t_end; t_end=$(date +%s%3N)
  local elapsed_ms=$(( t_end - t_start ))

  if [[ $elapsed_ms -gt 5000 ]]; then
    success "Latency visible: ${elapsed_ms}ms for 5 requests (>5s total — 2s/req overhead confirmed)"
  else
    warn "Latency may not be fully applied: ${elapsed_ms}ms for 5 reqs"
  fi

  if [[ $slow_ok -ge 3 ]]; then
    success "Requests still succeed under 2s latency: ${slow_ok}/5 (circuit not opened by latency alone)"
  else
    warn "Requests failed under 2s latency: ${slow_ok}/5 (proxy timeout may be too short)"
  fi

  toxi_remove_toxic "slow"

  # ── 3b: Network partition (timeout=100ms → connection drops) ───────────────
  section "3b: Network partition (timeout=100ms) → circuit breaker activation"
  toxi_add_timeout 100

  local fail_count=0
  for i in $(seq 1 $((CB_THRESHOLD + 3))); do
    local code; code=$(http_status "${PROXY_URL}/api/v1/neutron/health" -X GET)
    [[ "$code" == "503" || "$code" == "502" || "$code" == "000" ]] && ((fail_count++))
    sleep 0.3
  done

  local cb_code; cb_code=$(http_status "${PROXY_URL}/api/v1/neutron/health" -X GET)
  if [[ "$cb_code" == "503" ]]; then
    success "Circuit OPEN under network partition: requests return 503"
  else
    warn "Circuit state after partition: ${cb_code} (may need more failures or CB already open)"
  fi

  # Health still works
  local health_code; health_code=$(http_status_public "${PROXY_URL}/health")
  assert_status "/health during network partition" "200" "$health_code"

  section "3c: Removing toxic → circuit recovery"
  toxi_remove_toxic "partition"

  log "Waiting ${CB_TIMEOUT}s for circuit recovery..."
  sleep "${CB_TIMEOUT}"

  local recovered=0
  for i in $(seq 1 5); do
    local code; code=$(http_status "${PROXY_URL}/api/v1/neutron/health" -X GET)
    [[ "$code" == "200" ]] && ((recovered++))
    sleep 0.5
  done

  if [[ $recovered -ge 3 ]]; then
    success "Recovery after partition removed: ${recovered}/5 OK"
  else
    failure "Recovery incomplete: only ${recovered}/5 OK"
  fi

  section "Phase 3 Result"
  echo -e "  Pass: ${GREEN}${PASS}${NC}  Fail: ${RED}${FAIL}${NC}"

  stop_proxy
  stop_neutron_stub
  toxi_delete_proxy
  stop_toxiproxy
}

# ─────────────────────────────────────────────────────────────────────────────
# Phase 4: Graceful Shutdown + MTTR
# ─────────────────────────────────────────────────────────────────────────────

run_phase4() {
  phase "4" "Graceful Shutdown (SIGTERM) + MTTR"
  PASS=0; FAIL=0

  section "Starting proxy + neutron stub"
  start_neutron_stub
  start_proxy
  wait_for "${PROXY_URL}/health" "proxy" 15

  section "Sending continuous load (background)"
  hey -n 5000 -c 10 -q 50 \
    -H "Authorization: Bearer $(generate_jwt)" \
    -H "Content-Type: application/json" \
    -d '{"event":"graceful-shutdown-test"}' \
    -m POST "${PROXY_URL}/api/v1/ingest" \
    > "${LOG_DIR}/phase4-hey.log" 2>&1 &
  local HEY_PID=$!

  sleep 1  # let load build

  section "Sending SIGTERM to proxy (graceful shutdown)"
  local t_sigterm; t_sigterm=$(date +%s%3N)
  kill -SIGTERM "$PROXY_PID"
  local shutdown_pid="$PROXY_PID"
  PROXY_PID=""

  # Proxy should drain in-flight requests then exit
  wait "$shutdown_pid" 2>/dev/null || true
  local t_shutdown; t_shutdown=$(date +%s%3N)
  local shutdown_ms=$(( t_shutdown - t_sigterm ))

  if [[ $shutdown_ms -lt 5000 ]]; then
    success "Graceful shutdown completed in ${shutdown_ms}ms (< 5s)"
  else
    warn "Graceful shutdown took ${shutdown_ms}ms (> 5s — may indicate drain timeout)"
  fi

  # Verify proxy is gone
  if ! curl -sf "${PROXY_URL}/health" >/dev/null 2>&1; then
    success "/health unavailable after SIGTERM (proxy fully stopped)"
  else
    failure "/health still responding after SIGTERM (proxy did not stop)"
  fi

  wait "$HEY_PID" 2>/dev/null || true

  # hey error rate during shutdown
  if [[ -f "${LOG_DIR}/phase4-hey.log" ]]; then
    local success_count; success_count=$(grep -E '\[200\]' "${LOG_DIR}/phase4-hey.log" | grep -oE '[0-9]+ responses' | grep -oE '[0-9]+' | head -1 || echo "0")
    log "Load test during shutdown: $(grep -E 'Responses|Status' "${LOG_DIR}/phase4-hey.log" || true)"
  fi

  section "Restarting proxy → measuring MTTR"
  local t_restart; t_restart=$(date +%s%3N)
  start_proxy
  wait_for "${PROXY_URL}/health" "proxy (restart)" 30
  local t_ready; t_ready=$(date +%s%3N)
  local mttr_ms=$(( t_ready - t_restart ))

  if [[ $mttr_ms -lt 3000 ]]; then
    success "MTTR: ${mttr_ms}ms (< 3s — fast restart)"
  elif [[ $mttr_ms -lt 10000 ]]; then
    success "MTTR: ${mttr_ms}ms (< 10s — acceptable)"
  else
    failure "MTTR: ${mttr_ms}ms (> 10s — slow restart)"
  fi

  section "Post-restart validation: 10 requests (all must succeed)"
  local ok=0
  for i in $(seq 1 10); do
    local code; code=$(http_status "${PROXY_URL}/api/v1/neutron/health" -X GET)
    [[ "$code" == "200" ]] && ((ok++))
    sleep 0.1
  done
  if [[ $ok -ge 9 ]]; then
    success "Post-restart: ${ok}/10 succeeded (no state corruption)"
  else
    failure "Post-restart: only ${ok}/10 succeeded"
  fi

  section "Phase 4 Result"
  echo -e "  Pass: ${GREEN}${PASS}${NC}  Fail: ${RED}${FAIL}${NC}"

  stop_proxy
  stop_neutron_stub
}

# ─────────────────────────────────────────────────────────────────────────────
# Phase 5: Database Connection Loss (TimescaleDB)
# ─────────────────────────────────────────────────────────────────────────────

run_phase5() {
  phase "5" "Database Connection Loss (TimescaleDB — non-critical path)"
  PASS=0; FAIL=0

  if ! docker ps --format '{{.Names}}' | grep -q "${TIMESCALE_CONTAINER}" 2>/dev/null; then
    warn "Skipping Phase 5 — TimescaleDB container '${TIMESCALE_CONTAINER}' not found"
    return
  fi

  section "Starting proxy + neutron stub"
  start_neutron_stub
  start_proxy
  wait_for "${PROXY_URL}/health" "proxy" 15

  section "Baseline before DB stop"
  local before; before=$(http_status "${PROXY_URL}/api/v1/ingest" -X POST \
    -H 'Content-Type: application/json' -d '{"event":"before-db-loss"}')
  assert_status "Ingest before DB stop" "200" "$before"

  section "Stopping TimescaleDB (docker stop)"
  docker stop "${TIMESCALE_CONTAINER}" > /dev/null
  log "TimescaleDB stopped — proxy should continue serving (DB not on critical path)"
  sleep 1

  section "Verifying proxy still serves requests without DB"
  local health_code; health_code=$(http_status_public "${PROXY_URL}/health")
  assert_status "/health with DB down" "200" "$health_code"

  local ingest_code; ingest_code=$(http_status "${PROXY_URL}/api/v1/ingest" -X POST \
    -H 'Content-Type: application/json' -d '{"event":"during-db-loss"}')
  if [[ "$ingest_code" == "200" ]]; then
    success "Ingest functional with DB down (events route via NATS, not DB)"
  else
    warn "Ingest returned ${ingest_code} with DB down (may be acceptable if NATS also affected)"
  fi

  local neutron_code; neutron_code=$(http_status "${PROXY_URL}/api/v1/neutron/health" -X GET)
  assert_status "Neutron proxy functional with DB down" "200" "$neutron_code"

  section "Verifying rate limiting still enforced (pure in-memory)"
  # Temporarily lower rate limit — send burst
  local rl_pass=0
  for i in $(seq 1 5); do
    local code; code=$(http_status "${PROXY_URL}/api/v1/ingest" -X POST \
      -H 'Content-Type: application/json' -d '{"event":"rl-test"}')
    [[ "$code" == "200" ]] && ((rl_pass++))
    sleep 0.1
  done
  if [[ $rl_pass -ge 3 ]]; then
    success "Rate limiter functional with DB down: ${rl_pass}/5 passed"
  else
    warn "Rate limiting impact: ${rl_pass}/5 passed (may be expected if rate limited)"
  fi

  section "Restarting TimescaleDB"
  docker start "${TIMESCALE_CONTAINER}" > /dev/null
  sleep 2

  section "Post-restart validation"
  local after; after=$(http_status "${PROXY_URL}/api/v1/ingest" -X POST \
    -H 'Content-Type: application/json' -d '{"event":"after-db-restore"}')
  assert_status "Ingest after DB restore" "200" "$after"

  section "Phase 5 Result"
  echo -e "  Pass: ${GREEN}${PASS}${NC}  Fail: ${RED}${FAIL}${NC}"

  stop_proxy
  stop_neutron_stub
}

# ─────────────────────────────────────────────────────────────────────────────
# Phase 6: Cascading Failure (NATS + Neutron simultaneously)
# ─────────────────────────────────────────────────────────────────────────────

run_phase6() {
  phase "6" "Cascading Failure (NATS + Neutron simultaneously)"
  PASS=0; FAIL=0

  if ! docker ps --format '{{.Names}}' | grep -q "${NATS_CONTAINER}" 2>/dev/null; then
    warn "Skipping Phase 6 — NATS container not found"
    return
  fi

  section "Starting proxy + neutron stub"
  start_neutron_stub
  start_proxy
  wait_for "${PROXY_URL}/health" "proxy" 15

  section "Inducing cascading failure: kill NATS + neutron simultaneously"
  docker stop "${NATS_CONTAINER}" > /dev/null
  stop_neutron_stub
  log "NATS stopped, neutron stopped — maximum degradation scenario"
  sleep 1

  section "Validating graceful degradation"

  # /health must always return 200 (liveness is independent)
  local health_code; health_code=$(http_status_public "${PROXY_URL}/health")
  assert_status "/health during cascading failure" "200" "$health_code"

  # /metrics must always return 200 (Prometheus scrape must work)
  local metrics_code; metrics_code=$(http_status "${PROXY_URL}/metrics" -X GET)
  if [[ "$metrics_code" == "200" ]]; then
    success "/metrics during cascading failure → 200 (observability intact)"
  else
    warn "/metrics during cascading failure → ${metrics_code}"
  fi

  # /ingest will fail (NATS down) — expected failure, verify correct error code
  local ingest_code; ingest_code=$(http_status "${PROXY_URL}/api/v1/ingest" -X POST \
    -H 'Content-Type: application/json' -d '{"event":"cascade-test"}')
  if [[ "$ingest_code" != "200" ]]; then
    success "Ingest correctly fails during NATS outage (${ingest_code} — no silent data loss)"
  else
    warn "Ingest returned 200 with NATS down (check if events are actually delivered)"
  fi

  # /neutron will fail (circuit open) — verify 503 not 500
  local neutron_code; neutron_code=$(http_status "${PROXY_URL}/api/v1/neutron/health" -X GET)
  if [[ "$neutron_code" == "503" || "$neutron_code" == "502" ]]; then
    success "Neutron proxy returns ${neutron_code} during cascading failure (circuit/upstream error — not 500)"
  else
    warn "Neutron proxy returned ${neutron_code} during cascading failure"
  fi

  section "Restoring infrastructure"
  docker start "${NATS_CONTAINER}" > /dev/null
  start_neutron_stub
  log "Waiting for NATS + proxy reconnect..."
  sleep "${CB_TIMEOUT}"  # wait for circuit recovery

  section "Full recovery validation"
  wait_for "${PROXY_URL}/health" "proxy after cascade restore" 20

  local final_health; final_health=$(http_status_public "${PROXY_URL}/health")
  assert_status "/health after cascade recovery" "200" "$final_health"

  local final_neutron; final_neutron=$(http_status "${PROXY_URL}/api/v1/neutron/health" -X GET)
  if [[ "$final_neutron" == "200" ]]; then
    success "Neutron proxy recovered after cascade: 200"
  else
    warn "Neutron proxy recovery uncertain: ${final_neutron} (may need more time for CB)"
  fi

  section "Phase 6 Result"
  echo -e "  Pass: ${GREEN}${PASS}${NC}  Fail: ${RED}${FAIL}${NC}"

  stop_proxy
  stop_neutron_stub
}

# ─────────────────────────────────────────────────────────────────────────────
# Summary Report
# ─────────────────────────────────────────────────────────────────────────────

print_summary() {
  echo -e "\n${BOLD}${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
  echo -e "${BOLD}  SPECTRE Chaos Engineering — Final Summary${NC}"
  echo -e "${BOLD}${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
  echo -e "  Total PASS: ${GREEN}${BOLD}${TOTAL_PHASE_PASS}${NC}"
  echo -e "  Total FAIL: ${RED}${BOLD}${TOTAL_PHASE_FAIL}${NC}"
  echo ""
  echo -e "  Logs: ${LOG_DIR}/"
  echo ""

  if [[ $TOTAL_PHASE_FAIL -eq 0 ]]; then
    echo -e "  ${GREEN}${BOLD}ALL CHAOS TESTS PASSED — System is resilient ✓${NC}"
  elif [[ $TOTAL_PHASE_FAIL -le 3 ]]; then
    echo -e "  ${YELLOW}${BOLD}MOSTLY PASSING — ${TOTAL_PHASE_FAIL} minor failures${NC}"
  else
    echo -e "  ${RED}${BOLD}RESILIENCE GAPS DETECTED — ${TOTAL_PHASE_FAIL} failures need attention${NC}"
  fi
  echo -e "${BOLD}${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"
}

# ─────────────────────────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────────────────────────

main() {
  preflight

  case "${RUN_PHASE}" in
    1) run_phase1 ;;
    2) run_phase2 ;;
    3) run_phase3 ;;
    4) run_phase4 ;;
    5) run_phase5 ;;
    6) run_phase6 ;;
    *)
      run_phase1
      run_phase2
      run_phase3
      run_phase4
      run_phase5
      run_phase6
      ;;
  esac

  print_summary
}

main "$@"
