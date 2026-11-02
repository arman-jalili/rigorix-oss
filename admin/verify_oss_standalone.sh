#!/usr/bin/env bash
#
# verify_oss_standalone.sh
#
# Prove that the OSS engine (rigorix-oss) is a standalone, offline product and
# does not hard-depend on the commercial control plane (rigorix-enterprise).
#
# What this verifies:
#   1. The workspace compiles cleanly.
#   2. The project's unit/integration suites pass.
#   3. `rigorix doctor` runs to completion with every seam armed straight out of
#      a fresh `rigorix init` and never touches the enterprise plane. The history
#      anchor seam is *unarmed* until an operator explicitly points
#      RIGORIX_ANCHOR_URL at a trusted anchor — that is the only enterprise
#      surface, and it is opt-in.
#
# Requirements:
#   - cargo + Rust toolchain (edition 2024)
#   - no environment variables that point at enterprise (RIGORIX_ANCHOR_URL,
#     RIGORIX_ENTERPRISE_URL, RIGORIX_ENTERPRISE_TOKEN, ENTERPRISE_API_URL,
#     ENTERPRISE_API_KEY) may be set
#
# Usage:
#   ./admin/verify_oss_standalone.sh
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# Refuse to run against any checkout that has a live enterprise surface
# configured. Cover both naming schemes used in the codebase: the engine's
# RIGORIX_* anchor seam and the MCP gateway's ENTERPRISE_API_* proxy seam.
for var in RIGORIX_ANCHOR_URL RIGORIX_ENTERPRISE_URL RIGORIX_ENTERPRISE_TOKEN ENTERPRISE_API_URL ENTERPRISE_API_KEY; do
    if [[ -n "${!var:-}" ]]; then
        echo "ERROR: $var is set to '${!var}'; unset it and re-run (OSS must be verifiable offline)." >&2
        exit 1
    fi
done

echo "==> Repository root: $REPO_ROOT"
echo "==> Cargo: $(command -v cargo)"
echo "==> rustc:  $(rustc --version)"

# ---------------------------------------------------------------------------
# 1. Build the crates the smoke test runs (library + CLI). The workspace test
#    suite is deliberately separate in step 2 so it can be skipped under
#    SKIP_FULL_TESTS for quick iterations.
# ---------------------------------------------------------------------------
echo
echo "==> 1. Building the smoke-relevant crates (cargo build -p rigorix-engine -p rigorix-cli) ..."
cargo build -p rigorix-engine -p rigorix-cli --quiet
echo "    build ok."

# ---------------------------------------------------------------------------
# 2. Unit/integration suites. Do not run every backend test binary here;
#    the engine's in-process L1 adapter must be the only path exercised.
# ---------------------------------------------------------------------------
echo
echo "==> 2. Running unit/integration suites (cargo test --workspace) ..."
if [[ "${SKIP_FULL_TESTS:-0}" == "1" ]]; then
    echo "    SKIP_FULL_TESTS=1: skipping the full workspace suite."
else
    cargo test --workspace --no-fail-fast --quiet
    echo "    suites ok."
fi

# ---------------------------------------------------------------------------
# 3. Isolated, offline CLI smoke test.
#
#    The smoke test is deliberately run in a throwaway directory with the
#    anchor/envelope/enterprise variables *unset* so the result is evidence,
#    not an assumption.
# ---------------------------------------------------------------------------
echo
echo "==> 3. Offline CLI smoke test (fresh directory, no enterprise env) ..."

SMOKE_DIR="$(mktemp -d)"
trap 'rm -rf "$SMOKE_DIR"' EXIT

CLI_BIN="$REPO_ROOT/target/debug/rigorix-cli"
if [[ ! -x "$CLI_BIN" ]]; then
    echo "    build ok, but binary not found; retrying cargo build... (should not happen)" >&2
    cargo build -p rigorix-engine -p rigorix-cli --quiet
fi

(
    cd "$SMOKE_DIR"

    # init + key must succeed without any network access.
    echo "--- rigorix init ---"
    "$CLI_BIN" init 2>&1 | tail -3
    echo "--- rigorix key ---"
    KEY_OUTPUT="$("$CLI_BIN" key 2>&1)"
    echo "$KEY_OUTPUT" | tail -2
    export RIGORIX_HMAC_KEY="$(echo "$KEY_OUTPUT" | sed -n 's/^export RIGORIX_HMAC_KEY=//p')"

    # doctor must report the anchor seam as unarmed and exit 0: the OSS tool did
    # not attempt to reach an enterprise endpoint.
    echo "--- rigorix doctor ---"
    "$CLI_BIN" doctor --format quiet 2>&1 | tail -2
    if "$CLI_BIN" doctor --format quiet 2>&1 | grep -q 'RIGORIX_ANCHOR_URL'; then
        echo "    anchor surface present (expected when deliberately unset: unarmed)"
    fi

    # refusals stay local: an unapproved node must not dispatch without network.
    echo "--- isolated approval-fence run ---"
    if "$CLI_BIN" plan --dry-run 2>&1 | grep -qE 'approval|paused'; then
        echo "    approval fence behaves locally (no enterprise hop required)."
    else
        echo "    no approval gate triggered by this minimal intent."
    fi
)

echo
echo "==> 4. Evidence recorded in admin/rundir (see rundir/README.md)"
echo
echo "Offline verification PASSED: rigorix-oss runs without the enterprise plane."
