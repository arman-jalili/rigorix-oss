#!/usr/bin/env bash
# ============================================================================
# validate-budget-derivation.sh — every ceiling in .pi/local-ci-budget.tsv is
# the number the file's own header says it is (#1582).
#
# The table states its derivation and, until this guard existed, nothing checked it. Seven of its
# eighteen cells were not the stated number: four were one second tighter, one was one second looser
# (`coverage` cold, hand-edited to `warm + 1`), and `total` warm was a floating-point residue —
# `ceil(1.10 * 700)` is 771 because 700*1.1 is 770.0000000000001. A table that asserts its own
# provenance and is never checked is a comment with a tab in it.
#
# The rule, in integer arithmetic so a reader can re-derive it in a shell:
#
#   warm ceiling = floor(11 * warm_slowest / 10)
#   cold ceiling = max(floor(11 * cold_slowest / 10), warm ceiling)
#
# Usage: bash .pi/scripts/validate-budget-derivation.sh [--self-test]
# Exit:  0 every cell matches · 1 a cell disagrees, or a row's sample is missing · 2 usage
# ============================================================================
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

BUDGET=".pi/local-ci-budget.tsv"
STAGES=("lint" "build" "test" "security" "guards" "coverage" "docs" "integration" "total")
# The header's own words. Both halves of the rule are asserted as text, so a header that changes its
# mind about the formula without changing this script fails here rather than in someone's arithmetic.
RULE_TEXT="ceiling = floor(11 * slowest / 10)"
COLD_RULE_TEXT="cold    = max(floor(11 * cold_slowest / 10), warm ceiling)"

