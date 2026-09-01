#!/usr/bin/env bash
# L-06: coverage tooling installer (restored from ledger claim).
# Installs cargo-llvm-cov used by the coverage proofing stage.
set -euo pipefail

if command -v cargo-llvm-cov >/dev/null 2>&1; then
    echo "cargo-llvm-cov already installed: $(cargo-llvm-cov --version)"
    exit 0
fi

echo "Installing cargo-llvm-cov..."
cargo install cargo-llvm-cov --locked

echo "cargo-llvm-cov installed: $(cargo-llvm-cov --version)"
