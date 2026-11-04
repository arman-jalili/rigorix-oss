#!/usr/bin/env bash
# The release build the *budget* is measured from — one place, because the artifact is
# whatever the packaging path builds.
#
# #1386: two size verdicts in this repo have been decided by build configuration rather than
# by code — the debug `name` section (341 785 B) and this profile (1 706 234 B → 1 337 205 B
# on identical code). Both were invisible until the number was measured from the artifact
# the packaging path actually produces.
#
# # Why the profile lives here and not in a manifest
#
# `wasm` is a member of the root workspace, so `[profile.release]` in `wasm/Cargo.toml` is
# silently ignored, and `lto` / `panic` cannot be overridden per package. A manifest entry
# would therefore look like a decision and change nothing — the exact failure mode a
# budget file with no assertion behind it has. So the packaging path owns the profile, every
# build in `ci/` goes through this script, and the number in `bundle-budget.txt` is measured
# from the artifact this produces.
#
# # Why these four
#
# - `lto = "fat"`: cross-crate inlining is what removes the engine's generic plumbing. The
#   biggest single lever, and the one a debug-laden bundle never gets.
# - `opt-level = "z"`: size, not speed. The gate runs once per step in a browser tab.
# - `panic = "abort"`: unwinding tables are pure size here; ADR-012 already states that no
#   panic crosses the boundary, so nothing unwinds by design.
# - `codegen-units = 1`: one unit lets LTO see everything. Costs build time, buys bytes.
#
# `strip` is deliberately not set: the `name` custom section is removed by
# `ci/strip_name_section.py`, whose own assertions prove it ran (a stripped artifact is
# smaller than the unstripped one, so size alone cannot prove it).
set -euo pipefail

export CARGO_PROFILE_RELEASE_LTO=fat
export CARGO_PROFILE_RELEASE_OPT_LEVEL=z
export CARGO_PROFILE_RELEASE_PANIC=abort
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1

exec cargo build --quiet --release --target wasm32-unknown-unknown "$@"
