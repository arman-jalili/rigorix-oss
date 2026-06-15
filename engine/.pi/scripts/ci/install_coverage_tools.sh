#!/usr/bin/env bash
# Install coverage tools for Rigorix CI
# Run this in CI before executing coverage checks.

set -euo pipefail

echo "📊 Installing coverage tools..."

# Install cargo-llvm-cov (preferred)
if ! command -v cargo-llvm-cov &>/dev/null && ! cargo llvm-cov --help &>/dev/null 2>&1; then
    echo "  → Installing cargo-llvm-cov..."
    cargo install cargo-llvm-cov 2>&1 | tail -1
fi

echo "✅ Coverage tools ready."
