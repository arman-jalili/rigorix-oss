#!/usr/bin/env bash
# check-bundle-size.sh — NFR-002 / criterion 23: measure, then gate at +25%.
#
# Two modes, and the difference matters:
#
#   shipped  — a shipped artifact exists (pkg/*.wasm). Three assertions, all required:
#              (1) it carries no `name` custom section — size alone cannot prove the
#                  strip ran, since an unstripped module is *smaller* than the budget
#                  (#1353); (2) measured_bytes and budget_bytes, read from
#                  ci/bundle-budget.txt — `budget_bytes` is measured + 25% of the recorded
#                  measurement, and the ceiling is the number the specification fixed; (3) the record's own
#                  arithmetic reconciles — emitted_bytes minus name_section_bytes must be
#                  measured_bytes, and every integer in its note must be a recorded field
#                  (#1409). A record with two measurements has no measurement.
#   awaiting — there is no shipped artifact yet (packaging pending). The gate records the
#              release cdylib size and says it is not enforcing a ceiling. It does NOT
#              silently pass: it prints the state and the reason, and it refuses to treat
#              a zero-byte or absent artifact as "within budget".
#
# Fail-closed in both modes: a measurement that cannot be taken is an error, and a budget
# file that is missing or unparseable is an error.
set -euo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUDGET_FILE="$CRATE_DIR/ci/bundle-budget.txt"
SHIPPED="$CRATE_DIR/pkg/rigorix_gate_wasm_bg.wasm"
cd "$CRATE_DIR"

# The target directory belongs to the *workspace*, not to this crate: since the crate became
# a member (ADR-011) cargo writes to <repo>/target. Ask cargo rather than guessing, so the
# gate reads the same artifact the build produced — a gate pointing at a path that cannot
# exist fails for the wrong reason, which reads like a finding.
TARGET_DIR="$(cargo metadata --no-deps --format-version 1 2>/dev/null \
    | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p' | head -1)"
if [ -z "${TARGET_DIR:-}" ]; then
    echo "❌ could not resolve the target directory (cargo metadata failed)" >&2
    echo "   a gate that cannot find what it measures does not pass" >&2
    exit 1
fi
RAW="$TARGET_DIR/wasm32-unknown-unknown/release/rigorix_gate_wasm.wasm"

if [ ! -f "$BUDGET_FILE" ]; then
    echo "❌ no budget file: a size gate without a budget is an opinion" >&2
    exit 1
fi

read_field() {
    sed -nE "s/^$1:[[:space:]]*//p" "$BUDGET_FILE" | head -1
}

MEASURED=$(read_field measured_bytes)
BUDGET=$(read_field budget_bytes)
CEILING=$(read_field ceiling_bytes)
EMITTED=$(read_field emitted_bytes)
NAME_SECTION=$(read_field name_section_bytes)
if [ -z "${MEASURED:-}" ] || [ -z "${BUDGET:-}" ] || [ -z "${CEILING:-}" ] ||
   [ -z "${EMITTED:-}" ] || [ -z "${NAME_SECTION:-}" ]; then
    echo "❌ the budget file is unparseable" >&2
    echo "   need measured_bytes, budget_bytes, ceiling_bytes, emitted_bytes, name_section_bytes" >&2
    exit 1
fi

# #1409: the note beside the fields is prose the gate used to ignore, so it could describe an
# artifact two orders of magnitude away and nothing noticed. Two rules make it falsifiable: the
# strip's arithmetic must reconcile the three sizes, and every integer in the note must be one
# of the recorded fields. A statement the gate cannot reconcile does not belong in the file
# the gate reads.
if [ $((EMITTED - NAME_SECTION)) -ne "$MEASURED" ]; then
    echo "❌ the record does not reconcile: emitted ${EMITTED} minus name section ${NAME_SECTION}" >&2
    echo "   is $((EMITTED - NAME_SECTION)), but measured_bytes is ${MEASURED}" >&2
    echo "   re-record all three from one build; a record with two measurements has none" >&2
    exit 1
