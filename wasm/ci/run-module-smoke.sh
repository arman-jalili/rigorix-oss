#!/usr/bin/env bash
# run-module-smoke.sh — load the built module and call the four exports (#1349).
#
# The other gates prove the crate *compiles*: "release build for wasm" ran
# `cargo build --target wasm32-unknown-unknown` and passed on a 368-byte module
# with no exported symbol, because nothing loads it. A build gate is a fact about
# rustc; it is not a fact about the deliverable, which is a module a host can call.
#
# This gate runs the deliverable: it packages the release artifact the way a host
# consumes it (`wasm-bindgen --target nodejs`), instantiates it in Node, and calls
# all four functions against today's unlinked seam. It fails on
#
#   · a missing export          — the contract function is not in the module
#   · a fifth export            — an unannounced function needs an ADR (ADR-013)
#   · an instantiation error    — the module cannot be loaded at all
#   · a wrong return value      — a refusal that is a throw, a handle that is not
#                                 a handle, a version that does not match
#
# Fail-closed: no `wasm-bindgen` CLI, no `node`, or a build that fails is a
# failure, never a skip. (The gates that need the network are labelled; a local
# toolchain requirement is not a network dependency.)
#
# Usage: bash wasm/ci/run-module-smoke.sh
set -uo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$CRATE_DIR"

TARGET_DIR="$(cargo metadata --no-deps --format-version 1 2>/dev/null \
    | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p' | head -1)"
if [ -z "${TARGET_DIR:-}" ]; then
    echo "❌ could not resolve the target directory (cargo metadata failed)" >&2
    exit 1
fi

RAW="$TARGET_DIR/wasm32-unknown-unknown/release/rigorix_gate_wasm.wasm"
PKG_DIR="$CRATE_DIR/pkg"
GLUE="$PKG_DIR/rigorix_gate_wasm.js"
MODULE="$PKG_DIR/rigorix_gate_wasm_bg.wasm"
STRIP="$CRATE_DIR/ci/strip_name_section.py"

# The CLI version must equal the crate's, or the glue and the module disagree
# about the ABI and every import resolves to the wrong signature.
WANT_BINDGEN="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/[",]/, "", $3); print $3; exit }' Cargo.lock)"
if [ -z "${WANT_BINDGEN:-}" ]; then
    WANT_BINDGEN="$(sed -nE 's/^wasm-bindgen = "=?([0-9.]+)".*/\1/p' Cargo.toml | head -1)"
fi
if [ -z "${WANT_BINDGEN:-}" ]; then
    echo "❌ could not determine the wasm-bindgen version the crate uses" >&2
    exit 1
fi

if ! command -v node > /dev/null 2>&1; then
    echo "❌ node is required to load the module — the deliverable is called from a host" >&2
    exit 1
fi

if ! command -v wasm-bindgen > /dev/null 2>&1; then
    echo "❌ wasm-bindgen ${WANT_BINDGEN} CLI is not installed; the module cannot be packaged" >&2
    echo "   install it with: cargo install wasm-bindgen-cli --version ${WANT_BINDGEN} --locked" >&2
    exit 1
fi

HAVE_BINDGEN="$(wasm-bindgen --version | sed -nE 's/^wasm-bindgen ([0-9.]+)$/\1/p')"
if [ "$HAVE_BINDGEN" != "$WANT_BINDGEN" ]; then
    echo "❌ wasm-bindgen CLI ${HAVE_BINDGEN} does not match the crate's ${WANT_BINDGEN}" >&2
    echo "   cargo install wasm-bindgen-cli --version ${WANT_BINDGEN} --locked" >&2
    exit 1
fi

echo "== building the release module =="
if ! bash "$CRATE_DIR/ci/wasm-build.sh"; then
    echo "❌ the release module does not build" >&2
    exit 1
fi

echo "== packaging it the way a host consumes it (wasm-bindgen --target nodejs) =="
rm -rf "$PKG_DIR"
if ! wasm-bindgen --target nodejs --out-dir "$PKG_DIR" "$RAW" 2>&1 | tail -3; then
    echo "❌ wasm-bindgen could not package the module" >&2
    exit 1
fi
if [ ! -f "$GLUE" ]; then
    echo "❌ expected the glue at ${GLUE}" >&2
    exit 1
fi

# #1353: the emitted file is not the deliverable. wasm-bindgen output carries a
# custom `name` section (the debug symbol table) — 33 485 B of this module, 341 785 B
# of the engine-linked one — which no production host reads. The strip happens in
# the packaging path, not in a later step, so the artifact this gate loads, the
# artifact the size gate measures and the artifact host verification loads are one
# and the same file.
echo "== stripping the debug \`name\` section (the deliverable, not the emitted file) =="
if ! python3 "$STRIP" "$MODULE"; then
    echo "❌ could not strip the packaged artifact: the size gate would then measure a debug file" >&2
    exit 1
fi

# #1408: the shipped module must not carry the builder's filesystem. `ci/wasm-build.sh`
# remaps CARGO_HOME and RUSTUP_HOME, so a path in this file means the remap did not reach the
# build — the assertion is on the property, not on the size, because a size tolerance hid 118
# occurrences of a home directory for two releases. Fail-closed: `strings` absent is an error,
# not a pass.
echo "== the shipped module carries no builder paths (#1408) =="
if ! command -v strings > /dev/null 2>&1; then
    echo "❌ \`strings\` is required to assert the artifact's portability; install binutils" >&2
    exit 1
fi
BUILDER_PATHS="$(strings -a "$MODULE" | grep -c "$HOME" || true)"
if [ "${BUILDER_PATHS:-0}" -ne 0 ]; then
    echo "❌ the packaged module carries ${BUILDER_PATHS} occurrence(s) of ${HOME}" >&2
    echo "   the build did not remap CARGO_HOME/RUSTUP_HOME — see ci/wasm-build.sh (#1408)" >&2
    strings -a "$MODULE" | grep -m3 "$HOME" >&2 || true
    exit 1
fi
echo "✅ 0 occurrences of ${HOME} in the packaged module"

# The frozen list, read from the source of truth rather than restated here: a
# rename in `CONTRACT_FUNCTIONS` must reach this gate.
CONTRACT="$(sed -n '/pub const CONTRACT_FUNCTIONS/,/];/p' src/interfaces/wasm_api.rs \
    | sed -nE 's/^[[:space:]]*"([a-z_]+)",[[:space:]]*$/\1/p' | paste -sd, -)"
if [ -z "${CONTRACT:-}" ]; then
    echo "❌ could not read CONTRACT_FUNCTIONS from src/interfaces/wasm_api.rs" >&2
    exit 1
fi
echo "== contract functions: ${CONTRACT} =="

echo "== loading the module and calling every export =="
node ci/module-smoke.mjs "$GLUE" "$CONTRACT"
status=$?

# pkg/ is deliberately left in place: it is the artifact a host ships, and the
# bundle-size gate that runs next measures *it* rather than the raw cdylib. It is
# a build output (gitignored), not a source file.
exit "$status"
