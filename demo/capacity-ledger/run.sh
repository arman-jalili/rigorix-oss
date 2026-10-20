#!/usr/bin/env bash
# =============================================================================
# Capacity ledger demo (ADR-018) — Docker-free, offline, one command.
#
# Writes a throwaway `.rigorix/capacity.toml`, then shows:
#   Act 1  reserve -> retry -> refusal -> commit / release, with the aggregate
#          exposure after each step (committed and outstanding never summed);
#   Act 2  the same declaration armed through CapacitySetup and driving the real
#          executor: two executions contend for a cap of 4 with 3-unit steps —
#          exactly one dispatches, the other is refused with `capacity_denied`
#          BEFORE its tool runs, so its marker file never appears.
#
# No mocks, no network, no docker-compose. Every number printed is read back
# from the ledger after the operation that changed it.
#
# Usage:  demo/capacity-ledger/run.sh
# =============================================================================
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

exec cargo run -q -p rigorix-engine --example capacity_ledger_demo
