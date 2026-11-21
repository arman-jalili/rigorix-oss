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
#   every row, stage or run — a warm sample compiled nothing (0), a cold sample compiled at least one
#
# The `total` row used to be conditioned on COLD_CRATES (a warm run was one under a hundred crates),
# which made a 31-crate run's 23 minutes "warm" against a ceiling sampled from nothing-built runs
# (#1612). One rule for every row: did this subject pay a compile?
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
# The header's own words. Both halves of the rule are asserted as text, so a header that changes its
# mind about the formula without changing this script fails here rather than in someone's arithmetic.
RULE_TEXT="ceiling = floor(11 * slowest / 10)"
COLD_RULE_TEXT="cold    = max(floor(11 * cold_slowest / 10), warm ceiling)"
CONDITION_RULE_TEXT="a warm stage sample compiled nothing; a cold stage sample compiled at least one"
BAND_RULE_TEXT="band        = min(78, warm_ceiling)"
# The run-level row's tolerance is the sum of the rows it bounds (#1618), and it is stated as its own
# rule because it is derived from *other rows* rather than from a sample of its own: a total band that
# does not follow its stages is a second answer to the question the stages already answered.
TOTAL_BAND_RULE_TEXT="band_total  = sum of the stage bands"
# #1622: a ceiling is a duration, and a duration is a measurement of the code only when the host is not
# also moving. `machine` therefore states the load the sample was taken under, and `load N.NN` is the
# literal the header must contain — the same text-asserted shape as the three rules above.
MACHINE_RULE_TEXT="the load the sample was measured under"
LOAD_RECORDED_FROM='2026-11-19'
# Sums over the stage rows, read before the row loop so the answer does not depend on row order.
stage_band_sum() {  # $1 file → integer sum of the stage rows' bands
    awk -F'\t' '$1 != "stage" && $1 != "total" && $1 !~ /^#/ && NF > 9 { b += $10 } END { print b + 0 }' "$1"
}
stage_ceiling_sum() {  # $1 file → integer sum of the stage rows' warm ceilings
    awk -F'\t' '$1 != "stage" && $1 != "total" && $1 !~ /^#/ && NF > 1 { c += $2 } END { print c + 0 }' "$1"
}
# The band's upper value is the per-stage spread `.github/ci-budgets.tsv` records — 0.4-1.3 minutes —
# so the number is read from that file rather than typed here: a tolerance that lives in two files is a
# tolerance that can disagree with itself. `derived_band_max` returns it in integer seconds.
SPREAD_FILE=".github/ci-budgets.tsv"
derived_band_max() {
    local max_minutes
    max_minutes="$(grep -oE '0\.[0-9]+-[0-9]+\.[0-9]+ minutes' "$SPREAD_FILE" 2>/dev/null \
        | head -1 | sed -E 's/.*-([0-9]+\.[0-9]+) minutes/\1/' || true)"
    if [[ -z "$max_minutes" ]]; then
        printf '0'
        return
    fi
    # 1.3 min -> 78 s, in integer arithmetic (1.3 * 60 = 13 * 6).
    awk -v m="$max_minutes" 'BEGIN { printf "%d", (m * 60) + 0.5 }'
}

