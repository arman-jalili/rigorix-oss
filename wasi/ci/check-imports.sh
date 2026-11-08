#!/usr/bin/env bash
# check-imports.sh — the module's import section against its recorded list, exactly.
#
#   bash wasi/ci/check-imports.sh <module.wasm>     compare, naming what moved
#   bash wasi/ci/check-imports.sh --record <wasm>   rewrite the record from a module
#   bash wasi/ci/check-imports.sh --self-test       prove the check bites
#
# `--record` is a deliberate act with a diff, not a side effect: the record is the
# artifact's auditable claim, and a gate that rewrote its own expectation could never fail on
# a new capability.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
RECORD="$HERE/imports.txt"
MODE="check"
MODULE=""

while [ $# -gt 0 ]; do
  case "$1" in
    --record) MODE="record"; shift ;;
    --self-test) MODE="self-test"; shift ;;
    -h|--help) sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "check-imports: unknown argument '$1'" >&2; exit 2 ;;
    *) MODULE="$1"; shift ;;
  esac
done

recorded() { sed -e 's/#.*//' "$RECORD" | tr -d ' ' | grep -v '^$' | sort; }
actual() { node "$HERE/check-imports.mjs" --list "$1" | sort; }

compare() {
  local module="$1" label="${2:-}"
  local have expected
  have="$(actual "$module")"
  expected="$(recorded)"
  if [ "$have" = "$expected" ]; then
    echo "✓ ${label}import section matches the record ($(printf '%s\n' "$expected" | grep -c . ) item(s))"
    return 0
  fi
  echo "✗ ${label}import section differs from wasi/ci/imports.txt" >&2
  printf '%s\n' "$have" | while IFS= read -r line; do
    [ -n "$line" ] || continue
    printf '%s\n' "$expected" | grep -qxF "$line" || echo "    + $line   not recorded — a new capability, or a stale list" >&2
  done
  printf '%s\n' "$expected" | while IFS= read -r line; do
    [ -n "$line" ] || continue
    printf '%s\n' "$have" | grep -qxF "$line" || echo "    - $line   recorded, but gone — the record is stale" >&2
  done
  return 1
}

case "$MODE" in
  record)
    [ -n "$MODULE" ] || { echo "check-imports --record: a module path is required" >&2; exit 2; }
    [ -f "$MODULE" ] || { echo "check-imports: $MODULE does not exist" >&2; exit 1; }
    body="$(actual "$MODULE")"
    printf '%s\n' "$body" | grep -q . || { echo "check-imports: the module has no recorded entries; write that down deliberately rather than as an empty list" >&2; exit 1; }
    {
      sed -n '1,/^wasi_snapshot/p' "$RECORD" | sed '$d'
      printf '%s\n' "$body"
    } > "$RECORD.tmp"
    mv "$RECORD.tmp" "$RECORD"
    echo "✓ recorded $(printf '%s\n' "$body" | grep -c .) import(s) from $MODULE"
    ;;
  check)
    [ -n "$MODULE" ] || { echo "check-imports: a module path is required" >&2; exit 2; }
    [ -f "$MODULE" ] || { echo "check-imports: $MODULE does not exist" >&2; exit 1; }
    compare "$MODULE"
    ;;
  self-test)
    scratch="$(mktemp -d)"
    trap 'find "$scratch" -type f -delete; find "$scratch" -depth -type d -exec rmdir {} + 2>/dev/null || true' EXIT

    real="$ROOT/target/wasm32-wasip1/release/rigorix_gate_wasi.wasm"
    [ -f "$real" ] || { echo "self-test: build the artifact first (bash wasi/ci/build.sh)" >&2; exit 1; }
    compare "$real" "real module: " || { echo "self-test: the artifact does not match its own record" >&2; exit 1; }

    # The record naming one import more than the module has must fail — otherwise the gate
    # is a comment, and a comment passes everything.
    cp "$RECORD" "$scratch/imports.txt"
    printf 'wasi_snapshot_preview1::sock_open\n' >> "$RECORD"
    if compare "$real" >/dev/null 2>&1; then
      cp "$scratch/imports.txt" "$RECORD"
      echo "self-test: a fabricated import record passed — the check does not bite" >&2
      exit 1
    fi
    cp "$scratch/imports.txt" "$RECORD"
    echo "✓ self-test: a record naming an import the module does not have fails"

    # A record missing one of the module's imports must fail too: the direction that lets a
    # capability in unnoticed is the one that matters.
    grep -v '^wasi_snapshot_preview1::proc_exit$' "$scratch/imports.txt" > "$RECORD"
    if compare "$real" >/dev/null 2>&1; then
      cp "$scratch/imports.txt" "$RECORD"
      echo "self-test: a record missing an import the module has passed — the check does not bite" >&2
      exit 1
    fi
    cp "$scratch/imports.txt" "$RECORD"
    echo "✓ self-test: a record missing one of the module's imports fails"

    # The browser artifact imports `__wbindgen_*`: if this check passes on it, it has no
    # subject and would pass on anything.
    browser="$ROOT/target/wasm32-unknown-unknown/release/rigorix_gate_wasm.wasm"
    if [ -f "$browser" ]; then
      if compare "$browser" "browser module: " >/dev/null 2>&1; then
        echo "self-test: the browser artifact passed the WASI import check — no subject" >&2
        exit 1
      fi
      echo "✓ self-test: the browser artifact fails this check, as it must (it imports __wbindgen_*)"
    else
      echo "  ⊘ not required by this tree: no browser artifact to point the check at"
      echo "    (build it with bash wasm/ci/wasm-build.sh; a skip is named, not counted)"
    fi

    # A module whose import section cannot be parsed is a failure, not a pass.
    printf '\0asm\1\0\0\0' > "$scratch/truncated.wasm"
    if compare "$scratch/truncated.wasm" >/dev/null 2>&1; then
      echo "self-test: an unparseable module passed — the check guesses instead of failing" >&2
      exit 1
    fi
    echo "✓ self-test: an unparseable module fails"
    ;;
esac
