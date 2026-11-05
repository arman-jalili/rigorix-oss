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
# **An expression on `cancel-in-progress` is not enough, and the first attempt proved it:**
# `${{ github.ref != 'refs/heads/main' }}` was carried by run 42057939858 (for 73c376d8) and
# that run was still cancelled by the next merge's run (for 70763393). The value is a
# stringified boolean, and non-empty text is true. The load-bearing mechanism is therefore the
# **group**: on `main` it must be keyed by the commit, so each merge lands in a group of its
# own and there is nothing older in it to cancel. Pull requests keep one group per ref, which
# is what makes a superseded run stale.
#
# This script checks both keys and fails when `main` is not commit-keyed. `--self-test` plants
# each defect in a temporary workflow and shows the check fires, naming file and line.
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

# `key_line <file> <block-start> <key>` — the line number of a key inside the block.
key_line() {
    local file="$1" start="$2" key="$3"
    awk -v s="$start" -v k="$key" '
        NR <= s { next }
        /^#/ { next }
        /^[^[:space:]#]/ { exit }
        index($0, k) > 0 { print NR; exit }
    ' "$file"
}

value_of() {
    local file="$1" line="$2" key="$3"
    sed -n "${line}p" "$file" \
        | sed -E "s/^[[:space:]]*${key}:[[:space:]]*//; s/[[:space:]]+$//"
}

check() {
    local file="${1:-$WORKFLOW}"
    local block group_line cancel_line_no group value

    if [ ! -f "$file" ]; then
        echo "❌ no workflow: $file" >&2
        return 1
    fi

    block=$(block_start "$file")
    if [ -z "$block" ]; then
        echo "❌ $file: no top-level \`concurrency:\` — the behaviour is the default, which nobody chose and nobody stated" >&2
        return 1
    fi

    group_line=$(key_line "$file" "$block" "group:")
    cancel_line_no=$(key_line "$file" "$block" "cancel-in-progress:")
    if [ -z "$group_line" ] || [ -z "$cancel_line_no" ]; then
        echo "❌ $file:$block: the concurrency block needs both \`group:\` and \`cancel-in-progress:\`" >&2
        echo "   a value that is not written down is a value the next session cannot check" >&2
        return 1
    fi
    group=$(value_of "$file" "$group_line" "group")
    value=$(value_of "$file" "$cancel_line_no" "cancel-in-progress")

    # (1) The mechanism: on `main`, each merge must land in a group of its own. A group keyed
    #     only by the ref puts every merge of `main` in one group, and the next one cancels
    #     the last — which is #1422, whatever `cancel-in-progress` says (run 42057939858 kept
    #     the expression and was cancelled anyway).
    case "$group" in
        *github.sha* | *github.run_id*)
            case "$group" in
                *refs/heads/main* | *github.ref*)
                    : ;;
                *)
                    echo "❌ $file:$group_line: the group is per-commit but does not name the ref" >&2
                    echo "   pull requests would share a group and cancel each other's re-runs" >&2
                    return 1
                    ;;
            esac
            ;;
        *)
            echo "❌ $file:$group_line: group '$group' is not keyed by the commit" >&2
            echo "   every merge on refs/heads/main shares this group, so the next merge cancels the last (#1422)" >&2
            echo "   key it by github.sha on main: \${{ github.ref == 'refs/heads/main' && format('-{0}', github.sha) || '' }}" >&2
            return 1
            ;;
    esac

    # (2) `cancel-in-progress` inside a per-commit group is correct: it can only cancel a
    #     re-run of the same commit. On a pull request it is what makes a superseded run stale.
    case "$value" in
        true | True | TRUE) : ;;
        *)
            echo "❌ $file:$cancel_line_no: cancel-in-progress is '$value'" >&2
            echo "   with a per-commit group, \`true\` cancels only a re-run of the same commit; a boolean expression here was measured to cancel main anyway" >&2
            return 1
            ;;
    esac

    echo "✅ $file:$group_line group is per-commit on main; $file:$cancel_line_no cancel-in-progress: $value"
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

    # S1: the literal `true` with a per-ref group — #1422 exactly as it was found.
    cat > "$tmp/shared-group.yml" <<'YAML'
name: CI
on: [push]
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
jobs:
  x:
    runs-on: ubuntu-latest
YAML

    # S2: the fix.
    cat > "$tmp/per-commit.yml" <<'YAML'
name: CI
on: [push]
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}${{ github.ref == 'refs/heads/main' && format('-{0}', github.sha) || '' }}
  cancel-in-progress: true
jobs:
  x:
    runs-on: ubuntu-latest
YAML

    # S3: the expression that was tried first and did not hold (run 42057939858). A per-ref
    #     group with an exempting expression is still one group per merge on main.
    cat > "$tmp/expression-only.yml" <<'YAML'
name: CI
on: [push]
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: ${{ github.ref != 'refs/heads/main' }}
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

    run_case fail "$tmp/shared-group.yml" "$tmp/shared-group.yml:4" "a per-ref group shares main's merges, so they cancel each other" \
        || failures=$((failures + 1))
    run_case pass "$tmp/per-commit.yml" "always gets a run of its own" "a per-commit group on main passes" \
        || failures=$((failures + 1))
    run_case fail "$tmp/expression-only.yml" "is not keyed by the commit" "an exempting expression with a per-ref group still fails" \
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
    echo "✅ concurrency: main's group is per-commit, pull requests cancel (4 seeds)"
    return 0
}

if [ "${1:-}" = "--self-test" ]; then
    self_test
else
    check "${1:-$WORKFLOW}"
fi
