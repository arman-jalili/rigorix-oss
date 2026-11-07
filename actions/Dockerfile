# Rigorix GitHub Action — Container-based Action
#
# Multi-stage build for minimal image size.
# Pinned to digest for reproducible builds in production.

# ── Stage 1: Build ──
FROM rust:1.91-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY engine/Cargo.toml engine/
COPY cli/Cargo.toml cli/
COPY actions/Cargo.toml actions/
COPY mcp/Cargo.toml mcp/
# `server` is a workspace member: cargo loads EVERY member manifest even when
# building `-p rigorix-actions`, so its manifest (and src, to resolve targets)
# must be present or the workspace fails to load (#888 PR B regression).
COPY server/Cargo.toml server/
# `wasm` is a workspace member too (ADR-011). Same rule as `server`: its manifest and
# source must be present or `cargo` cannot load the workspace at all — the image build is
# the one consumer that copies the tree by hand and therefore the one that breaks first
# when a member is added.
COPY wasm/Cargo.toml wasm/

# Copy source code
COPY engine/src engine/src/
COPY engine/benches engine/benches/
COPY cli/src cli/src/
COPY actions/src actions/src/
COPY mcp/src mcp/src/
COPY server/src server/src/
COPY wasm/src wasm/src/
# #1452: the manifest names an example (`wasm/Cargo.toml`'s `[[example]] determinism_probe`),
# and cargo parses every workspace manifest before it builds anything — a declared example with
# no file fails the *actions* image at parse time, before a single line of `rigorix-action`
# compiles. The directory is copied, not the file, so a second example does not break it again.
COPY wasm/examples wasm/examples/

# Build release binary
RUN cargo build --release -p rigorix-actions --bin rigorix-action

# ── Stage 2: Runtime ──
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    git \
    && rm -rf /var/lib/apt/lists/*

# Run as root — GitHub Actions mounts the workspace as root-owned.
# In CI, root is safe; the container is ephemeral and isolated.
WORKDIR /github/workspace

# Copy binary from builder
COPY --from=builder /app/target/release/rigorix-action /usr/local/bin/rigorix-action

# Copy action metadata
COPY action.yml /action.yml

ENTRYPOINT ["/usr/local/bin/rigorix-action"]
