#!/usr/bin/env bash
# ============================================================================
# validate-portable-inventory.sh — the slice's counts are counted, not claimed
# ============================================================================
# `engine/.pi/portable-inventory.tsv` is the ADR-014 evidence for what
# `--features portable` excludes: one row per excluded dependency, module and
# inner item. Its summary block is prose, and prose drifts from the rows under
# it — which is what happened (#1361): the summary said "22 of 40 public
# modules (11 direct, 11 indirect)" while the rows held 25 of 38, because the
# second bucket had been estimated rather than counted and had swallowed the
# six modules a narrowed `tokio` feature set removes.
#
# This guard checks four things:
#
#   * structure — 6 tab-separated columns, a known `layer`, a numeric
#     `measured-sites` (or the explicit `—`);
#   * subject — every `module` names a real `pub mod` in `engine/src/lib.rs`,
#     every `item` path resolves under `engine/src/`;
#   * counts — the three trigger buckets, their total, and the denominator are
#     recomputed from the rows and compared with what the summary claims;
#   * no second authority — a document that cites the manifest must not restate
#     its list (ADR-014 does), because two copies drift.
#
# Usage:
#   .pi/scripts/validate-portable-inventory.sh              # check (gating)
#   .pi/scripts/validate-portable-inventory.sh --counts     # print the derived counts
#   .pi/scripts/validate-portable-inventory.sh --self-test  # prove each check bites
#   .pi/scripts/validate-portable-inventory.sh --manifest F --lib L --adr A
#                                                           # check other inputs
#                                                           # (used by --self-test)
# ============================================================================
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MANIFEST="$ROOT/engine/.pi/portable-inventory.tsv"
LIB="$ROOT/engine/src/lib.rs"
ADR="$ROOT/wasm/.pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

MODE="check"
while [ $# -gt 0 ]; do
    case "$1" in
        --self-test) MODE="self-test"; shift ;;
        --counts)    MODE="counts"; shift ;;
        --manifest)  MANIFEST="$2"; shift 2 ;;
        --lib)       LIB="$2"; shift 2 ;;
        --adr)       ADR="$2"; shift 2 ;;
        -h|--help)   sed -n '2,40p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

fail=0
note() { printf '%s\n' "$*"; }
bad()  { printf '%b%s%b\n' "$RED" "$*" "$NC" >&2; fail=$((fail + 1)); }

# --- derived counts ---------------------------------------------------------
# forbidden-crate · narrowed-tokio · closure · module-total · item-total · dep-total
derive_counts() {
    awk -F'\t' '
        $1 == "module" { mods++
                         if ($4 ~ /tree_sitter|reqwest|prometheus/) direct++
                         else if ($4 ~ /tokio/) narrow++
                         else closure++ }
        $1 == "item"   { items++ }
        $1 == "dependency" { deps++ }
        END { printf "%d %d %d %d %d %d\n", direct+0, narrow+0, closure+0, mods+0, items+0, deps+0 }
    ' "$1"
}

# --- the checks -------------------------------------------------------------
check_structure() {
    local manifest="$1"
    local lineno=0
    while IFS=$'\t' read -r layer subject excluded why replaces sites; do
        lineno=$((lineno + 1))
        case "$layer" in \#*|"") continue ;; esac
        case "$layer" in
            dependency|module|item) ;;
            *) bad "  line $lineno: unknown layer '$layer' (want dependency|module|item)"; continue ;;
        esac
        [ -n "${subject:-}" ] || bad "  line $lineno: empty subject"
        [ -n "${excluded:-}" ] || bad "  line $lineno: empty 'excluded' column"
        [ -n "${why:-}" ] || bad "  line $lineno: empty 'why' column"
        [ -n "${replaces:-}" ] || bad "  line $lineno: empty 'replaces-with' column"
        # Read the last field directly: a stray tab would shift the split, so verify against awk.
        true
    done < "$manifest"

    # Column count and numeric sites, checked with awk so a stray tab cannot hide.
    awk -F'\t' -v bad_prefix="$RED" '
        /^#/ || NF == 0 { next }
        NF != 6 { printf "%s  line %d: %d tab-separated columns, want 6%s\n", bad_prefix, NR, NF, "\033[0m"; errs++ }
        $6 !~ /^[0-9]+$/ && $6 != "—" { printf "%s  line %d: measured-sites \"%s\" is neither numeric nor —%s\n", bad_prefix, NR, $6, "\033[0m"; errs++ }
        END { exit (errs > 0 ? 1 : 0) }
    ' "$manifest" || fail=$((fail + 1))
}

