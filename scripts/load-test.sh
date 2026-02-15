#!/usr/bin/env bash
# SPECTRE Proxy Load Test Script
# Usage: ./scripts/load-test.sh [BASE_URL] [DURATION] [CONNECTIONS]

set -euo pipefail

BASE_URL="${1:-http://localhost:3000}"
DURATION="${2:-30s}"
CONNECTIONS="${3:-50}"
JWT_SECRET="${JWT_SECRET:-secret}"

echo "=== SPECTRE Proxy Load Test ==="
echo "Target:      $BASE_URL"
echo "Duration:    $DURATION"
echo "Connections: $CONNECTIONS"
echo ""

# Generate JWT token for service role
generate_token() {
  local role="${1:-service}"
  local header=$(echo -n '{"alg":"HS256","typ":"JWT"}' | base64 -w0 | tr '+/' '-_' | tr -d '=')
  local exp=$(($(date +%s) + 3600))
  local payload=$(echo -n "{\"sub\":\"loadtest\",\"role\":\"${role}\",\"exp\":${exp}}" | base64 -w0 | tr '+/' '-_' | tr -d '=')
  local signature=$(echo -n "${header}.${payload}" | openssl dgst -sha256 -hmac "$JWT_SECRET" -binary | base64 -w0 | tr '+/' '-_' | tr -d '=')
  echo "${header}.${payload}.${signature}"
}

TOKEN=$(generate_token "service")

# Check if target is reachable
echo "Checking target..."
if ! curl -sf "$BASE_URL/health" > /dev/null 2>&1; then
  echo "ERROR: Cannot reach $BASE_URL/health"
  echo "Start the proxy first: JWT_SECRET=secret cargo run -p spectre-proxy"
  exit 1
fi
echo "Target is healthy."
echo ""

# Phase 1: Health endpoint (no auth)
echo "--- Phase 1: Health Endpoint (no auth) ---"
if command -v wrk &> /dev/null; then
  wrk -t4 -c"$CONNECTIONS" -d"$DURATION" "$BASE_URL/health"
elif command -v hey &> /dev/null; then
  hey -z "$DURATION" -c "$CONNECTIONS" "$BASE_URL/health"
else
  echo "Using curl-based load test (install wrk or hey for better results)"
  for i in $(seq 1 100); do
    curl -sf "$BASE_URL/health" > /dev/null &
  done
  wait
  echo "Sent 100 concurrent requests to /health"
fi
echo ""

# Phase 2: Metrics endpoint (no auth)
echo "--- Phase 2: Metrics Endpoint ---"
if command -v wrk &> /dev/null; then
  wrk -t2 -c10 -d10s "$BASE_URL/metrics"
elif command -v hey &> /dev/null; then
  hey -z 10s -c 10 "$BASE_URL/metrics"
fi
echo ""

# Phase 3: Authenticated endpoint (with JWT)
echo "--- Phase 3: Ingest Endpoint (authenticated) ---"
if command -v wrk &> /dev/null; then
  wrk -t4 -c"$CONNECTIONS" -d"$DURATION" \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -s /dev/stdin "$BASE_URL/api/v1/ingest" <<'LUA'
wrk.method = "POST"
wrk.body = '{"event":"load_test","data":{"timestamp":"2026-01-01T00:00:00Z"}}'
LUA
elif command -v hey &> /dev/null; then
  hey -z "$DURATION" -c "$CONNECTIONS" -m POST \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"event":"load_test","data":{"timestamp":"2026-01-01T00:00:00Z"}}' \
    "$BASE_URL/api/v1/ingest"
fi
echo ""

# Phase 4: Rate limit test
echo "--- Phase 4: Rate Limit Test ---"
echo "Sending burst of 250 requests..."
RATE_LIMITED=0
for i in $(seq 1 250); do
  STATUS=$(curl -sf -o /dev/null -w "%{http_code}" \
    -X POST \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"event":"ratelimit_test"}' \
    "$BASE_URL/api/v1/ingest" 2>/dev/null || echo "000")
  if [ "$STATUS" = "429" ]; then
    RATE_LIMITED=$((RATE_LIMITED + 1))
  fi
done
echo "Rate limited responses (429): $RATE_LIMITED / 250"
if [ "$RATE_LIMITED" -gt 0 ]; then
  echo "Rate limiting is working correctly!"
else
  echo "NOTE: Rate limit may be set higher than burst size"
fi
echo ""

# Summary
echo "=== Load Test Complete ==="
echo "Check metrics at: $BASE_URL/metrics"