# check_file <path> — prints one line per disagreement, returns 1 if any.
check_file() {
    local file="$1" failures=0 row=0 key warm_ceiling warm_runs warm_slowest cold_ceiling cold_runs cold_slowest
    local _on _machine derived_warm derived_cold
    if ! grep -qF "$RULE_TEXT" "$file"; then
        echo -e "${RED}❌${NC} $file: the header does not state the rule this guard applies ($RULE_TEXT)"
        failures=$((failures + 1))
    fi
    if ! grep -qF "$COLD_RULE_TEXT" "$file"; then
        echo -e "${RED}❌${NC} $file: the header does not state the cold rule ($COLD_RULE_TEXT)"
        failures=$((failures + 1))
    fi
    while IFS=$'\t' read -r key warm_ceiling warm_runs warm_slowest cold_ceiling cold_runs cold_slowest _on _machine; do
        [[ "$key" == "stage" ]] && continue
        [[ "$key" =~ ^# ]] && continue
        [[ -z "${key:-}" ]] && continue
        row=$((row + 1))
        if [[ -z "${warm_slowest:-}" || -z "${cold_slowest:-}" ]]; then
            echo -e "${RED}❌${NC} $key: a ceiling without its sample is a number with no provenance"
            failures=$((failures + 1))
            continue
        fi
        if [[ "${warm_runs:-0}" -lt 1 || "${cold_runs:-0}" -lt 1 ]]; then
            echo -e "${RED}❌${NC} $key: the sample size must be at least one run in each regime (warm ${warm_runs:-0}, cold ${cold_runs:-0})"
            failures=$((failures + 1))
        fi
        derived_warm=$(( warm_slowest * 11 / 10 ))
        derived_cold=$(( cold_slowest * 11 / 10 ))
        (( derived_cold < derived_warm )) && derived_cold="$derived_warm"
        if [[ "$warm_ceiling" != "$derived_warm" ]]; then
            echo -e "${RED}❌${NC} $key warm: recorded ${warm_ceiling}s, the rule gives ${derived_warm}s from slowest ${warm_slowest}s"
            failures=$((failures + 1))
        fi
        if [[ "$cold_ceiling" != "$derived_cold" ]]; then
            echo -e "${RED}❌${NC} $key cold: recorded ${cold_ceiling}s, the rule gives ${derived_cold}s from slowest ${cold_slowest}s"
            failures=$((failures + 1))
        fi
    done < <(grep -v '^#' "$file")
    # A row that is missing is a ceiling that does not apply: an absent stage is not a passing one.
    local stage
    for stage in "${STAGES[@]}"; do
        if ! grep -qE "^${stage}"$'\t' "$file"; then
            echo -e "${RED}❌${NC} $file: no row for '$stage' — a stage without a ceiling is a stage nobody bounds"
            failures=$((failures + 1))
        fi
    done
    return "$failures"
}

self_test() {
    local tmp failures=0
    tmp="$(mktemp -d)"
    # A record that follows the rule, built from the rule and not from the live table: a fixture copied
    # from the subject agrees with the subject whatever the rule says.
    {
        printf '# fixture\n# %s\n# %s\n' "$RULE_TEXT" "$COLD_RULE_TEXT"
        printf 'stage\twarm_ceiling\twarm_runs\twarm_slowest\tcold_ceiling\tcold_runs\tcold_slowest\tmeasured_on\tmachine\n'
        printf 'lint\t108\t4\t99\t1172\t6\t1066\t2026-11-15\tfixture\n'
        printf 'build\t17\t4\t16\t756\t6\t688\t2026-11-15\tfixture\n'
        printf 'test\t127\t4\t116\t431\t6\t392\t2026-11-15\tfixture\n'
        printf 'security\t33\t4\t30\t38\t6\t35\t2026-11-15\tfixture\n'
        printf 'guards\t83\t4\t76\t221\t6\t201\t2026-11-15\tfixture\n'
        printf 'coverage\t379\t4\t345\t379\t6\t345\t2026-11-15\tfixture\n'
        printf 'docs\t31\t4\t29\t38\t6\t35\t2026-11-15\tfixture\n'
        printf 'integration\t1\t4\t1\t1\t6\t1\t2026-11-15\tfixture\n'
        printf 'total\t770\t5\t700\t2409\t6\t2190\t2026-11-15\tfixture\n'
    } > "$tmp/good.tsv"

    if check_file "$tmp/good.tsv" >/dev/null 2>&1; then
        echo -e "  ${GREEN}✅${NC} a table that follows the rule passes"
    else
        echo -e "  ${RED}❌${NC} a table that follows the rule is refused"
        failures=$((failures + 1))
    fi

    # The planted defect is one second, because one second is the size of the failure that created this
    # issue: #1562 was a 700s run against a 699s ceiling. A guard that needs a 30% error to fire cannot
    # see it.
    sed 's/^guards\t83\t/guards\t84\t/' "$tmp/good.tsv" > "$tmp/planted.tsv"
    local out rc=0
    out="$(check_file "$tmp/planted.tsv" 2>&1)" || rc=$?
    if [[ "$rc" -ne 0 ]]; then
        echo -e "  ${GREEN}✅${NC} a planted one-second change is caught (exit $rc)"
    else
        echo -e "  ${RED}❌${NC} a planted one-second change passed"
        failures=$((failures + 1))
    fi
    if grep -q "guards warm: recorded 84s, the rule gives 83s" <<<"$out"; then
        echo -e "  ${GREEN}✅${NC} the failure names the cell, the recorded value and the derived one"
    else
        echo -e "  ${RED}❌${NC} the failure does not name what it found:"
        printf '%s\n' "$out" | sed 's/^/      /'
        failures=$((failures + 1))
    fi

    # A row whose sample is missing: the columns are the provenance, and a ceiling without one is what
    # this guard exists to prevent.
    grep -v '^docs' "$tmp/good.tsv" > "$tmp/missing.tsv"
    rc=0
    out="$(check_file "$tmp/missing.tsv" 2>&1)" || rc=$?
    if [[ "$rc" -ne 0 ]] && grep -q "no row for 'docs'" <<<"$out"; then
        echo -e "  ${GREEN}✅${NC} a missing row is a failure, not an absence"
    else
        echo -e "  ${RED}❌${NC} a missing row passed"
        failures=$((failures + 1))
    fi

    # And the header is part of the subject: a table whose derivation is not stated cannot be checked,
    # which is how seven cells drifted in the first place.
    grep -vF "$RULE_TEXT" "$tmp/good.tsv" > "$tmp/headerless.tsv"
    rc=0
    out="$(check_file "$tmp/headerless.tsv" 2>&1)" || rc=$?
    if [[ "$rc" -ne 0 ]] && grep -q "does not state the rule" <<<"$out"; then
        echo -e "  ${GREEN}✅${NC} a header that stops stating the rule fails"
    else
        echo -e "  ${RED}❌${NC} a header that stops stating the rule passed"
        failures=$((failures + 1))
    fi

    if [[ "$failures" -eq 0 ]]; then
        echo -e "  ${GREEN}✅ budget-derivation self-test: 5/5${NC}"
        return 0
    fi
    echo -e "  ${RED}❌ budget-derivation self-test: ${failures} case(s) failed${NC}"
    return 1
}

case "${1:-}" in
    --self-test)
        self_test
        exit $?
        ;;
    "")
        ;;
    *)
        echo "Usage: bash $0 [--self-test]" >&2
        exit 2
        ;;
esac

if [[ ! -f "$BUDGET" ]]; then
    echo -e "${RED}❌${NC} $BUDGET not found — there is nothing to check, which is not a pass" >&2
    exit 1
fi

if out="$(check_file "$BUDGET" 2>&1)"; then
    rows="$(grep -v '^#' "$BUDGET" | grep -vc '^stage'$'\t' || true)"
    echo -e "${GREEN}✅ budget derivation: every ceiling in $BUDGET is floor(11 × slowest / 10), cold at or above warm — ${rows} stage(s)${NC}"
    exit 0
fi
printf '%s\n' "$out"
echo -e "${RED}❌ a ceiling in $BUDGET is not the number its header states${NC}"
echo "  Raise a ceiling by re-measuring the sample, then re-derive it — never by editing the cell alone."
exit 1