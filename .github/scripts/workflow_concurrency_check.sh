#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# main never cancels its own verification (#1422).
#
# `ci.yml` carries a `concurrency.cancel-in-progress` value, and its meaning is not the same
# on a pull request as it is on `refs/heads/main`.
#
#   · On a pull request, `true` is correct: a new commit supersedes the old one, so the old
#     run's verdict is stale and the runner time is wasted.
#   · On `refs/heads/main` the meaning inverts. Each merge *adds* a commit that must be
#     verified, and the next merge cancels the verification of the one before it. Five merges
#     landed on `main` with no run that reached a conclusion because the value was the
#     literal `true` (#1422); the first run allowed to finish found #1421.
#
# So the value must be an expression, and it must exempt `refs/heads/main`. This script is
# what makes that a check rather than a comment: the literal fails, naming the file and the
# line, and `--self-test` plants the literal in a temporary workflow to show the check fires.
#
# Advisory workflows keep `cancel-in-progress: true` on purpose (`hardening.yml:42-44`): they
# are not a verdict on a commit, and their cancellation is therefore not one either. This
# script checks the merge gate, not every workflow in the directory.
#
# Usage:
#   bash .github/scripts/workflow_concurrency_check.sh              # check the repo's ci.yml
#   bash .github/scripts/workflow_concurrency_check.sh --self-test  # check the check, with seeds
#   bash .github/scripts/workflow_concurrency_check.sh <workflow>   # check one file
# ─────────────────────────────────────────────────────────────────────
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORKFLOW="$REPO_ROOT/.github/workflows/ci.yml"

# The line numbers of the top-level `concurrency:` block and its `cancel-in-progress:` key.
# Line-based on purpose, like `ci_graph_check.sh`: the workflow is the input, and a YAML
# parser would make this check depend on a dependency it does not need.
block_start() {
    grep -n '^concurrency:' "$1" 2> /dev/null | head -1 | cut -d: -f1
}

cancel_line() {
    local file="$1" start="$2"
    awk -v s="$start" '
        NR <= s { next }
        /^#/ { next }
        /^[^[:space:]#]/ { exit }
        /^[[:space:]]*cancel-in-progress:/ { print NR; exit }
    ' "$file"
}

check() {
    local file="${1:-$WORKFLOW}"
    local start line value

    if [ ! -f "$file" ]; then
        echo "❌ no workflow: $file" >&2
        return 1
    fi

    start=$(block_start "$file")
    if [ -z "$start" ]; then
        echo "❌ $file: no top-level \`concurrency:\` — the behaviour is the default, which nobody chose and nobody stated" >&2
        return 1
    fi

    line=$(cancel_line "$file" "$start")
    if [ -z "$line" ]; then
        echo "❌ $file:$start: the concurrency block declares no \`cancel-in-progress:\`" >&2
        echo "   a value that is not written down is a value the next session cannot check" >&2
        return 1
    fi

    value=$(sed -n "${line}p" "$file" \
        | sed -E 's/^[[:space:]]*cancel-in-progress:[[:space:]]*//; s/[[:space:]]+$//')

    # (1) The literal was the defect. It cannot distinguish a PR from main, whichever value
    #     it holds, so both literals are refused: `true` cancels main's merges, `false` makes
    #     every PR run queue behind the one it supersedes.
    case "$value" in
        true | True | TRUE | false | False | FALSE)
            echo "❌ $file:$line: cancel-in-progress is the literal '$value'" >&2
            echo "   on refs/heads/main, \`true\` cancels the verification of the merge before each merge (#1422)" >&2
            echo "   use \${{ github.ref != 'refs/heads/main' }}: a pull request still cancels, main never does" >&2
            return 1
            ;;
    esac

    # (2) An expression, because only an expression can read the ref.
    case "$value" in
        *'${{'*'}}'*) : ;;
        *)
            echo "❌ $file:$line: cancel-in-progress is '$value', which is not an expression" >&2
            echo "   only an expression can tell a pull request from refs/heads/main" >&2
            return 1
            ;;
    esac

    # (3) The exemption, named. An expression that does not mention main has not stated the
    #     asymmetry, and a reader cannot tell whether the omission was deliberate.
    case "$value" in
        *refs/heads/main*) : ;;
        *)
            echo "❌ $file:$line: '$value' does not exempt refs/heads/main" >&2
            echo "   main's merges would still cancel each other's verification (#1422)" >&2
            return 1
            ;;
    esac

    echo "✅ $file:$line cancel-in-progress: $value"
    echo "   a pull request replaces its predecessor; a merge on main always gets a run of its own"
    return 0
}

