#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
#
# Usage: scripts/build-release-wasm.sh [VERSION] [OUT_DIR]
#
# Builds the release WASM exactly the way the release workflow does, so anyone
# can rebuild a tagged release and get a byte-identical file:
#   - toolchain pinned by rust-toolchain.toml (no +toolchain override)
#   - dependencies pinned by Cargo.lock (--locked)
#   - size-optimized `release` profile from Cargo.toml (opt-level "z", LTO,
#     symbols stripped), target wasm32v1-none — the same file deploy-testnet.yml
#     deploys
#   - a fresh target directory, so no stale artifacts leak in
#
# Writes to OUT_DIR (default: dist/):
#   mergemint_contracts-VERSION.wasm
#   mergemint_contracts-VERSION.wasm.sha256   (`sha256sum -c` format)
#
# VERSION defaults to `git describe --tags --always`.
# See "Verifying a release WASM" in docs/runbook.md.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

VERSION="${1:-$(git describe --tags --always --dirty)}"
OUT_DIR="${2:-dist}"
TARGET=wasm32v1-none
NAME="mergemint_contracts-${VERSION}.wasm"

if [ -n "${RUSTUP_TOOLCHAIN:-}" ]; then
  echo "error: RUSTUP_TOOLCHAIN is set (${RUSTUP_TOOLCHAIN}); unset it so rust-toolchain.toml decides the toolchain." >&2
  exit 1
fi
# Warn rather than fail: verifiers often build from a checkout with local edits.
if ! git diff --quiet HEAD -- src Cargo.toml Cargo.lock rust-toolchain.toml; then
  echo "warning: uncommitted changes in contract sources; the hash will not match the release." >&2
fi

BUILD_DIR="$(mktemp -d)"
trap 'rm -rf "$BUILD_DIR"' EXIT

echo "Toolchain: $(rustc --version)"
# Clear RUSTFLAGS so local cargo config or env cannot change the output bytes.
RUSTFLAGS="" CARGO_ENCODED_RUSTFLAGS="" \
  cargo build --locked --release --target "$TARGET" --target-dir "$BUILD_DIR"

mkdir -p "$OUT_DIR"
cp "$BUILD_DIR/$TARGET/release/mergemint_contracts.wasm" "$OUT_DIR/$NAME"
(cd "$OUT_DIR" && sha256sum "$NAME" > "$NAME.sha256")

SHA256="$(cut -d' ' -f1 "$OUT_DIR/$NAME.sha256")"
echo "WASM:   $OUT_DIR/$NAME ($(wc -c < "$OUT_DIR/$NAME") bytes)"
echo "SHA256: $SHA256"

# Expose results to a calling GitHub Actions step.
if [ -n "${GITHUB_OUTPUT:-}" ]; then
  {
    echo "name=$NAME"
    echo "path=$OUT_DIR/$NAME"
    echo "sha256=$SHA256"
  } >> "$GITHUB_OUTPUT"
fi
