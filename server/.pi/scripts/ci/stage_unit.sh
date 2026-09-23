#!/usr/bin/env bash
# Unit stage: library-level tests for the native API host.
set -euo pipefail

cd "$(dirname "$0")/../../.."

echo "--- server unit tests ---"
cargo test -p rigorix-server --lib
