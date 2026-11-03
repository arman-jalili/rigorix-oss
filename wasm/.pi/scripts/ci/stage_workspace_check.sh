#!/usr/bin/env bash
# Stage: workspace compile gate.
#
# `ExecutionEvent::EffectScopeChecked` was added to the engine enum in #1189 and
# merged with only engine-scoped checks. Two downstream exhaustive matches —
# cli/src/tui/event_bridge.rs and server/src/event_bridge.rs — stopped compiling,
# and main stayed red until #1195. This stage compiles every workspace member and
# every target, so a public-enum addition fails locally before it is pushed.
#
# Run from either the repo root or engine/; the repo root is resolved from this
# script's location.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

if cargo check --workspace --all-targets; then
	echo "  ✓ PASS: workspace compiles (all members, all targets)"
else
	echo "  ✗ FAIL: workspace compile gate — a member or target does not compile"
	echo "    Fix the code; do not narrow this gate to the engine."
	exit 1
fi
