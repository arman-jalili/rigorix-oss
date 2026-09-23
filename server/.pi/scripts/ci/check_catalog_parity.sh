#!/usr/bin/env bash
# Catalog parity (ADR-0001 D9): the server serves exactly the frozen
# `rigorix.*` catalog and every MCP tool maps 1:1. When RIGORIX_SDK_SCHEMAS is
# set (CI conformance job) the in-code catalog is compared field-for-field
# against rigorix-sdk `api/catalog.json`.
set -euo pipefail

cd "$(dirname "$0")/../../.."

echo "--- server catalog parity ---"
cargo test -p rigorix-server --test catalog_parity
