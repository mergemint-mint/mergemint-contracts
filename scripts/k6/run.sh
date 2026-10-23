#!/usr/bin/env bash
# Runs a k6 scenario against the backend.
#
# Usage: scripts/k6/run.sh [mixed|list|filter|stream|tx|sse|sse-limit] [extra k6 args...]
#
# Uses a local `k6` binary when available, otherwise the grafana/k6 Docker
# image. Configuration is read from the environment (BASE_URL, DURATION,
# LIST_RPS, ...) — see scripts/k6/README.md.
set -euo pipefail

SCENARIO="${1:-mixed}"
shift || true

K6_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$K6_DIR/$SCENARIO.js"
if [ ! -f "$SCRIPT" ]; then
  echo "Unknown scenario '$SCENARIO'. Choose one of: mixed list filter stream tx sse sse-limit" >&2
  exit 1
fi

# Forward every documented setting that is set in the environment as -e flags.
ENV_ARGS=()
# A variable set to an empty string is forwarded too (e.g. API_PREFIX=).
for var in BASE_URL API_PREFIX DURATION RAMP LIST_RPS FILTER_RPS TX_RPS STREAM_VUS \
  STREAM_HOLD_SECONDS PAGE_LIMIT ASSIGNEES BOUNTY_IDS SSE_CLIENTS SSE_HOLD_SECONDS \
  TRIGGER_RPS SSE_LATENCY_P95_MS SSE_LATENCY_P99_MS SSE_LIMIT_ATTEMPTS SSE_EXPECTED_CAP \
  SSE_LIMIT_HOLD_SECONDS; do
  if [ -n "${!var+set}" ]; then
    ENV_ARGS+=(-e "$var=${!var}")
  fi
done

# The SSE scenarios need a k6 binary with the xk6-sse extension.
case "$SCENARIO" in
  sse | sse-limit)
    K6_SSE_BIN="${K6_SSE_BIN:-$K6_DIR/bin/k6-sse}"
    if [ ! -x "$K6_SSE_BIN" ]; then
      echo "The '$SCENARIO' scenario needs k6 with xk6-sse. Build it with scripts/k6/build-k6-sse.sh" >&2
      echo "or point K6_SSE_BIN at an existing binary." >&2
      exit 1
    fi
    exec "$K6_SSE_BIN" run "${ENV_ARGS[@]}" "$@" "$SCRIPT"
    ;;
esac

if command -v k6 > /dev/null 2>&1; then
  exec k6 run "${ENV_ARGS[@]}" "$@" "$SCRIPT"
fi

if command -v docker > /dev/null 2>&1; then
  # --network host lets the container reach a backend on localhost:8080.
  exec docker run --rm -i --network host \
    -v "$K6_DIR:/scripts:ro" \
    grafana/k6:2.3.0 run "${ENV_ARGS[@]}" "$@" "/scripts/$SCENARIO.js"
fi

echo "Neither k6 nor docker found. Install k6: https://grafana.com/docs/k6/latest/set-up/install-k6/" >&2
exit 1