fi
ACCOUNTED=" ${MEASURED} ${BUDGET} ${CEILING} ${EMITTED} ${NAME_SECTION} "
for integer in $(sed -n '/^note:/,$p' "$BUDGET_FILE" \
    | grep -oE '(^|[^0-9A-Za-z_-])[0-9][0-9][0-9]+' \
    | tr -dc '0-9\n'); do
    case "$ACCOUNTED" in
        *" ${integer} "*) : ;;
        *)
            echo "❌ the note states ${integer}, which is not a recorded measurement" >&2
            echo "   the note describes this artifact only; superseded numbers belong in ADR-014" >&2
            exit 1 ;;
    esac
done

# ── What the numbers may claim (#1417) ──────────────────────────────────────
#
# `measured_bytes` is the *accounting* operand: `emitted_bytes - name_section_bytes` must
# equal it, and all three are recorded in the same file, so that arithmetic is
# host-independent. `observed_*` is a dated observation of one build on one host and is
# compared to **nothing**:
#
#   64 B between two builds of one branch on one machine (checkout path, `-C metadata`, LTO
#   reachability — unattributed), 429 B between macOS and the Linux runner. A byte-exact
#   equality is therefore a claim only the recording host can make at one moment, which is an
#   observation, not a gate. #1405 hid that behind a 1 % band; #1417 removes the comparison
#   and keeps what actually holds everywhere.
#
# Host-independent, therefore strict: the ceiling, the budget headroom, the absence of any
# builder path, and a non-empty artifact.
#
#   gate_numbers <budget-file> <actual-bytes>      → prints, returns non-zero on a failure
gate_numbers() {
    local budget_file="$1" actual="$2"
    local measured budget ceiling emitted name_section
    local observed_bytes observed_on observed_at
    local drift sign

    local read
    read() { sed -nE "s/^$1:[[:space:]]*//p" "$budget_file" | head -1; }

    measured=$(read measured_bytes)
    budget=$(read budget_bytes)
    ceiling=$(read ceiling_bytes)
    emitted=$(read emitted_bytes)
    name_section=$(read name_section_bytes)
    observed_bytes=$(read observed_bytes)
    observed_on=$(read observed_on)
    observed_at=$(read observed_at)

    if [ -z "${measured:-}" ] || [ -z "${budget:-}" ] || [ -z "${ceiling:-}" ] ||
       [ -z "${emitted:-}" ] || [ -z "${name_section:-}" ] ||
       [ -z "${observed_bytes:-}" ] || [ -z "${observed_on:-}" ] || [ -z "${observed_at:-}" ]; then
        echo "❌ the budget file is unparseable" >&2
        echo "   need measured_bytes, budget_bytes, ceiling_bytes, emitted_bytes," >&2
        echo "   name_section_bytes, observed_bytes, observed_on, observed_at" >&2
        return 1
    fi

    # ── configuration, before any comparison ────────────────────────────────
    if [ $((emitted - name_section)) -ne "$measured" ]; then
        echo "❌ the record does not reconcile: emitted ${emitted} - name section ${name_section}" >&2
        echo "   = $((emitted - name_section)), but measured_bytes is ${measured}" >&2
        echo "   re-record all three from one build; a record with two measurements has none" >&2
        return 1
    fi
    if [ "$actual" -le 0 ]; then
        echo "❌ the artifact is empty: a zero-byte module is not within any budget" >&2
        return 1
    fi
    # A shipped artifact with no budget is an error, never "within budget" (#1349).
    if [ "$budget" -eq 0 ]; then
        echo "❌ a shipped artifact exists but no budget was set: measure it first" >&2
        return 1
    fi
    if [ "$measured" -gt "$budget" ]; then
        echo "❌ configuration error: budget_bytes ${budget} is below measured_bytes ${measured}" >&2
        echo "   the ratchet is derived from the measurement; a budget under it can never be met" >&2
        return 1
    fi

    echo "   emitted ${emitted} B · name section ${name_section} B · measured ${measured} B · budget ${budget} B · ceiling ${ceiling} B · actual ${actual} B"

    # ── the observation: printed, never compared ────────────────────────────
    drift=$(( actual - observed_bytes ))
    sign="+"
    if [ "$drift" -lt 0 ]; then sign=""; fi
    echo "✅ identity not compared across builds · observed ${observed_bytes} B on ${observed_on} (${observed_at%T*}) · this build ${actual} B (${sign}${drift} B)"
    if [ -n "$(read observed_sha256)" ]; then
        echo "   observed sha256 $(read observed_sha256)"
    fi

    # ── strict, because host-independent ───────────────────────────────────
    if [ "$actual" -gt "$budget" ]; then
        echo "❌ over budget by $((actual - budget)) B (budget ${budget} B, ceiling ${ceiling} B)" >&2
        return 1
    fi
    if [ "$actual" -gt "$ceiling" ]; then
        echo "❌ over the ${ceiling} B ceiling by $((actual - ceiling)) B (actual ${actual} B)" >&2
        return 1
    fi
    echo "✅ within budget ($((budget - actual)) B headroom)"
    return 0
}