# check_file <path> — prints one line per disagreement, returns 1 if any.
check_file() {
    local file="$1" failures=0 row=0 key warm_ceiling warm_runs warm_slowest warm_compiled
    local cold_ceiling cold_runs cold_slowest cold_compiled band _on _machine band_max derived_band
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
    local band_sum ceiling_sum
    band_sum="$(stage_band_sum "$file")"
    ceiling_sum="$(stage_ceiling_sum "$file")"
    if ! grep -qF "$TOTAL_BAND_RULE_TEXT" "$file"; then
        echo -e "${RED}❌${NC} $file: the header does not state the run-level band rule ($TOTAL_BAND_RULE_TEXT)"
        failures=$((failures + 1))
    fi
    if [[ "$band_sum" -le 0 ]]; then
        echo -e "${RED}❌${NC} $file: the stage bands sum to ${band_sum}s, so the run-level band cannot be derived — a total band of 0 is the row #1618 filed"
        failures=$((failures + 1))
    fi
    if ! grep -qF "$BAND_RULE_TEXT" "$file"; then
        echo -e "${RED}❌${NC} $file: the header does not state the band rule ($BAND_RULE_TEXT)"
        failures=$((failures + 1))
    fi
    # ── the machine each sample was taken on (#1622) ──
# A ceiling is a duration; a duration is a measurement of the code only when the host is not also
# moving. The same portable step ran 2 s alone and 45 s inside a run at load 16.15, and the table could
# only record the 45 s. So the column states the load, and a sample taken from `LOAD_RECORDED_FROM` on
# must carry a number — `load unrecorded` stays legal only for the samples that predate the counter.
if ! grep -q 'load N.NN' "$file"; then
    echo -e "${RED}❌${NC} $file: the header does not state what the machine column must contain ($MACHINE_RULE_TEXT) — a ceiling without its load is a number with half a provenance"
    failures=$((failures + 1))
fi

band_max="$(derived_band_max)"
    if [[ "$band_max" -eq 0 ]]; then
        echo -e "${RED}❌${NC} $SPREAD_FILE records no per-stage spread (expected '0.4-1.3 minutes'), so the band cannot be derived — a tolerance nobody measured is a preference"
        failures=$((failures + 1))
    fi
    while IFS=$'\t' read -r key warm_ceiling warm_runs warm_slowest warm_compiled \
        cold_ceiling cold_runs cold_slowest cold_compiled band _on _machine; do
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
        # ── the machine (#1622) ──
        if [[ -z "${_machine:-}" ]]; then
            echo -e "${RED}❌${NC} $key: no machine column — a sample that does not say what it was taken on cannot be re-read"
            failures=$((failures + 1))
        elif ! [[ "${_machine}" =~ load\ ([0-9]+\.[0-9]+|unrecorded) ]]; then
            echo -e "${RED}❌${NC} $key: machine '${_machine}' does not state the load ($MACHINE_RULE_TEXT)"
            failures=$((failures + 1))
        elif [[ "${_machine}" == *"load unrecorded"* ]]; then
            # ISO dates compare lexicographically, which is the point of the format.
            if [[ ! "${_on:-}" < "$LOAD_RECORDED_FROM" ]]; then
                echo -e "${RED}❌${NC} $key: measured_on ${_on:-?} is not before ${LOAD_RECORDED_FROM}, so 'load unrecorded' is refused — record the load the sample was taken under"
                failures=$((failures + 1))
            fi
        fi
        # ── the noise band (#1612) ──
        # A row's tolerance is derived, not chosen: `min(band_max, warm ceiling)` for a stage, 0 for
        # the run-level row, whose subject is the whole run and which cannot be "the same code measured
        # twice" when it overshoots by minutes.
        if [[ -z "${band:-}" ]]; then
            echo -e "${RED}❌${NC} $key: no band column — a row without a tolerance is a row with zero tolerance"
            failures=$((failures + 1))
        elif ! [[ "$band" =~ ^[0-9]+$ ]]; then
            echo -e "${RED}❌${NC} $key: band '${band}' is not a whole number of seconds"
            failures=$((failures + 1))
        elif [[ "$band_max" -gt 0 ]]; then
            if [[ "$key" == "total" ]]; then
                # The sum of the rows above it, not a number of its own (#1618).
                derived_band="$band_sum"
            else
                derived_band="$band_max"
                (( derived_band > warm_ceiling )) && derived_band="$warm_ceiling"
            fi
            if [[ "$band" != "$derived_band" ]]; then
                if [[ "$key" == "total" ]]; then
                    echo -e "${RED}❌${NC} $key: recorded band ${band}s, the rule gives ${derived_band}s (the sum of the stage bands: ${band_sum}s) — a stage row changed and the run-level row did not follow it"
                else
                    echo -e "${RED}❌${NC} $key: recorded band ${band}s, the rule gives ${derived_band}s (min(${band_max}, ${warm_ceiling}) for a stage)"
                fi
                failures=$((failures + 1))
            fi
        fi
        # ── the run-level row bounds a decomposition, not itself (#1618) ──
        # 770 + 0 < 779 was the arithmetic: a run in which every stage sat exactly at its ceiling
        # failed the run while no stage failed. The band absorbs the decomposition's own spread, so the
        # invariant is `ceiling + band >= sum of the stage ceilings`.
        if [[ "$key" == "total" ]]; then
            if (( warm_ceiling + band < ceiling_sum )); then
                echo -e "${RED}❌${NC} $key: ceiling ${warm_ceiling}s + band ${band}s = $(( warm_ceiling + band ))s is below the sum of the stage ceilings (${ceiling_sum}s) — a run in which every stage sits at its own ceiling would fail here while no stage failed"
                failures=$((failures + 1))
            fi
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
            if [[ "$column" == "warm" && "$compiled_value" -ne 0 ]]; then
                echo -e "${RED}❌${NC} $key warm: the sample compiled ${compiled_value} crate(s), so it is not a nothing-built sample — measure it on the cold column"
                failures=$((failures + 1))
            fi
            if [[ "$column" == "cold" && "$compiled_value" -lt 1 ]]; then
                echo -e "${RED}❌${NC} $key cold: the sample compiled nothing, so it is not a built-something sample"
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
        printf '# fixture\n# %s\n# %s\n# %s\n# %s\n# %s\n# %s\n#   load N.NN the load the sample was measured under (#1622)\n' \
            "$RULE_TEXT" "$COLD_RULE_TEXT" "$CONDITION_RULE_TEXT" "$BAND_RULE_TEXT" "$TOTAL_BAND_RULE_TEXT" "$MACHINE_RULE_TEXT"
        printf 'stage\twarm_ceiling\twarm_runs\twarm_slowest\twarm_compiled\tcold_ceiling\tcold_runs\tcold_slowest\tcold_compiled\tband\tmeasured_on\tmachine\n'
        printf 'lint\t108\t4\t99\t0\t1172\t6\t1066\t19\t78\t2026-11-15\tfixture, load 1.00\n'
        printf 'build\t17\t4\t16\t0\t756\t6\t688\t1\t17\t2026-11-15\tfixture, load 1.00\n'
        printf 'test\t127\t4\t116\tunmeasured\t431\t6\t392\tunmeasured\t78\t2026-11-15\tfixture, load 1.00\n'
        printf 'security\t33\t4\t30\t0\t38\t6\t35\t3\t33\t2026-11-15\tfixture, load 1.00\n'
        printf 'guards\t83\t4\t76\t0\t221\t6\t201\t38\t78\t2026-11-15\tfixture, load 1.00\n'
        printf 'coverage\t379\t4\t345\t0\t379\t6\t345\t12\t78\t2026-11-15\tfixture, load 1.00\n'
        printf 'docs\t31\t4\t29\t0\t38\t6\t35\t2\t31\t2026-11-15\tfixture, load 1.00\n'
        printf 'integration\t1\t4\t1\t0\t1\t6\t1\t1\t1\t2026-11-15\tfixture, load 1.00\n'
        printf 'total\t770\t5\t700\t0\t2409\t6\t2190\t488\t394\t2026-11-15\tfixture, load 1.00\n'
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

    # The run-level row is conditioned by the same rule as every stage: a warm sample is a run that built
    # nothing, so 31 crates in the warm cell is a built-something run written in the wrong column (#1612).
    sed 's/^total\t770\t5\t700\t0\t/total\t770\t5\t700\t31\t/' "$tmp/good.tsv" > "$tmp/total-cold.tsv"
    rc=0
    out="$(check_file "$tmp/total-cold.tsv" 2>&1)" || rc=$?
    if [[ "$rc" -ne 0 ]] && grep -q "total warm: the sample compiled 31 crate(s)" <<<"$out"; then
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

    # The band is a tolerance, and a tolerance typed by hand is the thing this guard exists to stop: a
    # band wider than the measured spread silently absorbs a regression, and a narrower one fails the
    # same code measured twice.
    sed 's/^guards\t83\t4\t76\t0\t\([0-9]*\)\t\([0-9]*\)\t\([0-9]*\)\t38\t78\t/guards\t83\t4\t76\t0\t\1\t\2\t\3\t38\t120\t/' \
        "$tmp/good.tsv" > "$tmp/wide-band.tsv"
    rc=0
    out="$(check_file "$tmp/wide-band.tsv" 2>&1)" || rc=$?
    if [[ "$rc" -ne 0 ]] && grep -q "guards: recorded band 120s, the rule gives 78s" <<<"$out"; then
        echo -e "  ${GREEN}✅${NC} a band that is not the measured spread is refused, naming both numbers"
    else
        echo -e "  ${RED}❌${NC} a hand-widened band passed"
        printf '%s\n' "$out" | sed 's/^/      /'
        failures=$((failures + 1))
    fi

    # A short row cannot borrow the full spread: 78 s of noise on a 1-second ceiling is 78x its budget.
    sed 's/^docs\t31\t4\t29\t0\t38\t6\t35\t2\t31\t/docs\t31\t4\t29\t0\t38\t6\t35\t2\t78\t/' \
        "$tmp/good.tsv" > "$tmp/short-band.tsv"
    rc=0
    out="$(check_file "$tmp/short-band.tsv" 2>&1)" || rc=$?
    if [[ "$rc" -ne 0 ]] && grep -q "docs: recorded band 78s, the rule gives 31s" <<<"$out"; then
        echo -e "  ${GREEN}✅${NC} a short row's band is capped at its own ceiling"
    else
        echo -e "  ${RED}❌${NC} a short row borrowed the full spread"
        printf '%s\n' "$out" | sed 's/^/      /'
        failures=$((failures + 1))
    fi

    # The run-level band is the sum of the rows it bounds, so a total that carries the per-stage spread
    # itself — or nothing — is refused, naming the sum it should have followed (#1618).
    sed 's/^total\t770\t5\t700\t0\t\([0-9]*\)\t\([0-9]*\)\t\([0-9]*\)\t488\t394\t/total\t770\t5\t700\t0\t\1\t\2\t\3\t488\t78\t/' \
        "$tmp/good.tsv" > "$tmp/total-band.tsv"
    rc=0
    out="$(check_file "$tmp/total-band.tsv" 2>&1)" || rc=$?
    if [[ "$rc" -ne 0 ]] && grep -q "total: recorded band 78s, the rule gives 394s" <<<"$out"; then
        echo -e "  ${GREEN}✅${NC} a run-level band that is not the sum of the stage bands is refused"
    else
        echo -e "  ${RED}❌${NC} the run-level row carried a band of its own"
        printf '%s\n' "$out" | sed 's/^/      /'
        failures=$((failures + 1))
    fi

    # And the plant #1618 actually asks for: a *stage* row changes and the total does not follow. This is
    # the silent invalidation the issue names — the sum moves from 394 to 395 and nobody notices.
    sed 's/^integration\t1\t4\t1\t0\t1\t6\t1\t1\t1\t/integration\t2\t4\t2\t0\t2\t6\t2\t1\t2\t/' \
        "$tmp/good.tsv" > "$tmp/stale-total.tsv"
    rc=0
    out="$(check_file "$tmp/stale-total.tsv" 2>&1)" || rc=$?
    if [[ "$rc" -ne 0 ]] && grep -q "total: recorded band 394s, the rule gives 395s" <<<"$out"; then
        echo -e "  ${GREEN}✅${NC} a stage row that moves the sum is caught at the total, not silently absorbed"
    else
        echo -e "  ${RED}❌${NC} a stage row changed and the total was not re-derived"
        printf '%s\n' "$out" | sed 's/^/      /'
        failures=$((failures + 1))
    fi

    # The invariant itself: a total ceiling plus band below the sum of the stage ceilings is a bound on a
    # decomposition of itself. 100 + 394 = 494 < 779.
    sed 's/^total\t770\t5\t700\t/total\t100\t5\t91\t/' "$tmp/good.tsv" > "$tmp/total-too-low.tsv"
    rc=0
    out="$(check_file "$tmp/total-too-low.tsv" 2>&1)" || rc=$?
    if [[ "$rc" -ne 0 ]] && grep -q "total: ceiling 100s + band 394s = 494s is below the sum of the stage ceilings (779s)" <<<"$out"; then
        echo -e "  ${GREEN}✅${NC} a run-level ceiling below the decomposition it bounds is refused"
    else
        echo -e "  ${RED}❌${NC} a run-level ceiling below its own decomposition passed"
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

    # ── #1622: the machine a sample was taken on ──
    # A ceiling whose row does not say what it was measured on cannot be re-read: the 45 s portable step
    # at load 16.15 and the 2 s step alone are the same code, and only the load distinguishes them.
    sed 's/\tfixture, load 1.00/\tfixture/' "$tmp/good.tsv" > "$tmp/no-load.tsv"
    rc=0; out="$(check_file "$tmp/no-load.tsv" 2>&1)" || rc=$?
    if [[ "$rc" -ne 0 ]]; then
        echo -e "  ${GREEN}✅${NC} a sample that does not state its load is refused (fails, as it must)"
    else
        echo -e "  ${RED}❌${NC} a sample that does not state its load passed"
        failures=$((failures + 1))
    fi

    # The date bound: `load unrecorded` is legal only for samples taken before the counter existed. A row
    # that says today and does not know the load is a row that chose not to write it down.
    sed "s/^build\t17\t4\t16\t0\t756\t6\t688\t1\t17\t2026-11-15\tfixture, load 1.00\$/build\t17\t4\t16\t0\t756\t6\t688\t1\t17\t${LOAD_RECORDED_FROM}\tfixture, load unrecorded/" \
        "$tmp/good.tsv" > "$tmp/unrecorded-today.tsv"
    rc=0; out="$(check_file "$tmp/unrecorded-today.tsv" 2>&1)" || rc=$?
    if [[ "$rc" -ne 0 ]]; then
        echo -e "  ${GREEN}✅${NC} 'load unrecorded' on a sample taken after ${LOAD_RECORDED_FROM} is refused (fails, as it must)"
    else
        echo -e "  ${RED}❌${NC} 'load unrecorded' on a sample taken after ${LOAD_RECORDED_FROM} passed"
        failures=$((failures + 1))
    fi

    # A plant that does not apply is not a test: each fixture above asserts it changed the file.
    local planted
    for planted in warm-built cold-idle total-cold vague wide-band short-band total-band stale-total total-too-low no-load unrecorded-today; do
        if cmp -s "$tmp/good.tsv" "$tmp/$planted.tsv"; then
            echo -e "  ${RED}❌${NC} the '$planted' fixture is identical to the good table — it planted nothing"
            failures=$((failures + 1))
        fi
    done

    if [[ "$failures" -eq 0 ]]; then
        # No hand-maintained total: it read 12/12 while running fifteen cases, and a count typed
        # beside a list is a count that drifts the moment the list grows (#1618).
        echo -e "  ${GREEN}✅ budget-derivation self-test: every case passed${NC}"
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