#!/usr/bin/env bash
# ============================================================================
# install_coverage_tools.sh — Rust: install cargo-llvm-cov
#
# cargo-llvm-cov needs the llvm-tools-preview rustup component. It is the
# real coverage tool used by .pi/scripts/languages/rust/coverage.sh.
# ============================================================================
set -euo pipefail

rustup component add llvm-tools-preview || echo "llvm-tools-preview unavailable on this toolchain"

if command -v cargo-llvm-cov >/dev/null 2>&1; then
    echo "cargo-llvm-cov already installed: $(cargo-llvm-cov --version)"
    exit 0
fi

echo "Installing cargo-llvm-cov..."
cargo install cargo-llvm-cov --locked

echo "cargo-llvm-cov installed: $(cargo-llvm-cov --version)"
