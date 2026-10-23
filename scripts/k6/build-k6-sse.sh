#!/usr/bin/env bash
# Builds a k6 binary that includes the xk6-sse extension, needed by sse.js
# and sse-limit.js. Stock k6 has no SSE client.
#
# Usage: scripts/k6/build-k6-sse.sh
# Output: scripts/k6/bin/k6-sse (git-ignored). run.sh picks it up automatically.
#
# Uses a local `xk6` binary (needs Go) if it finds one, and otherwise the
# grafana/xk6 Docker image. Versions are pinned so results are reproducible.
set -euo pipefail

K6_VERSION="${K6_VERSION:-v2.3.0}"
XK6_SSE_VERSION="${XK6_SSE_VERSION:-v0.2.0}"
XK6_IMAGE="${XK6_IMAGE:-grafana/xk6:1.4.14}"

K6_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_DIR="$K6_DIR/bin"
mkdir -p "$OUT_DIR"

WITH="github.com/phymbert/xk6-sse@$XK6_SSE_VERSION"
echo "Building k6 $K6_VERSION with $WITH -> $OUT_DIR/k6-sse"

if command -v xk6 > /dev/null 2>&1; then
  xk6 build "$K6_VERSION" --with "$WITH" --output "$OUT_DIR/k6-sse"
elif command -v docker > /dev/null 2>&1; then
  docker run --rm -u "$(id -u):$(id -g)" -v "$OUT_DIR:/xk6" \
    "$XK6_IMAGE" build "$K6_VERSION" --with "$WITH" --output /xk6/k6-sse
else
  echo "Neither xk6 nor docker found. Install xk6: https://github.com/grafana/xk6" >&2
  exit 1
fi

"$OUT_DIR/k6-sse" version
