#!/usr/bin/env bash
# scripts/load_test.sh — concurrent load test for mergemint-backend.
#
# Fires concurrent requests at mergemint-backend's key routes and reports
# latency (min/avg/p95/max) and error rate per route. Complements
# scripts/smoke_test.sh, which only exercises a single happy-path request
# against the on-chain contract rather than the HTTP server under load.
#
# Usage:
#   cargo run --bin mergemint-backend &   # start the server in another shell
#   ./scripts/load_test.sh
#
# Optional env vars:
#   HOST         - backend base URL (default: http://localhost:8080)
#   CONCURRENCY  - number of requests in flight at once (default: 20)
#   REQUESTS     - total requests fired per route (default: 200)

set -euo pipefail

HOST="${HOST:-http://localhost:8080}"
CONCURRENCY="${CONCURRENCY:-20}"
REQUESTS="${REQUESTS:-200}"

log() { echo "[load_test] $*"; }
fail() { echo "[load_test] FAIL: $*" >&2; exit 1; }

command -v curl >/dev/null 2>&1 || fail "curl is required but not installed."

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

# ---------------------------------------------------------------------------
# Routes under test
#
# mergemint-backend currently mounts two POST routes (see main.rs); both are
# exercised here so the load test covers every route the server serves.
# Requests target a bounty ID that does not exist in the in-memory store, so
# a 404 is the expected response — this script measures latency/error-rate
# of the HTTP stack under concurrency, not business-logic correctness (that
# is covered by `cargo test`).
# ---------------------------------------------------------------------------
ROUTES=(
  "self-claim|/tx/self-claim|{\"bounty_id\":\"load-test-bounty\",\"claimant\":\"GLOADTEST\"}"
  "resolve-dispute|/tx/resolve-dispute|{\"bounty_id\":\"load-test-bounty\",\"arbitrator\":\"GLOADTEST\",\"winner\":\"GLOADTEST\"}"
)

# Fire one request, appending "<http_status> <time_total_seconds>" to $out_file.
fire_request() {
  local url="$1" body="$2" out_file="$3"
  local result
  result=$(curl -s -o /dev/null -w "%{http_code} %{time_total}\n" \
    -X POST "$url" \
    -H "Content-Type: application/json" \
    -d "$body" || echo "000 0")
  echo "$result" >>"$out_file"
}

run_route_load_test() {
  local name="$1" path="$2" body="$3"
  local url="${HOST}${path}"
  local out_file="${WORKDIR}/${name}.out"
  : >"$out_file"

  log "Load testing ${name} (${url}) — ${REQUESTS} requests, concurrency ${CONCURRENCY}..."

  local inflight=0
  for ((i = 0; i < REQUESTS; i++)); do
    fire_request "$url" "$body" "$out_file" &
    inflight=$((inflight + 1))
    if ((inflight >= CONCURRENCY)); then
      wait -n
      inflight=$((inflight - 1))
    fi
  done
  wait

  report_route "$name" "$out_file"
}

report_route() {
  local name="$1" out_file="$2"
  local total errors success error_rate
  total=$(wc -l <"$out_file" | tr -d ' ')
  errors=$(awk '$1 == "000" || $1 >= 500 { c++ } END { print c+0 }' "$out_file")
  success=$((total - errors))
  error_rate=$(awk -v e="$errors" -v t="$total" 'BEGIN { if (t == 0) print "0.00"; else printf "%.2f", (e / t) * 100 }')

  # Latency stats over time_total (seconds), converted to milliseconds.
  local stats min avg p95 max
  stats=$(awk '{ print $2 * 1000 }' "$out_file" | sort -n | awk '
    { a[NR] = $1; sum += $1 }
    END {
      n = NR
      if (n == 0) { print "0 0 0 0"; exit }
      min = a[1]; max = a[n]
      avg = sum / n
      p95_idx = int(0.95 * n); if (p95_idx < 1) p95_idx = 1
      p95 = a[p95_idx]
      printf "%.2f %.2f %.2f %.2f", min, avg, p95, max
    }')
  read -r min avg p95 max <<<"$stats"

  echo ""
  echo "== ${name} =="
  echo "  requests:    ${total}"
  echo "  success:     ${success}"
  echo "  errors:      ${errors} (${error_rate}%)"
  echo "  latency ms:  min=${min} avg=${avg} p95=${p95} max=${max}"
}

log "Target: ${HOST}"
for route in "${ROUTES[@]}"; do
  IFS='|' read -r name path body <<<"$route"
  run_route_load_test "$name" "$path" "$body"
done

echo ""
log "Load test complete."
