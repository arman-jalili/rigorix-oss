#!/usr/bin/env bash
# ============================================================================
# validate-budget-derivation.sh — every ceiling in .pi/local-ci-budget.tsv is
# the number the file's own header says it is, and is conditioned on the
# experiment it was measured in (#1582, conditioned per stage in #1604).
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
# #1604 added the second half: a ceiling is selected by a count, so the count belongs to the sample.
# A stage's column is chosen by that stage's own compiled count, which makes `warm_compiled` a claim
# the table can get wrong — and the failure it exists to prevent is a warm ceiling sampled from runs
# whose stage compiled crates, which is what every warm row said before #1604 could express it:
#
#   stage rows — a warm sample compiled nothing (0), a cold sample compiled at least one (>= 1)
#   the `total` row — a warm sample is a run below COLD_CRATES, so 0 <= warm_compiled < 100
#
# `unmeasured` is a legal value in a compiled cell: it is what the sample actually recorded while the
# counter could not see what it was counting (#1599), and writing 0 instead would be inventing a
# measurement. The guard counts them on success, so the table's ignorance is printed rather than
# implied. A number, once written, is checked.
#
# Usage: bash .pi/scripts/validate-budget-derivation.sh [--self-test]
# Exit:  0 every cell matches · 1 a cell disagrees, a compiled count contradicts its column, or a
#           row's sample is missing · 2 usage
# ============================================================================
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

BUDGET=".pi/local-ci-budget.tsv"
STAGES=("lint" "build" "test" "security" "guards" "coverage" "docs" "integration" "total")
COLD_CRATES=100
# The header's own words. Both halves of the rule are asserted as text, so a header that changes its
# mind about the formula without changing this script fails here rather than in someone's arithmetic.
RULE_TEXT="ceiling = floor(11 * slowest / 10)"
COLD_RULE_TEXT="cold    = max(floor(11 * cold_slowest / 10), warm ceiling)"
CONDITION_RULE_TEXT="a warm stage sample compiled nothing; a cold stage sample compiled at least one"

# check_file <path> — prints one line per disagreement, returns 1 if any.
check_file() {
    local file="$1" failures=0 row=0 key warm_ceiling warm_runs warm_slowest warm_compiled
    local cold_ceiling cold_runs cold_slowest cold_compiled _on _machine
    local derived_warm derived_cold measured_compiled=0
    if ! grep -qF "$RULE_TEXT" "$file"; then
        echo -e "${RED}❌${NC} $file: the header does not state the rule this guard applies ($RULE_TEXT)"
        failures=$((failures + 1))
    fi
    if ! grep -qF "$COLD_RULE_TEXT" "$file"; then
        echo -e "${RED}❌${NC} $file: the header does not state the cold rule ($COLD_RULE_TEXT)"
        failures=$((failures + 1))
    fi
    if ! grep -qF "$CONDITION_RULE_TEXT" "$file"; then
        echo -e "${RED}❌${NC} $file: the header does not state the conditioning rule ($CONDITION_RULE_TEXT)"
        failures=$((failures + 1))
    fi
    while IFS=$'\t' read -r key warm_ceiling warm_runs warm_slowest warm_compiled \
        cold_ceiling cold_runs cold_slowest cold_compiled _on _machine; do
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
            echo -e "${RED}❌${NC} $key: the sample size must be at least one run in each column (warm ${warm_runs:-0}, cold ${cold_runs:-0})"
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
        # ── the conditioning (#1604) ──
        # A compiled cell is a number or the word `unmeasured`. Anything else — a blank, a `?`, a
        # range — is a cell the guard would have to guess about, and a cell nobody can check is the
        # defect this table keeps producing.
        local column compiled_value
        for column in warm cold; do
            if [[ "$column" == "warm" ]]; then compiled_value="${warm_compiled:-}"; else compiled_value="${cold_compiled:-}"; fi
            if [[ -z "${compiled_value:-}" ]]; then
                echo -e "${RED}❌${NC} $key $column: no compiled count recorded — the ceiling is conditioned on it, so a blank is not a sample"
                failures=$((failures + 1))
                continue
            fi
            if [[ "$compiled_value" == "unmeasured" ]]; then
                continue
            fi
            if ! [[ "$compiled_value" =~ ^[0-9]+$ ]]; then
                echo -e "${RED}❌${NC} $key $column: compiled count '${compiled_value}' is neither a number nor 'unmeasured'"
                failures=$((failures + 1))
                continue
            fi
            measured_compiled=$((measured_compiled + 1))
            if [[ "$column" == "warm" && "$key" != "total" && "$compiled_value" -ne 0 ]]; then
                echo -e "${RED}❌${NC} $key warm: the sample compiled ${compiled_value} crate(s) in this stage, so it is not a nothing-built sample — measure it on the cold column"
                failures=$((failures + 1))
            fi
            if [[ "$column" == "warm" && "$key" == "total" && "$compiled_value" -ge "$COLD_CRATES" ]]; then
                echo -e "${RED}❌${NC} $key warm: the sample compiled ${compiled_value} crates, which is at or above COLD_CRATES (${COLD_CRATES}) — that run was cold"
                failures=$((failures + 1))
            fi
            if [[ "$column" == "cold" && "$key" != "total" && "$compiled_value" -lt 1 ]]; then
                echo -e "${RED}❌${NC} $key cold: the sample compiled nothing in this stage, so it is not a built-something sample"
                failures=$((failures + 1))
            fi
        done
    done < <(grep -v '^#' "$file")
    # A row that is missing is a ceiling that does not apply: an absent stage is not a passing one.
    local stage
    for stage in "${STAGES[@]}"; do
        if ! grep -qE "^${stage}"$'\t' "$file"; then
            echo -e "${RED}❌${NC} $file: no row for '$stage' — a stage without a ceiling is a stage nobody bounds"
            failures=$((failures + 1))
        fi
    done
    # The counts for the success line are read by `summary_line` in the caller: `check_file` runs inside
    # `$(…)`, so a variable it sets dies with the subshell — the first version of this printed
    # "0 row(s), 0 compiled cell(s) measured" over a table with nine rows.
    return "$failures"
}

