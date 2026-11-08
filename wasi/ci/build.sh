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

# ── the artifact must not ship the builder's filesystem (#1474) ────────────────────
#
# A release artifact that contains `/Users/<someone>` is reproducible on exactly one machine,
# and leaks the builder's layout to whoever downloads it. The sibling script states the rule and
# the reason the sysroot itself is remapped rather than its parent; this build copies it because
# the two artifacts are built by different commands and the second one must not be the exception.
: "${CARGO_HOME:=$HOME/.cargo}"
: "${RUSTUP_HOME:=$HOME/.rustup}"
RUST_SYSROOT="$(rustc --print sysroot 2>/dev/null || true)"
REMAP="--remap-path-prefix=${ROOT}=/build --remap-path-prefix=${CARGO_HOME}/registry=/registry --remap-path-prefix=${RUSTUP_HOME}/toolchains=/rustc"
if [ -n "${RUST_SYSROOT:-}" ]; then
  # The longest prefix wins, so a normal rustup layout keeps the sysroot mapping.
  REMAP="${REMAP} --remap-path-prefix=${RUST_SYSROOT}=/rustc"
fi
# Paths also arrive through `--extern` arguments and debug info, so the release profile is set
# here rather than in a manifest a non-root package cannot own.
export CARGO_PROFILE_RELEASE_DEBUG=0
export CARGO_PROFILE_RELEASE_STRIP=debuginfo

if ! rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
  echo "build: the $TARGET target is not installed" >&2
  echo "       rustup target add $TARGET" >&2
  exit 1
fi

cd "$ROOT"
RUSTFLAGS="${RUSTFLAGS:-} -C panic=abort ${REMAP}" cargo build -p rigorix-gate-wasi --release --target "$TARGET"
mkdir -p "$OUT_DIR"
test -f "$OUT_DIR/$ARTIFACT" || { echo "build: $OUT_DIR/$ARTIFACT was not produced" >&2; exit 1; }

# The debug symbol table is a custom `name` section: no host needs it, and it is where the
# per-file paths live. The browser packaging path strips it for the same reason; the strip is a
# section walk, so it is deterministic and the output depends only on the input bytes.
python3 "$ROOT/wasm/ci/strip_name_section.py" "$OUT_DIR/$ARTIFACT"
echo "✓ $OUT_DIR/$ARTIFACT ($(wc -c < "$OUT_DIR/$ARTIFACT" | tr -d ' ') bytes)"
