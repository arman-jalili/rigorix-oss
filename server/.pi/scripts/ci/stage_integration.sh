#!/usr/bin/env bash
# Integration stage: JSON-RPC transport, auth levels, catalog parity.
set -euo pipefail

cd "$(dirname "$0")/../../.."

echo "--- server integration tests ---"
cargo test -p rigorix-server --tests