# summary_line <path> — "<rows> <measured> <unmeasured>" over the table's compiled cells.
# Read from the file rather than accumulated in check_file, for the subshell reason above.
summary_line() {
    awk -F'\t' '
        $1 == "stage" || $1 ~ /^#/ { next }
        NF < 11 { next }
        { rows++; if ($5 != "unmeasured" && $5 != "") measured++; else blind++;
                 if ($9 != "unmeasured" && $9 != "") measured++; else blind++ }
        END { printf "%d %d %d", rows + 0, measured + 0, blind + 0 }
    ' "$1"
}

self_test() {
    local tmp failures=0
    tmp="$(mktemp -d)"
    # A record that follows the rule, built from the rule and not from the live table: a fixture copied
    # from the subject agrees with the subject whatever the rule says.
    {
        printf '# fixture\n# %s\n# %s\n# %s\n' "$RULE_TEXT" "$COLD_RULE_TEXT" "$CONDITION_RULE_TEXT"
        printf 'stage\twarm_ceiling\twarm_runs\twarm_slowest\twarm_compiled\tcold_ceiling\tcold_runs\tcold_slowest\tcold_compiled\tmeasured_on\tmachine\n'
        printf 'lint\t108\t4\t99\t0\t1172\t6\t1066\t19\t2026-11-15\tfixture\n'
        printf 'build\t17\t4\t16\t0\t756\t6\t688\t1\t2026-11-15\tfixture\n'
        printf 'test\t127\t4\t116\tunmeasured\t431\t6\t392\tunmeasured\t2026-11-15\tfixture\n'
        printf 'security\t33\t4\t30\t0\t38\t6\t35\t3\t2026-11-15\tfixture\n'
        printf 'guards\t83\t4\t76\t0\t221\t6\t201\t38\t2026-11-15\tfixture\n'
        printf 'coverage\t379\t4\t345\t0\t379\t6\t345\t12\t2026-11-15\tfixture\n'
        printf 'docs\t31\t4\t29\t0\t38\t6\t35\t2\t2026-11-15\tfixture\n'
        printf 'integration\t1\t4\t1\t0\t1\t6\t1\t1\t2026-11-15\tfixture\n'
        printf 'total\t770\t5\t700\t9\t2409\t6\t2190\t488\t2026-11-15\tfixture\n'
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

    # The defect #1604 names, planted at the size it occurs: `lint` really did compile 19 crates in a
    # warm run, and the old table could not say so. A warm sample with a crate in it is a sample of a
    # different experiment, and the guard must refuse it rather than average it in.
    sed 's/^lint\t108\t4\t99\t0\t/lint\t108\t4\t99\t19\t/' "$tmp/good.tsv" > "$tmp/warm-built.tsv"
    rc=0
    out="$(check_file "$tmp/warm-built.tsv" 2>&1)" || rc=$?
    if [[ "$rc" -ne 0 ]] && grep -q "lint warm: the sample compiled 19 crate(s)" <<<"$out"; then
        echo -e "  ${GREEN}✅${NC} a warm stage sample that compiled crates is refused, naming the row and the count"
    else
        echo -e "  ${RED}❌${NC} a warm stage sample that compiled 19 crates passed"
        printf '%s\n' "$out" | sed 's/^/      /'
        failures=$((failures + 1))
    fi

    # The other half of the same rule: a cold sample whose stage compiled nothing is not a cold sample.
    sed 's/^guards\t83\t4\t76\t0\t\([0-9]*\)\t\([0-9]*\)\t\([0-9]*\)\t38\t/guards\t83\t4\t76\t0\t\1\t\2\t\3\t0\t/' \
        "$tmp/good.tsv" > "$tmp/cold-idle.tsv"
    rc=0
    out="$(check_file "$tmp/cold-idle.tsv" 2>&1)" || rc=$?
    if [[ "$rc" -ne 0 ]] && grep -q "guards cold: the sample compiled nothing" <<<"$out"; then
        echo -e "  ${GREEN}✅${NC} a cold stage sample that compiled nothing is refused"
    else
        echo -e "  ${RED}❌${NC} a cold stage sample that compiled nothing passed"
        printf '%s\n' "$out" | sed 's/^/      /'
        failures=$((failures + 1))
    fi

    # `total` is conditioned on the run, so its threshold is COLD_CRATES rather than 1: a warm total
    # sample compiled fewer than 100 crates, and 400 is a cold run written in the wrong column.
    sed 's/^total\t770\t5\t700\t9\t/total\t770\t5\t700\t400\t/' "$tmp/good.tsv" > "$tmp/total-cold.tsv"
    rc=0
    out="$(check_file "$tmp/total-cold.tsv" 2>&1)" || rc=$?
    if [[ "$rc" -ne 0 ]] && grep -q "total warm: the sample compiled 400 crates" <<<"$out"; then
        echo -e "  ${GREEN}✅${NC} a warm total sample from a cold run is refused"
    else
        echo -e "  ${RED}❌${NC} a warm total sample of 400 crates passed"
        printf '%s\n' "$out" | sed 's/^/      /'
        failures=$((failures + 1))
    fi

    # A compiled cell that is neither a number nor `unmeasured` cannot be checked, so it is refused.
    sed 's/^docs\t31\t4\t29\t0\t/docs\t31\t4\t29\t?\t/' "$tmp/good.tsv" > "$tmp/vague.tsv"
    rc=0
    out="$(check_file "$tmp/vague.tsv" 2>&1)" || rc=$?
    if [[ "$rc" -ne 0 ]] && grep -q "docs warm: compiled count '?' is neither a number nor 'unmeasured'" <<<"$out"; then
        echo -e "  ${GREEN}✅${NC} a compiled cell that cannot be read is a failure, not a pass"
    else
        echo -e "  ${RED}❌${NC} an unreadable compiled cell passed"
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

    # Including the conditioning rule: the columns mean nothing if the header stops saying which
    # experiment each one describes.
    grep -vF "$CONDITION_RULE_TEXT" "$tmp/good.tsv" > "$tmp/unconditioned.tsv"
    rc=0
    out="$(check_file "$tmp/unconditioned.tsv" 2>&1)" || rc=$?
    if [[ "$rc" -ne 0 ]] && grep -q "does not state the conditioning rule" <<<"$out"; then
        echo -e "  ${GREEN}✅${NC} a header that stops stating the conditioning rule fails"
    else
        echo -e "  ${RED}❌${NC} a header that stops stating the conditioning rule passed"
        failures=$((failures + 1))
    fi

    # A plant that does not apply is not a test: each fixture above asserts it changed the file.
    local planted
    for planted in warm-built cold-idle total-cold vague; do
        if cmp -s "$tmp/good.tsv" "$tmp/$planted.tsv"; then
            echo -e "  ${RED}❌${NC} the '$planted' fixture is identical to the good table — it planted nothing"
            failures=$((failures + 1))
        fi
    done

    if [[ "$failures" -eq 0 ]]; then
        echo -e "  ${GREEN}✅ budget-derivation self-test: 9/9${NC}"
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
    read -r rows measured blind <<<"$(summary_line "$BUDGET")"
    echo -e "${GREEN}✅ budget derivation: every ceiling in $BUDGET is floor(11 × slowest / 10) per column, cold at or above warm, and conditioned on the compiled count of its own sample — ${rows} row(s), ${measured} compiled cell(s) measured, ${blind} recorded unmeasured${NC}"
    if [[ "${blind:-0}" -gt 0 ]]; then
        echo "  ${blind} sample cell(s) say 'unmeasured': those runs predate the counter that can see a compile (#1599), so the conditioning cannot be certified for them. The re-baseline after #1604 replaces them with numbers, and a warm cell that is not 0 is then refused."
    fi
    exit 0
fi
printf '%s\n' "$out"
echo -e "${RED}❌ a ceiling in $BUDGET is not the number its header states${NC}"
echo "  Raise a ceiling by re-measuring the sample, then re-derive it — never by editing the cell alone."
exit 1