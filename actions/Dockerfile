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

# Copy source code
COPY engine/src engine/src/
COPY cli/src cli/src/
COPY actions/src actions/src/

# Build release binary
RUN cargo build --release -p rigorix-actions --bin rigorix-action

# ── Stage 2: Runtime ──
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    git \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd --create-home --shell /bin/bash rigorix
USER rigorix
WORKDIR /home/rigorix

# Copy binary from builder
COPY --from=builder /app/target/release/rigorix-action /usr/local/bin/rigorix-action

# Copy action metadata
COPY actions/action.yml /action.yml

ENTRYPOINT ["/usr/local/bin/rigorix-action"]
