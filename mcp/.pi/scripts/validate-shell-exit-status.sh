#!/usr/bin/env bash
# ============================================================================
# validate-shell-exit-status.sh — arithmetic commands must not end an errexit
# script, or a function that errexit script calls (#1291)
#
# `(( X++ ))` returns status 1 when the result is 0, so the **first** increment
# of a counter that starts at 0 fails. Whether that ends the script depends on
# the shell:
#
#   set -e; x=0; ((x++)); echo survived
#     bash 3.2.57 (macOS)  → exit 0, prints "survived"
#     bash 4.4 / 5.x       → exit 1, prints nothing
#
# Two shapes are caught:
#   1. a standalone arithmetic command in a script that enables errexit
#   2. the same command as the tail of a one-line function body — there the
#      *function* returns 1, which errexit treats as a failed command on **every**
#      shell, bash 3.2 included: `log_pass() { echo ok; ((PASS++)); }` has been
#      ending stage scripts on macOS too.
#
# The fix is the assignment form, `X=$((X + 1))`, which always returns 0 and
# does not swallow a real failure (unlike `((x++)) || true`).
#
# Usage: bash .pi/scripts/validate-shell-exit-status.sh [root]
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEFAULT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# One arithmetic command: (( name++ )) or (( name-- )), whitespace-tolerant.
ARITH='\(\([[:space:]]*[A-Za-z_][A-Za-z0-9_]*[[:space:]]*(\+\+|--)[[:space:]]*\)\)'
# The whole line is that command (optionally with a trailing `;` and comment).
STANDALONE_RE="^[[:space:]]*${ARITH}[[:space:]]*;?[[:space:]]*(#.*)?$"
# ... or it is the last command of a one-line function body: f() { ...; ((x++)); }
FUNC_TAIL_RE="^[[:space:]]*[A-Za-z_][A-Za-z0-9_]*[[:space:]]*\(\)[[:space:]]*\{.*;[[:space:]]*${ARITH}[[:space:]]*;[[:space:]]*\}[[:space:]]*(#.*)?$"
# A file is in scope when it enables errexit (`set -e…` or `set -o errexit`).
ERREXIT_RE='^[[:space:]]*set[[:space:]]+-[a-zA-Z]*e|set[[:space:]]+-o[[:space:]]+errexit'

# ---------------------------------------------------------------------------
# Scanning
# ---------------------------------------------------------------------------

enables_errexit() {
    grep -Eq "$ERREXIT_RE" "$1" 2> /dev/null
}

# Candidate shell scripts under a root. Build trees are pruned (a `-not -path`
# filter still descends into them — measured 8.6s per find in this tree).
list_scripts() {
    local root="$1"
    {
        find "$root" \
            \( -name target -o -name node_modules -o -name .git -o -name .venv \
            -o -name graphify-out -o -name vendor \) -prune -o \
            -type f -name '*.sh' -print 2> /dev/null
        if [ -d "$root/.pi/scripts" ]; then
            find "$root/.pi/scripts" -type f -print 2> /dev/null
        fi
    } | sort -u
}

# Print `path:line` for every offending command. Always exits 0 — the caller
# counts the lines (a nonzero return would trip `pipefail` in the callers below).
scan() {
    local root="${1:-$DEFAULT_ROOT}" file hits line
    while IFS= read -r file; do
        enables_errexit "$file" || continue
        # This file carries the self-test fixtures as text, so it would match
        # itself; the fixtures are the point.
        case "${file##*/}" in
            validate-shell-exit-status.sh) continue ;;
        esac
        # grep -n prints `line:text`; the text may contain `:` so split once.
        hits=$(grep -nE "$STANDALONE_RE|$FUNC_TAIL_RE" "$file" 2> /dev/null || true)
        [ -n "$hits" ] || continue
        while IFS= read -r hit; do
            [ -n "$hit" ] || continue
            line="${hit%%:*}"
            echo "  ✗ ${file}:${line}: arithmetic command under errexit — use X=\$((X + 1))"
        done <<< "$hits"
    done < <(list_scripts "$root")
    return 0
}