# One seed: `expect` is pass or fail, and `needle` is text the output must contain (the file
# and line, for the failure cases — a failure that does not name its subject is a rumour).
run_case() {
    local expect="$1" file="$2" needle="$3" label="$4" out rc
    out=$(check "$file" 2>&1)
    rc=$?
    if [ "$expect" = "pass" ] && [ "$rc" -ne 0 ]; then
        echo "❌ seed did not pass: $label"
        printf '%s\n' "$out" | sed 's/^/     /'
        return 1
    fi
    if [ "$expect" = "fail" ] && [ "$rc" -eq 0 ]; then
        echo "❌ seed did not fire: $label — a check that cannot fail is not a check"
        printf '%s\n' "$out" | sed 's/^/     /'
        return 1
    fi
    if ! printf '%s' "$out" | grep -qF "$needle"; then
        echo "❌ seed fired without naming '$needle': $label"
        printf '%s\n' "$out" | sed 's/^/     /'
        return 1
    fi
    echo "✅ seed: $label"
    return 0
}

self_test() {
    local tmp failures=0
    tmp=$(mktemp -d)
    # shellcheck disable=SC2064
    trap "rm -rf '$tmp'" EXIT

    # S1: the literal, exactly as #1422 found it. `cancel-in-progress` is line 5.
    cat > "$tmp/literal.yml" <<'YAML'
name: CI
on: [push]
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
jobs:
  x:
    runs-on: ubuntu-latest
YAML

    # S2: the value the workflow is supposed to carry.
    cat > "$tmp/exempting.yml" <<'YAML'
name: CI
on: [push]
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: ${{ github.ref != 'refs/heads/main' }}
jobs:
  x:
    runs-on: ubuntu-latest
YAML

    # S3: an expression that never looks at main — the asymmetry is unstated.
    cat > "$tmp/ref-agnostic.yml" <<'YAML'
name: CI
on: [push]
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}
jobs:
  x:
    runs-on: ubuntu-latest
YAML

    # S4: no concurrency at all — a value nobody wrote is a value nobody checks.
    cat > "$tmp/absent.yml" <<'YAML'
name: CI
on: [push]
jobs:
  x:
    runs-on: ubuntu-latest
YAML

    run_case fail "$tmp/literal.yml" "$tmp/literal.yml:5" "the literal 'true' fails, naming file and line" \
        || failures=$((failures + 1))
    run_case pass "$tmp/exempting.yml" "always gets a run of its own" "the exempting expression passes" \
        || failures=$((failures + 1))
    run_case fail "$tmp/ref-agnostic.yml" "does not exempt refs/heads/main" "an expression that ignores main fails" \
        || failures=$((failures + 1))
    run_case fail "$tmp/absent.yml" "no top-level" "a workflow with no concurrency fails" \
        || failures=$((failures + 1))

    # And then the file this script exists for, so the seeds cannot pass while the real
    # workflow is the thing they describe.
    check "$WORKFLOW" || failures=$((failures + 1))

    if [ "$failures" -ne 0 ]; then
        echo "❌ $failures concurrency check(s) failed"
        return 1
    fi
    echo "✅ concurrency: main never cancels, pull requests do (4 seeds)"
    return 0
}

if [ "${1:-}" = "--self-test" ]; then
    self_test
else
    check "${1:-$WORKFLOW}"
fi