# ── Self-test: the four seeds, each shown to fire (#1417) ───────────────────
#
#   bash wasm/ci/check-bundle-size.sh --self-test
#
# K3 is the seed this issue exists for: changing the observation must **pass** and print the
# drift, because an assertion that stops a build over a number nobody can attribute is the
# defect, not the check.
ci_size_selftest() {
    local tmp failures=0 ok=0
    tmp="$(mktemp -d)"
    # `$tmp` is local, and `set -u` turns the EXIT trap's use of it after return into an
    # unbound-variable error — which would fail the harness from the cleanup, not the seed.
    trap 'find "${tmp:-/nonexistent}" -type f -delete 2>/dev/null || true; rmdir "${tmp:-/nonexistent}" 2>/dev/null || true' EXIT

    fixture() {
        local path="$1" measured="$2" budget="$3" ceiling="$4" observed="$5"
        cat >"$path" <<BUDGET
measured_bytes: ${measured}
budget_bytes: ${budget}
ceiling_bytes: ${ceiling}
emitted_bytes: $((measured + 432635))
name_section_bytes: 432635
observed_bytes: ${observed}
observed_on: aarch64-apple-darwin
observed_at: 2026-11-05T13:05:00Z
observed_sha256: 0000000000000000000000000000000000000000000000000000000000000000
BUDGET
    }

    expect() {
        local label="$1" want="$2" file="$3" actual="$4" needle="$5"
        local out status
        # `set -e`: a bare assignment whose command substitution fails would exit the seed
        # harness before it could report, which is how a gate ends up "passing" by dying.
        out="$(gate_numbers "$file" "$actual" 2>&1)" && status=0 || status=$?
        if [ "$status" -ne "$want" ]; then
            echo "  ❌ ${label}: exit ${status}, wanted ${want}" >&2
            printf '%s\n' "$out" | sed 's/^/     /' >&2
            failures=$((failures + 1))
            return
        fi
        case "$out" in
            *"$needle"*) echo "  ✅ ${label}"; ok=$((ok + 1)) ;;
            *)
                echo "  ❌ ${label}: output does not contain '${needle}'" >&2
                printf '%s\n' "$out" | sed 's/^/     /' >&2
                failures=$((failures + 1)) ;;
        esac
    }

    local f="$tmp/budget.txt"
    fixture "$f" 1334325 1667906 1500000 1334325

    # K1 — the ceiling is a hard, host-independent bound, and both numbers are printed.
    expect "K1 ceiling exceeded fails, printing both numbers" 1 "$f" 1500001 "over the 1500000 B ceiling by 1 B (actual 1500001 B)"
    # K2 — a builder path in the artifact names the path (the check lives in builder-paths.sh,
    #      and the seed proves it still catches what the equality used to hide).
    . "$CRATE_DIR/ci/builder-paths.sh"
    printf 'rigorix-gate-wasm\n%s/.cargo/registry/src/x\n' "$HOME" > "$tmp/planted.wasm"
    printf 'rigorix-gate-wasm\n/registry/src/x\n' > "$tmp/clean.wasm"
    if [ -n "$(builder_path_report "$tmp/planted.wasm")" ] && [ -z "$(builder_path_report "$tmp/clean.wasm")" ]; then
        echo "  ✅ K2 a planted builder path is caught, a clean module is not"
        ok=$((ok + 1))
    else
        echo "  ❌ K2 the builder-path assertion no longer distinguishes the two" >&2
        failures=$((failures + 1))
    fi
    # K3 — the assertion-absent seed: a moved observation passes and prints the drift.
    fixture "$f" 1334325 1667906 1500000 1337205
    expect "K3 a different observation passes and prints the drift" 0 "$f" 1334325 "-2880 B"
    # K4 — configuration: a budget below the measurement can never be met.
    fixture "$f" 1334325 1000000 1500000 1334325
    expect "K4 budget below measured is a configuration error" 1 "$f" 1334325 "configuration error"

    if [ "$failures" -ne 0 ]; then
        echo "❌ check-bundle-size self-test failed (${failures})" >&2
        return 1
    fi
    echo "✅ check-bundle-size self-test: ${ok} seeds fired"
    return 0
}