# ---------------------------------------------------------------------------
# Self-test — the guard is proven before it is trusted
# ---------------------------------------------------------------------------

self_test() {
    local tmp failures=0
    tmp="$(mktemp -d)"
    # shellcheck disable=SC2064
    trap "rm -rf '$tmp'" RETURN

    write_fixture() { # name, body
        printf '%s\n' "$2" > "$tmp/$1"
    }

    # Positive: a standalone increment in an errexit script.
    write_fixture "positive_standalone.sh" '#!/usr/bin/env bash
set -euo pipefail
n=0
((n++))
echo done'
    # Positive: the same command as a one-line function tail (fails on every shell).
    write_fixture "positive_func_tail.sh" '#!/usr/bin/env bash
set -e
log() { echo ok; ((n++)); }
log'
    # Positive: the command is the last line of the file.
    write_fixture "positive_last_line.sh" '#!/usr/bin/env bash
set -e
n=0
((n++))'
    # Negative: guarded uses that are not failed commands.
    write_fixture "negative_guarded.sh" '#!/usr/bin/env bash
set -e
n=0
((n++)) || true
((n++)) && echo ok
if (( n++ )); then echo yes; fi
while (( n-- )); do break; done
n=$((n + 1))
echo "$((n++))"'
    # Negative: assignment form, which is the fix.
    write_fixture "negative_fixed.sh" '#!/usr/bin/env bash
set -e
n=0
n=$((n + 1))
log() { echo ok; n=$((n + 1)); }
log'
    # Negative: a script that does not enable errexit.
    write_fixture "negative_no_errexit.sh" '#!/usr/bin/env bash
n=0
((n++))
((n++))
echo done'

    expect() { # expected_count, fixture
        local want="$1" name="$2" got
        got=$(scan "$tmp" 2> /dev/null | grep -c "$name" || true)
        if [ "$got" != "$want" ]; then
            echo "  ✗ SELF-TEST: $name expected $want violation(s), got $got"
            failures=$((failures + 1))
        fi
    }

    expect 1 positive_standalone.sh
    expect 1 positive_func_tail.sh
    expect 1 positive_last_line.sh
    expect 0 negative_guarded.sh
    expect 0 negative_fixed.sh
    expect 0 negative_no_errexit.sh

    # Positive fixtures must be caught, negative ones must be clean — and the
    # scan must report the path *and* the line, or the guard cannot point at
    # the fix.
    local report
    report="$(scan "$tmp" 2> /dev/null)"
    if ! printf '%s\n' "$report" | grep -q 'positive_standalone.sh:4'; then
        echo "  ✗ SELF-TEST: expected path:line 'positive_standalone.sh:4' in the report"
        failures=$((failures + 1))
    fi

    if [ "$failures" -gt 0 ]; then
        echo "❌ Shell exit-status guard self-test FAILED (${failures} case(s))"
        return 1
    fi
    echo "  ✓ Self-test: 6 fixture cases + path:line reporting"
    return 0
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

echo "Shell exit-status guard (errexit arithmetic) — #1291"

if ! self_test; then
    exit 1
fi

ROOT="${1:-$DEFAULT_ROOT}"
echo "  Scanning: ${ROOT}"

report="$(scan "$ROOT")"
if [ -n "$report" ]; then
    echo "$report"
fi
status=$(printf '%s' "$report" | grep -c '✗' || true)

if [ "$status" -gt 0 ]; then
    echo ""
    echo "❌ ${status} arithmetic command(s) can end an errexit script."
    echo "   Replace \`(( X++ ))\` with \`X=\$((X + 1))\` (and \`(( X-- ))\` with \`X=\$((X - 1))\`)."
    echo "   \`(( X++ )) || true\` is not a fix: it suppresses every failure, not just this one."
    exit 1
fi

echo "  ✅ No arithmetic command can end an errexit script"
exit 0
