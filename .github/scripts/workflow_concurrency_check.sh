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
#     landed on `main` with no run that reached a conclusion because the value was the literal
#     `true` (#1422); the first run allowed to finish found #1421.
#
# **Neither the literal nor an expression fixed it, and both were measured:**
#
#   · `${{ github.ref != 'refs/heads/main' }}` — run 73c376d8 carried it and was still cancelled
#     by the next merge's run (70763393): the value stringifies to "false", and non-empty text
#     is true.
#   · a group keyed by the commit — run 42079428759 (2dcfc933) was cancelled 108 s after
#     42087059012 (9fecc28f) was created, unattributed by the run's record.
#
# So the check no longer validates a comparison. It validates the property that makes the
# mechanism hold: **the group is unique per run unless the run belongs to a pull request**. A
# group with one member has nothing older to cancel; a group per PR is what makes a superseded
# run stale. `github.run_id` cannot be empty and cannot repeat, so there is no branch of the
# expression that can collapse two runs together.
#
# `--self-test` plants each rejected shape and the required one, and shows the verdicts.
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

    # (1) A commit key is refused first, so a reader who reintroduces that exact shape gets the
    #     reason it was withdrawn: it was measured, not argued (run 42079428759), and it is the
    #     shape that made this issue look solved when it was not.
    case "$group" in
        *github.sha*)
            echo "❌ $file:$group_line: group '$group' is keyed by the commit" >&2
            echo "   that shape was measured not to hold: run 42079428759 was cancelled by the next merge's run" >&2
            echo "   use '\${{ github.event.pull_request.number || github.run_id }}'" >&2
            return 1
            ;;
    esac

    # (2) Uniqueness, unconditionally available. Without `github.run_id` there is no branch of
    #     the expression that is guaranteed distinct for two runs on the same ref, and that is
    #     precisely the branch `main` takes.
    case "$group" in
        *github.run_id*) : ;;
        *)
            echo "❌ $file:$group_line: group '$group' has no unconditional unique part" >&2
            echo "   two merges on refs/heads/main can share this group, and the next one cancels the last (#1422)" >&2
            echo "   key it by '\${{ github.event.pull_request.number || github.run_id }}'" >&2
            return 1
            ;;
    esac

    # (3) A pull request must still land in one group per PR, not one per run: that is the half
    #     of the behaviour that cancels a superseded commit.
    case "$group" in
        *pull_request.number*) : ;;
        *)
            echo "❌ $file:$group_line: group '$group' is per run for pull requests too" >&2
            echo "   a superseded commit would queue behind the one it replaces instead of cancelling it" >&2
            return 1
            ;;
    esac

    # (4) With the group unique, `true` is correct: inside a group of one it cancels nothing,
    #     and on a PR it cancels the run the new commit supersedes.
    case "$value" in
        true | True | TRUE) : ;;
        *)
            echo "❌ $file:$cancel_line_no: cancel-in-progress is '$value'" >&2
            echo "   with a group that is unique per run on main, \`true\` is the correct value" >&2
            return 1
            ;;
    esac

    echo "✅ $file:$group_line group is unique per run; $file:$cancel_line_no cancel-in-progress: $value"
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

    # S1: one group per ref — #1422 as it was found. The next merge cancels the last.
    cat > "$tmp/per-ref.yml" <<'YAML'
name: CI
on: [push]
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
jobs:
  x:
    runs-on: ubuntu-latest
YAML

    # S2: the required shape.
    cat > "$tmp/per-run.yml" <<'YAML'
name: CI
on: [push]
concurrency:
  group: ${{ github.workflow }}-${{ github.event.pull_request.number || github.run_id }}
  cancel-in-progress: true
jobs:
  x:
    runs-on: ubuntu-latest
YAML

    # S3: the commit key that was measured not to hold (42079428759).
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

    # S4: no concurrency at all — a value nobody wrote is a value nobody checks.
    cat > "$tmp/absent.yml" <<'YAML'
name: CI
on: [push]
jobs:
  x:
    runs-on: ubuntu-latest
YAML

    run_case fail "$tmp/per-ref.yml" "has no unconditional unique part" \
        "a per-ref group shares main's merges, so they cancel each other" || failures=$((failures + 1))
    run_case pass "$tmp/per-run.yml" "always gets a run of its own" \
        "a group unique per run passes" || failures=$((failures + 1))
    run_case fail "$tmp/per-commit.yml" "is keyed by the commit" \
        "the commit key that was measured not to hold fails" || failures=$((failures + 1))
    run_case fail "$tmp/absent.yml" "no top-level" \
        "a workflow with no concurrency fails" || failures=$((failures + 1))

    # And then the file this script exists for, so the seeds cannot pass while the real
    # workflow is the thing they describe.
    check "$WORKFLOW" || failures=$((failures + 1))

    if [ "$failures" -ne 0 ]; then
        echo "❌ $failures concurrency check(s) failed"
        return 1
    fi
    echo "✅ concurrency: the group is unique per run, a PR keeps one group per PR (4 seeds)"
    return 0
}

if [ "${1:-}" = "--self-test" ]; then
    self_test
else
    check "${1:-$WORKFLOW}"
fi