if [ "${1:-}" = "--self-test" ]; then
    ci_size_selftest
    exit $?
fi

STRIP="$CRATE_DIR/ci/strip_name_section.py"

if [ -f "$SHIPPED" ]; then
    # #1353: the presence assertion comes first and is the load-bearing one. The subject
    # of this gate is the artifact a host ships, and a host ships no debug symbols; the
    # size check alone would pass on an unstripped file because unstripped < budget.
    echo "== shipped artifact: $(basename "$SHIPPED") =="
    # Prove the parser can see a `name` section before trusting it to say there is none
    # (`.pi/scripts/validate-release-train.sh` sets the same precedent).
    if ! python3 "$STRIP" --self-test; then
        echo "❌ the strip helper cannot detect a `name` section, so its verdict means nothing" >&2
        exit 1
    fi
    if ! python3 "$STRIP" --check "$SHIPPED"; then
        echo "❌ the artifact under measurement still carries its debug section" >&2
        echo "   the packaging step (ci/run-module-smoke.sh) must strip it before this gate" >&2
        exit 1
    fi
    ACTUAL=$(wc -c <"$SHIPPED" | tr -d ' ')
    gate_numbers "$BUDGET_FILE" "$ACTUAL" || exit 1
    exit 0
fi

echo "== no shipped artifact yet (packaging pending) =="
echo "   expected at pkg/$(basename "$SHIPPED")"
if [ ! -f "$RAW" ]; then
    echo "   building the release cdylib to record the trend..."
    if ! bash "$CRATE_DIR/ci/wasm-build.sh"; then
        echo "❌ the release cdylib does not build, so nothing can be measured" >&2
        exit 1
    fi
fi
RAW_SIZE=$(wc -c <"$RAW" | tr -d ' ')
echo "   release cdylib: ${RAW_SIZE} B (no exports yet — not a shippable number)"
echo "   state: $(read_field state)"
if [ "$RAW_SIZE" -lt 512 ]; then
    echo "   ⚠️  the module exports nothing yet, so this size is not a budget subject."
    echo "      The ceiling starts at the first shipped artifact; from then on this gate"
    echo "      fails on any regression beyond +25%."
fi
echo
echo "Bundle size: trend recorded, ceiling not yet in force (by design, and stated)."
