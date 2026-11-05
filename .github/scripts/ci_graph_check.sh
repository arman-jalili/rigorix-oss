#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# The CI dependency graph, checked (#1411).
#
# `.github/ci-graph.tsv` is the source. This script is what makes it the source rather than
# a description: a workflow edge with no row fails, a row with no edge fails, a `needs` list
# that disagrees with its row fails, and any `needs` naming `lint` fails with the reason
# (#1411 — one edge hid seven stages).
#
# Usage:
#   bash .github/scripts/ci_graph_check.sh              # check the repo's workflow
#   bash .github/scripts/ci_graph_check.sh --self-test  # check the check, with seeds
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GRAPH="$REPO_ROOT/.github/ci-graph.tsv"
WORKFLOW="$REPO_ROOT/.github/workflows/ci.yml"

# Print `<job>\t<needs>` per job in `jobs:`. Line-based on purpose: the workflow is the
# input, and a YAML library would make this check depend on a parser it does not need.
workflow_edges() {
    awk '
        /^jobs:/ { injobs = 1; next }
        injobs && /^[^[:space:]#]/ { injobs = 0 }
        injobs && /^  [A-Za-z0-9_-]+:[[:space:]]*$/ {
            job = substr($0, 3)
            sub(/:[[:space:]]*$/, "", job)
            order[++n] = job
            needs[job] = "-"
            next
        }
        injobs && /^    needs:/ {
            line = $0
            sub(/^    needs:[[:space:]]*/, "", line)
            gsub(/[][[:space:]]/, "", line)
            needs[job] = line
        }
        END { for (i = 1; i <= n; i++) printf "%s\t%s\n", order[i], needs[order[i]] }
    ' "$1"
}

# Print `<stage>\t<needs>` from the graph, comments and blanks dropped.
graph_rows() {
    awk -F'\t' '!/^#/ && NF >= 2 { gsub(/[[:space:]]/, "", $1); gsub(/[[:space:]]/, "", $2); print $1 "\t" $2 }' "$1"
}

check() {
    local workflow="${1:-$WORKFLOW}" graph="${2:-$GRAPH}"
    local failures=0

    if [ ! -f "$graph" ]; then
        echo "❌ no graph: $graph — a workflow with no declared shape is a shape nobody agreed to" >&2
        return 1
    fi
    if [ ! -f "$workflow" ]; then
        echo "❌ no workflow: $workflow" >&2
        return 1
    fi

    local wf gr
    wf="$(workflow_edges "$workflow")"
    gr="$(graph_rows "$graph")"

    # 1. `lint` is a leaf for every *stage*. A stage that does work must not inherit a lint
    #    finding: that edge once skipped the build and everything downstream, so seven
    #    subjects never ran and the gate accused five of them of failing (#1411).
    #
    #    The two aggregators are the exception, and they are the only one: their subject is
    #    every stage's result, so they must name every stage. The set is written here rather
    #    than inferred, so a third job cannot become an aggregator by declaring itself one.
    local aggregators="ci-report gate"
    local offenders
    offenders="$(printf '%s\n' "$wf" | awk -F'\t' -v agg="$aggregators" '
        BEGIN { n = split(agg, a, " "); for (i = 1; i <= n; i++) skip[a[i]] = 1 }
        ($2 ~ /(^|,)lint(,|$)/) && !($1 in skip) { print $1 }')"
    if [ -n "$offenders" ]; then
        echo "❌ a stage needs \`lint\`: $(printf '%s' "$offenders" | tr '\n' ' ')" >&2
        echo "   a lint finding does not decide any other stage's subject; see .github/ci-graph.tsv" >&2
        failures=$((failures + 1))
    else
        echo "✅ lint is a leaf for every stage; only the two aggregators name it"
    fi

    # 2. Every workflow job has a row, and every row has a job.
    local job
    while IFS=$'\t' read -r job _; do
        [ -n "$job" ] || continue
        if ! printf '%s\n' "$gr" | awk -F'\t' -v j="$job" '$1 == j { found = 1 } END { exit !found }'; then
            echo "❌ job '$job' has no row in ci-graph.tsv — an edge nobody declared" >&2
            failures=$((failures + 1))
        fi
    done <<< "$wf"

    local stage
    while IFS=$'\t' read -r stage _; do
        [ -n "$stage" ] || continue
        if ! printf '%s\n' "$wf" | awk -F'\t' -v s="$stage" '$1 == s { found = 1 } END { exit !found }'; then
            echo "❌ row '$stage' has no job in the workflow — a declared edge nobody has" >&2
            failures=$((failures + 1))
        fi
    done <<< "$gr"

    # 3. The `needs` lists agree, in both directions. A row that says `workspace` while the
    #    workflow says `workspace, lint` would pass checks 1–2 and be wrong.
    while IFS=$'\t' read -r stage declared; do
        [ -n "$stage" ] || continue
        local actual
        actual="$(printf '%s\n' "$wf" | awk -F'\t' -v s="$stage" '$1 == s { print $2 }')"
        [ -n "$actual" ] || continue            # already reported as a row with no job
        if [ "$declared" != "$actual" ]; then
            echo "❌ '$stage': graph says needs=[$declared], workflow says needs=[$actual]" >&2
            failures=$((failures + 1))
        fi
    done <<< "$gr"

    if [ "$failures" -ne 0 ]; then
        echo "❌ CI graph: $failures mismatch(es) between .github/ci-graph.tsv and the workflow" >&2
        return 1
    fi
    echo "✅ CI graph: $(printf '%s\n' "$wf" | grep -c . ) job(s), every edge declared"
}

# ── Self-test ────────────────────────────────────────────────────────────────────────────
# The three seeds #1411 names. A check that cannot be shown to fail is a check that has only
# ever passed, which is how `skipped` came to read as "not in the way".
selftest() {
    local tmp source failures=0
    tmp="$(mktemp -d)"
    source="$tmp/ci.yml"
    trap 'rm -rf "$tmp"' RETURN

    expect() {
        local label="$1" want="$2" got="$3"
        if [ "$want" = "$got" ]; then
            echo "  ✅ $label"
        else
            echo "  ❌ $label: want '$want', got '$got'" >&2
            failures=$((failures + 1))
        fi
    }

    # The real graph and workflow must pass, or the seeds prove nothing.
    check "$WORKFLOW" "$GRAPH" > /dev/null 2>&1
    expect "the committed graph and workflow agree" 0 $?

    # Seed 1: re-add `lint` to build's needs. The mutation is done in python3, not sed:
    # the earlier sed used GNU-only addressing, so on macOS it errored and the seed "passed"
    # because the checker ran against an unmodified workflow.
    python3 - "$WORKFLOW" "$source" <<'SEED1'
import sys
src, dst = sys.argv[1], sys.argv[2]
lines = open(src, encoding='utf-8').read().split('\n')
out, job = [], None
for line in lines:
    stripped = line.strip()
    if line.startswith('  ') and not line.startswith('   ') and stripped.endswith(':'):
        job = stripped[:-1]
    if job == 'build' and stripped == 'needs: [workspace]':
        line = line.replace('[workspace]', '[workspace, lint]')
    out.append(line)
open(dst, 'w', encoding='utf-8').write('\n'.join(out))
SEED1
    if ! grep -q 'needs: \[workspace, lint\]' "$source"; then
        echo "  ❌ seed — build needs lint: the mutation matched nothing" >&2
        failures=$((failures + 1))
    else
        check "$source" "$GRAPH" > /dev/null 2>&1
        expect "seed — build needs lint fails" 1 $?
    fi

    # Seed 2: delete a row while keeping the edge.
    awk '!/^portable\t/' "$GRAPH" > "$tmp/graph-no-portable.tsv"
    check "$WORKFLOW" "$tmp/graph-no-portable.tsv" > /dev/null 2>&1
    expect "seed — edge with no row fails" 1 $?

    # Seed 3: a row claiming an edge the workflow does not have. The mutation must be in the
    # column the checker compares — an earlier version edited the "why" prose, which no edge
    # comparison reads, so the checker passed and the seed reported only that it was inert.
    awk -F'\t' -v OFS='\t' '$1 == "portable" { $2 = "build" } { print }' "$GRAPH" \
        > "$tmp/graph-drifted.tsv"
    if ! awk -F'\t' '$1 == "portable" && $2 == "build"' "$tmp/graph-drifted.tsv" | grep -q .; then
        echo "  ❌ seed — row disagreeing with the workflow: the mutation matched no row" >&2
        failures=$((failures + 1))
    else
        check "$WORKFLOW" "$tmp/graph-drifted.tsv" > /dev/null 2>&1
        expect "seed — row disagreeing with the workflow fails" 1 $?
    fi

    if [ "$failures" -ne 0 ]; then
        echo "❌ ci_graph_check self-test failed ($failures)" >&2
        return 1
    fi
    echo "✅ ci_graph_check self-test: 3 seeds fired, 1 clean pass"
}

if [ "${1:-}" = "--self-test" ]; then
    selftest
    exit $?
fi

check
