#!/usr/bin/env bash
# check-bundle-size.sh — NFR-002 / criterion 23: measure, then gate at +25%.
#
# Two modes, and the difference matters:
#
#   shipped  — a shipped artifact exists (pkg/*.wasm). Three assertions, all required:
#              (1) it carries no `name` custom section — size alone cannot prove the
#                  strip ran, since an unstripped module is *smaller* than the budget
#                  (#1353); (2) measured_bytes and budget_bytes, read from
#                  ci/bundle-budget.txt, are not exceeded by +25 %; (3) the record's own
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
    # #1408: the artifact is portable, so its size is a property of the source and the pinned
    # toolchain. Assert the property the tolerance used to stand in for: no path from the host
    # that built it. The prefixes come from `ci/builder-paths.sh`, shared with the packaging
    # path, because a gate that checks `$HOME` alone passes an artifact that still names its
    # toolchain directory.
    # shellcheck source=builder-paths.sh
    . "$CRATE_DIR/ci/builder-paths.sh"
    if ! BUILDER_PATHS="$(builder_path_report "$SHIPPED")"; then
        exit 1
    fi
    if [ -n "$BUILDER_PATHS" ]; then
        while IFS="$(printf '\t')" read -r count prefix; do
            echo "❌ ${count} occurrence(s) of the builder path ${prefix} — the artifact is not machine-independent" >&2
        done <<EOF
$BUILDER_PATHS
EOF
        echo "   the packaging path must remap the registry, the toolchain and the sysroot (ci/wasm-build.sh, #1408)" >&2
        exit 1
    fi
    echo "   emitted ${EMITTED} B · name section ${NAME_SECTION} B · measured ${MEASURED} B · budget ${BUDGET} B · ceiling ${CEILING} B · actual ${ACTUAL} B"
    # #1386: `measured_bytes` claims to *be* the measurement, so the artifact must match it.
    # Without this, a stale floor is invisible: the budget below is derived from a number
    # nobody checked, and it drifts with every rebuild that changes size.
    #
    # #1405 put a 1 % tolerance here because absolute build paths made the same commit measure
    # 1 337 205 B on macOS and 1 335 752 B on Linux. #1408 removed the cause — the build now
    # remaps both toolchain prefixes and both gates assert no builder path remains — so the
    # comparison is exact again. A tolerance would only re-hide a remap that stopped working.
    if [ "$ACTUAL" -ne "$MEASURED" ]; then
        echo "❌ the recorded measurement is not this artifact: actual ${ACTUAL} B, recorded ${MEASURED} B" >&2
        echo "   delta $(( ACTUAL - MEASURED )) B — re-record measured_bytes from the artifact this build" >&2
        echo "   produced, then derive budget_bytes as measured + 25 % (#1408, #1409)" >&2
        exit 1
    fi
    if [ "${BUDGET:-0}" -eq 0 ]; then
        echo "❌ a shipped artifact exists but no budget was set: measure it first" >&2
        exit 1
    fi
    if [ "$ACTUAL" -gt "$BUDGET" ]; then
        echo "❌ over budget by $((ACTUAL - BUDGET)) B (limit = measured + 25%)" >&2
        exit 1
    fi
    # The +25 % ratchet is derived; the ceiling is the number the specification fixed, and a
    # raised ratchet must never carry an artifact past it (#1386).
    if [ "$ACTUAL" -gt "$CEILING" ]; then
        echo "❌ over the ${CEILING} B ceiling by $((ACTUAL - CEILING)) B" >&2
        exit 1
    fi
    echo "✅ within budget ($((BUDGET - ACTUAL)) B headroom)"
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