check_subjects() {
    local manifest="$1" lib="$2"
    local declared
    declared="$(grep -cE '^pub mod' "$lib")"
    [ "$declared" -gt 0 ] || { bad "  $lib declares no public modules — wrong file?"; return; }

    while IFS=$'\t' read -r layer subject _rest; do
        case "$layer" in \#*|"") continue ;; esac
        case "$layer" in
            module)
                grep -qE "^pub mod ${subject};" "$lib" \
                    || bad "  module '$subject' is listed but not declared as 'pub mod $subject;' in lib.rs"
                ;;
            item)
                # `a::b::c` → engine/src/a/b/{c.rs,c/mod.rs}
                local path="${subject//://}" base
                base="$(dirname "$lib")"
                if [ -e "$base/$path.rs" ] || [ -e "$base/$path/mod.rs" ]; then :; else
                    bad "  item '$subject' does not resolve under $base/"
                fi
                ;;
        esac
    done < "$manifest"
}

check_counts() {
    local manifest="$1" lib="$2"
    local direct narrow closure mods items deps declared
    read -r direct narrow closure mods items deps < <(derive_counts "$manifest")
    declared="$(grep -cE '^pub mod' "$lib")"

    # The summary block must state the counts the rows produce. Each assertion
    # is a literal the summary has to contain — computed, never trusted.
    local total_direct=$((direct + narrow))
    grep -q "${mods} of ${declared} public modules" "$manifest" \
        || bad "  summary does not state '${mods} of ${declared} public modules' (rows give ${mods}, lib.rs declares ${declared})"
    grep -q "${direct} for a forbidden crate" "$manifest" \
        || bad "  summary does not state '${direct} for a forbidden crate' (rows give ${direct})"
    grep -q "${narrow} for the narrowed" "$manifest" \
        || bad "  summary does not state '${narrow} for the narrowed tokio feature set' (rows give ${narrow})"
    grep -q "${closure} by closure" "$manifest" \
        || bad "  summary does not state '${closure} by closure' (rows give ${closure})"
    grep -q "${items} items gated" "$manifest" \
        || bad "  summary does not state '${items} items gated' (rows give ${items})"

    # The bucket sentence must name the three, not collapse them into two.
    grep -qE "(${direct} for a forbidden crate|${total_direct} direct)" "$manifest" \
        || bad "  summary collapses the trigger buckets; name all three"

    note "  derived: ${direct} forbidden-crate + ${narrow} narrowed-tokio = ${total_direct} direct, ${closure} closure · ${mods} of ${declared} modules · ${items} items · ${deps} dependencies"
}

check_no_restatement() {
    local manifest="$1" adr="$2" lib="$3"
    [ -f "$adr" ] || { bad "  $adr is missing — ADR-014 is the record that cites this manifest"; return; }

    grep -q "portable-inventory.tsv" "$adr" \
        || bad "  $(basename "$adr") does not cite engine/.pi/portable-inventory.tsv"

    # Two copies drift: if the record reproduces the exclusion list, the list has
    # two authorities. Allow a couple of illustrative names, fail on more.
    local named=0 module
    while IFS=$'\t' read -r layer subject _rest; do
        case "$layer" in module) ;; *) continue ;; esac
        if grep -qw -- "$subject" "$adr"; then named=$((named + 1)); module="$subject"; fi
    done < "$manifest"
    if [ "$named" -gt 3 ]; then
        bad "  $(basename "$adr") names $named excluded modules (e.g. '$module') — cite the manifest, do not restate it"
    fi
    note "  citation: $(basename "$adr") names $named excluded modules (limit 3)"

    # A number in the record is a claim: if it states the ratio, it must be the
    # derived one. This is the check that #1361 needed and did not have.
    local ratio
    ratio="$(grep -oE '[0-9]+ of [0-9]+ public modules' "$adr" | head -1)"
    if [ -n "$ratio" ]; then
        local direct narrow closure mods items deps declared want
        read -r direct narrow closure mods items deps < <(derive_counts "$manifest")
        declared="$(grep -cE '^pub mod' "$lib")"
        want="${mods} of ${declared} public modules"
        [ "$ratio" = "$want" ] || bad "  $(basename "$adr") states '$ratio' but the rows give '$want'"
        note "  record's ratio: $ratio"
    fi
}

run_check() {
    local manifest="$1" lib="$2" adr="$3"
    fail=0
    check_structure "$manifest"
    check_subjects "$manifest" "$lib"
    check_counts "$manifest" "$lib"
    check_no_restatement "$manifest" "$adr" "$lib"
}

