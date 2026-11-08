#!/usr/bin/env bash
# build.sh — the one command that produces the artifact.
#
#   bash wasi/ci/build.sh [--release-dir <dir>]
#
# Why a script rather than a line in a workflow: the target, the panic strategy and the
# output name are the artifact's identity, and a workflow that spells them out is a second
# copy of that identity. It also means the same command runs in the release job and in the
# gate, so "it built on my machine" and "it built for the release" are one statement.
#
# `-C panic=abort` and not a `[profile.release]` block: Cargo ignores a profile in a
# non-root package (it warns), and a setting that is silently ignored is worse than one that
# is absent. A trap is not a refusal (ADR-012), so there is no unwinding to trap on.
set -euo pipefail

TARGET="wasm32-wasip1"
ARTIFACT="rigorix_gate_wasi.wasm"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="$ROOT/target/$TARGET/release"

while [ $# -gt 0 ]; do
  case "$1" in
    --release-dir)
      [ $# -ge 2 ] || { echo "build: --release-dir needs a directory" >&2; exit 2; }
      OUT_DIR="$2"; shift 2 ;;
    -h|--help) sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "build: unknown argument '$1'" >&2; exit 2 ;;
  esac
done

if ! rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
  echo "build: the $TARGET target is not installed" >&2
  echo "       rustup target add $TARGET" >&2
  exit 1
fi

cd "$ROOT"
RUSTFLAGS="${RUSTFLAGS:-} -C panic=abort" cargo build -p rigorix-gate-wasi --release --target "$TARGET"
mkdir -p "$OUT_DIR"
test -f "$OUT_DIR/$ARTIFACT" || { echo "build: $OUT_DIR/$ARTIFACT was not produced" >&2; exit 1; }
echo "✓ $OUT_DIR/$ARTIFACT ($(wc -c < "$OUT_DIR/$ARTIFACT" | tr -d ' ') bytes)"
