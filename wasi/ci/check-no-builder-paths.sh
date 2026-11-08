#!/usr/bin/env bash
# check-no-builder-paths.sh — the artifact must not ship the builder's filesystem (#1474).
#
#   bash wasi/ci/check-no-builder-paths.sh [<module.wasm>]
#   bash wasi/ci/check-no-builder-paths.sh --self-test
#
# # Why this is a release blocker and not a tidy-up
#
# The first build shipped **108** absolute paths, including the builder's home. An artifact like
# that is reproducible on exactly one machine, and it hands whoever downloads it a map of the
# machine it was built on. The fix is two halves: remap the path prefixes at compile time (so the
# bytes never contain them) and strip the `name` custom section (the debug symbol table, which is
# where per-file paths live). Both are in `build.sh`, which is why this gate asserts the *output*:
# a flag that is set and a binary that still leaks are different claims.
#
# # The four strings that must be absent
#
# `$ROOT`, `$HOME`, `$CARGO_HOME`, `$RUSTUP_HOME` — the four roots a compile can embed. The
# sysroot is covered by the remap of `$RUSTUP_HOME/toolchains` and of the sysroot itself; if it
# ever survives, it survives as a suffix of `$HOME` and is caught by the same scan.
#
# # The one legitimate `/Users/`
#
# The artifact contains the *content policy* — the pattern list a scanner uses to find secrets,
# PII and home paths. That list contains `"/Users/"` as a literal, on one line that also carries
# `"/home/"`, `/root`, `/private/var` and a Windows drive. So the rule is not "no `/Users/`
# anywhere": it is that **every** remaining `/Users/` occurrence must be on such a policy line.
# Any other is a builder path that survived, and it fails with its own line printed.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
MODULE="${1:-$ROOT/target/wasm32-wasip1/release/rigorix_gate_wasi.wasm}"
SELF_TEST=0
[ "${1:-}" = "--self-test" ] && { SELF_TEST=1; MODULE="$ROOT/target/wasm32-wasip1/release/rigorix_gate_wasi.wasm"; }

: "${CARGO_HOME:=$HOME/.cargo}"
: "${RUSTUP_HOME:=$HOME/.rustup}"

# Count occurrences of a literal in the module's printable strings.
count_of() { strings -a "$1" | grep -cF -- "$2" || true; }

scan() {
  local module="$1" failures=0 needle count
  if [ ! -f "$module" ]; then
    echo "✗ $module does not exist (bash wasi/ci/build.sh)" >&2
    return 1
  fi

  for needle in "$ROOT" "$HOME" "$CARGO_HOME" "$RUSTUP_HOME"; do
    [ -n "$needle" ] || continue
    count="$(count_of "$module" "$needle")"
    if [ "$count" -ne 0 ]; then
      echo "✗ the artifact contains the builder path $needle ($count occurrence(s))" >&2
      strings -a "$module" | grep -F -- "$needle" | head -3 | sed 's/^/    /' >&2
      failures=$((failures + 1))
    fi
  done

  # Every `/Users/` left must be the content policy's pattern, not a file path.
  local stray
  stray="$(strings -a "$module" | grep -F '/Users/' | grep -vF '/home/' || true)"
  if [ -n "$stray" ]; then
    echo "✗ the artifact contains a /Users/ path that is not the content policy's pattern:" >&2
    printf '%s\n' "$stray" | head -5 | sed 's/^/    /' >&2
    failures=$((failures + 1))
  fi

  if [ "$failures" -eq 0 ]; then
    echo "✓ no builder path in the artifact (root, home, cargo, rustup); the only /Users/ is the content policy's pattern"
  fi

  # The symbol table is where per-file paths live, so "no name section" is part of the claim.
  if ! python3 "$ROOT/wasm/ci/strip_name_section.py" --check "$module"; then
    failures=$((failures + 1))
  fi

  return "$failures"
}

if [ "$SELF_TEST" -eq 0 ]; then
  scan "$MODULE" || { echo "❌ builder paths: the artifact ships its build machine" >&2; exit 1; }
  exit 0
fi

# ── self-test: the scan must be able to fail ──────────────────────────────────────
scratch="$(mktemp -d)"
trap 'find "$scratch" -type f -delete; find "$scratch" -depth -type d -exec rmdir {} + 2>/dev/null || true' EXIT
scan "$MODULE" >/dev/null || { echo "self-test: the real artifact does not pass its own gate" >&2; exit 1; }
echo "✓ self-test: the real artifact passes"

# A module carrying the builder's root: the defect this gate exists for.
printf '%s\n' "$ROOT/target/wasm32-wasip1/release/libsomething.rlib" > "$scratch/leak.txt"
cp "$MODULE" "$scratch/leaked-root.wasm"
cat "$scratch/leak.txt" >> "$scratch/leaked-root.wasm"
if scan "$scratch/leaked-root.wasm" >/dev/null 2>&1; then
  echo "self-test: an artifact containing $ROOT passed — the gate does not bite" >&2
  exit 1
fi
echo "✓ self-test: an artifact containing the builder's root fails, naming it"

cp "$MODULE" "$scratch/leaked-home.wasm"
printf '%s\n' "$HOME/.rustup/toolchains/x/lib/rustlib/src" >> "$scratch/leaked-home.wasm"
if scan "$scratch/leaked-home.wasm" >/dev/null 2>&1; then
  echo "self-test: an artifact containing \$HOME passed" >&2
  exit 1
fi
echo "✓ self-test: an artifact containing the builder's home fails"

cp "$MODULE" "$scratch/leaked-user.wasm"
printf '%s\n' "/Users/someone-else/project/src/lib.rs" >> "$scratch/leaked-user.wasm"
if scan "$scratch/leaked-user.wasm" >/dev/null 2>&1; then
  echo "self-test: an artifact containing someone else's home path passed" >&2
  exit 1
fi
echo "✓ self-test: another user's path fails too (the gate is about paths, not about me)"

# The flags that make the scan pass must actually be in the build, or the gate is green for a
# reason nobody can see. Stated as a source assertion rather than a mutation build, and named.
for flag in '--remap-path-prefix=${ROOT}=/build' '--remap-path-prefix=${CARGO_HOME}/registry=/registry' 'strip_name_section.py'; do
  if grep -qF -- "$flag" "$HERE/build.sh"; then
    echo "✓ self-test: build.sh sets $flag"
  else
    echo "self-test: build.sh no longer sets '$flag' — the artifact would leak again" >&2
    exit 1
  fi
done

echo
echo "✅ builder-path self-test: 6 seed(s), each one bites"
