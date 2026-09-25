# syntax=docker/dockerfile:1
#
# Multi-stage build for mergemint-backend.
#
#   chef    — Rust toolchain + cargo-chef (build-time only)
#   planner — computes a dependency "recipe" from Cargo.toml / Cargo.lock
#   builder — builds dependencies from the recipe (cached layer), then the app
#   runtime — distroless, non-root, contains only the release binary
#
# Dependency compilation lives in its own layer keyed on the recipe, so
# source-only changes rebuild just the application crate.

ARG RUST_VERSION=1.85

FROM rust:${RUST_VERSION}-slim-bookworm AS chef
RUN cargo install cargo-chef --locked --version 0.1.71
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --locked --bin mergemint-backend --recipe-path recipe.json
COPY . .
RUN cargo build --release --locked --bin mergemint-backend

# cc-debian12 ships glibc + libgcc (needed by the Rust std runtime) and CA
# certificates, but no shell or package manager. The :nonroot tag runs as
# uid/gid 65532.
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

WORKDIR /app
COPY --from=builder --chown=nonroot:nonroot /app/target/release/mergemint-backend /app/mergemint-backend

USER nonroot:nonroot
EXPOSE 8080

# No curl in distroless — the binary probes its own /health endpoint.
HEALTHCHECK --interval=10s --timeout=5s --start-period=10s --retries=3 \
  CMD ["/app/mergemint-backend", "healthcheck"]

ENTRYPOINT ["/app/mergemint-backend"]