# --- modes ------------------------------------------------------------------
case "$MODE" in
counts)
    read -r direct narrow closure mods items deps < <(derive_counts "$MANIFEST")
    declared="$(grep -cE '^pub mod' "$LIB")"
    printf 'forbidden-crate %d\nnarrowed-tokio %d\nclosure %d\nmodule-total %d\nmodule-total-all %d\ndeclared %d\nitems %d\ndependencies %d\n' \
        "$direct" "$narrow" "$closure" "$mods" "$((direct + narrow + closure))" "$declared" "$items" "$deps"
    exit 0
    ;;
self-test)
    tmp="$(mktemp -d)"
    trap 'find "$tmp" -type f -delete; rmdir "$tmp"' EXIT
    ok=0; ko=0
    expect() { # expect <label> <wanted-exit> <manifest> <lib> <adr>
        local label="$1" want="$2" m="$3" l="$4" a="$5"
        run_check "$m" "$l" "$a" >/dev/null 2>&1
        if [ "$want" = "0" ] && [ "$fail" -eq 0 ]; then
            printf '  %b✅%b %s\n' "$GREEN" "$NC" "$label"; ok=$((ok + 1))
        elif [ "$want" = "1" ] && [ "$fail" -gt 0 ]; then
            printf '  %b✅%b %s (fails, as it must)\n' "$GREEN" "$NC" "$label"; ok=$((ok + 1))
        else
            printf '  %b❌%b %s (exit %s, wanted %s)\n' "$RED" "$NC" "$label" "$fail" "$want"; ko=$((ko + 1))
        fi
    }
    cp "$MANIFEST" "$tmp/m.tsv"; cp "$ADR" "$tmp/a.md"
    printf '# a lib with the right declarations\n' > "$tmp/lib.rs"
    awk -F'\t' '$1=="module"{print "pub mod " $2 ";"}' "$MANIFEST" >> "$tmp/lib.rs"

    expect "the tree's own manifest" 0 "$MANIFEST" "$LIB" "$ADR"

    # 1. a summary that undercounts the denominator
    sed -e 's/25 of the crate'"'"'s 38 public modules/22 of 40 public modules/' \
        -e 's/(25 of 38 public modules)/(22 of 40 public modules)/' "$MANIFEST" > "$tmp/m22.tsv"
    expect "summary says 22 of 40" 1 "$tmp/m22.tsv" "$LIB" "$ADR"

    # 2. the bucket collapse that caused #1361 (six narrowed modules counted as closure)
    sed 's/= 17 direct, 8 by closure,/= 11 indirect,/' "$MANIFEST" > "$tmp/mbucket.tsv"
    expect "summary collapses three buckets into two" 1 "$tmp/mbucket.tsv" "$LIB" "$ADR"

    # 3. a row naming a module that does not exist
    cp "$MANIFEST" "$tmp/mghost.tsv"
    printf 'module\tghost_module\twhole module\treqwest\t—\t0\n' >> "$tmp/mghost.tsv"
    expect "a module row that names nothing real" 1 "$tmp/mghost.tsv" "$LIB" "$ADR"

    # 4. a row with a missing column
    cp "$MANIFEST" "$tmp/mcol.tsv"
    printf 'module\tplanning\twhole module\treqwest\n' >> "$tmp/mcol.tsv"
    expect "a row with 4 columns" 1 "$tmp/mcol.tsv" "$LIB" "$ADR"

    # 5. an ADR that restates the list
    cp "$MANIFEST" "$tmp/mrestate.tsv"
    { cat "$ADR"; awk -F'\t' '$1=="module"{print "- `" $2 "` out"}' "$MANIFEST"; } > "$tmp/arestate.md"
    expect "ADR restates the exclusion list" 1 "$MANIFEST" "$LIB" "$tmp/arestate.md"

    # 6. an ADR that does not cite the manifest
    grep -v "portable-inventory.tsv" "$ADR" > "$tmp/anocite.md"
    expect "ADR does not cite the manifest" 1 "$MANIFEST" "$LIB" "$tmp/anocite.md"

    # 7. the denominator drifting in lib.rs (a module removed but the summary unchanged)
    grep -v '^pub mod hooks;' "$tmp/lib.rs" > "$tmp/lib37.rs"
    expect "lib.rs declares one module fewer than the summary claims" 1 "$MANIFEST" "$tmp/lib37.rs" "$ADR"

    printf '  self-test: %d/%d assertions\n' "$ok" "$((ok + ko))"
    [ "$ko" -eq 0 ] || exit 1
    exit 0
    ;;
check)
    note "── portable inventory ──"
    run_check "$MANIFEST" "$LIB" "$ADR"
    if [ "$fail" -eq 0 ]; then
        printf '%b✅%b manifest, subjects, counts and the single citation agree\n' "$GREEN" "$NC"
        exit 0
    fi
    printf '%s%d check(s) failed%s\n' "$RED" "$fail" "$NC" >&2
    exit 1
    ;;
esac
