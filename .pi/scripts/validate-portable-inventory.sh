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
        --gates)     MODE="gates"; shift ;;
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

# ---------------------------------------------------------------------------
# Ratios are derived, not pattern-matched (#1363).
#
# The rule: anything that looks like a ratio, and cannot be derived, is a failure
# with a named reason — never a skip. A guard that silently skips what it does not
# understand is always narrower than the document it certifies, and the ratio it
# skips is exactly the one the record gets wrong.
# ---------------------------------------------------------------------------
RATIOS="$ROOT/wasm/.pi/adr-014-ratios.tsv"

check_ratios() {
    local manifest="$1" lib="$2" adr="$3"
    local ratios="${4:-$RATIOS}"

    [ -f "$ratios" ] || { bad "  $ratios is missing — the record's ratios have no derivations"; return; }
    local local_fail_before="$fail"

    local direct narrow closure mods items deps declared
    read -r direct narrow closure mods items deps < <(derive_counts "$manifest")
    declared="$(grep -cE '^pub mod' "$lib")"
    local excluded_total=$((direct + narrow + closure))
    local excluded_closure_and_narrow=$((narrow + closure))

    # Substitutions available to the derivation rows. Bytes come from the ratios
    # file itself, so the arithmetic below is the only place they are used.
    local subst=""
    subst=" mods=$mods declared=$declared excluded_crate=$direct excluded_narrow=$narrow"
    subst="$subst excluded_closure=$closure excluded_total=$excluded_total"
    subst="$subst excluded_closure_and_narrow=$excluded_closure_and_narrow items=$items deps=$deps"

    # 1. every quantity row's value must be cross-checked where it can be.
    local kind name value
    while IFS=$'\t' read -r kind name value _den _phrase _note; do
        [ "$kind" = "quantity" ] || continue
        case "$name" in
            seam_shipped)
                local budget_file="$ROOT/wasm/ci/bundle-budget.txt"
                if [ -f "$budget_file" ]; then
                    local file_value
                    file_value="$(sed -n 's/^measured_bytes:[[:space:]]*//p' "$budget_file")"
                    [ "$file_value" = "$value" ] \
                        || bad "  ratios: $name is $value but bundle-budget.txt says ${file_value:-<absent>}"
                fi ;;
            seam_budget)
                local budget_file2="$ROOT/wasm/ci/bundle-budget.txt"
                if [ -f "$budget_file2" ]; then
                    local file_budget
                    file_budget="$(sed -n 's/^budget_bytes:[[:space:]]*//p' "$budget_file2")"
                    [ "$file_budget" = "$value" ] \
                        || bad "  ratios: $name is $value but bundle-budget.txt says ${file_budget:-<absent>}"
                fi ;;
        esac
        subst="$subst $name=$value"
    done < "$ratios"

    # 2. each derivation row: the arithmetic must reproduce `stated`, the fragment
    #    must be in the record, and `stated` must sit inside it. The fragment is what
    #    pins the subject — a token on a line with an unrelated "of the …" clause is
    #    exactly the mistake #1363 found.
    local stated num den says note
    while IFS=$'\t' read -r kind stated num den says note; do
        case "$kind" in \#*|quantity|"") continue ;; esac
        local num_v den_v
        num_v="$(resolve_quantity "$num" "$subst")"
        den_v="$(resolve_quantity "$den" "$subst")"
        if [ "$kind" = "total" ]; then
            # A derived count with no denominator: "20 inner items". Only the
            # numerator has to resolve.
            if [ -z "$num_v" ]; then
                bad "  ratios: cannot derive '$stated' — quantity '$num' does not resolve"
                continue
            fi
        elif [ -z "$num_v" ] || [ -z "$den_v" ] || [ "$den_v" = "0" ]; then
            bad "  ratios: cannot derive '$stated' — operands $num/$den do not resolve ($num_v/$den_v)"
            continue
        fi
        case "$kind" in
            share)
                decimals="$(printf '%s' "$stated" | sed -n 's/.*\.\([0-9][0-9]*\).*/\1/p' | tr -d '\n' | wc -c | tr -d ' ')"
                [ "$decimals" -gt 0 ] || decimals=0
                computed="$(awk -v n="$num_v" -v d="$den_v" -v p="$decimals" \
                    'BEGIN { printf "%.*f", p, n / d * 100 }')"
                [ "$computed" = "$(printf '%s' "$stated" | tr -d ' %')" ] \
                    || bad "  ratios: '$stated' does not derive — $num/$den = ${computed} % (row: '$note')" ;;
            growth)
                decimals="$(printf '%s' "$stated" | sed -n 's/.*\.\([0-9][0-9]*\).*/\1/p' | tr -d '\n' | wc -c | tr -d ' ')"
                [ "$decimals" -gt 0 ] || decimals=0
                computed="$(awk -v n="$num_v" -v d="$den_v" -v p="$decimals" \
                    'BEGIN { printf "%.*f", p, (n / d - 1) * 100 }')"
                [ "$computed" = "$(printf '%s' "$stated" | tr -d ' +%')" ] \
                    || bad "  ratios: '$stated' does not derive — $num/$den is +${computed} % (row: '$note')" ;;
            count)
                [ "$stated" = "$num_v of $den_v" ] \
                    || bad "  ratios: '$stated' does not derive — rows give '$num_v of $den_v' (row: '$note')" ;;
            total)
                # A count of a derived quantity, written "N <noun>". The noun stays
                # in `stated`, so the record's own wording is what gets checked.
                case "$stated" in
                    "$num_v "*) : ;;
                    *) bad "  ratios: '$stated' does not derive — the manifest holds $num_v (row: '$note')" ;;
                esac ;;
            *)
                bad "  ratios: unknown kind '$kind' for '$stated'" ;;
        esac
        # The fragment, and the token inside it.
        # No `sed | grep -q`: under `pipefail`, `grep -q` exits at the first match, `sed`
        # takes EPIPE, and the pipeline reports 141 — so the check would fail on a *present*
        # fragment whenever the file grew past a pipe buffer. The fragment is read into a
        # variable instead, which has neither an early exit nor a pipeline.
        normalised="$(sed 's/\*\*//g' "$adr")"
        case "$normalised" in
            *"$says"*) : ;;
            *) bad "  ratios: '$stated' — the record no longer contains its subject fragment \"$says\" (row: '$note')" ;;
        esac
        case "$says" in
            *"$stated"*) : ;;
            *) bad "  ratios: row '$stated' has a fragment that does not contain the token: \"$says\"" ;;
        esac
    done < "$ratios"

    # 3. nothing ratio-shaped may go unchecked, and the check is per site: the token
    #    on line N must be covered by a row whose fragment is also on line N.
    #    Emphasis is stripped first, so `**25 of 38**` reads as `25 of 38`.
    local hits lineno token stripped
    stripped="$(sed 's/\*\*//g' "$adr")"
    hits="$(printf '%s\n' "$stripped" \
        | grep -inEo '[0-9]+(\.[0-9]+)? ?%|[0-9]+ of [0-9]+|[0-9]+ (inner items|items|dependencies)|(^|[^A-Za-z])(half|majority|most|quarter|third|twice|double)([^A-Za-z]|$)' \
        | sed -E 's/^([0-9]+):/\1\t/' \
        | while IFS="$(printf '\t')" read -r n text; do
              printf '%s\t%s\n' "$n" "$(printf '%s' "$text" | sed -E 's/^[^A-Za-z0-9]+//; s/[^A-Za-z0-9%.]+$//' | tr 'A-Z' 'a-z')"
          done | sort -u)"
    while IFS="$(printf '\t')" read -r lineno token; do
        [ -n "$token" ] || continue
        local line covered=0 row_stated row_says
        line="$(printf '%s\n' "$stripped" | sed -n "${lineno}p")"
        while IFS=$'\t' read -r kind row_stated _n _d row_says _note2; do
            case "$kind" in \#*|quantity|"") continue ;; esac
            [ "$(printf '%s' "$row_stated" | tr 'A-Z' 'a-z')" = "$token" ] || continue
            case "$line" in
                *"$row_says"*) covered=1; break ;;
            esac
        done < "$ratios"
        if [ "$covered" -eq 0 ]; then
            case " $hits " in
                *" $token "*) : ;;
            esac
            bad "  ratios: $(basename "$adr"):$lineno states '$token' with no derivation covering that site — cannot derive is a failure, not a skip"
        fi
    done <<EOF
$hits
EOF

    # 3b. Bare counts (#1377). #1374 re-derived every token the ratio check reads and left
    #     the prose beside it describing the previous inventory: "Of those 25, only 11" sat
    #     next to "12 of 22" and "10 of 22" in one paragraph, arithmetically self-consistent
    #     and factually wrong. `N of M` and `N %` have shapes; "25 modules" does not, so a
    #     count noun is matched by NAME against the derived value for that noun.
    #
    #     A count noun the guard cannot derive is a failure with a reason — the rule #1363
    #     applied to ratios, applied to bare integers.
    local crates_value
    crates_value="$(awk -F'\t' '$1=="quantity" && $2=="crates" {print $3}' "$ratios")"
    python3 "$ROOT/.pi/scripts/bare-counts.py" \
        "$adr" "$mods" "$declared" "$items" "$deps" "$narrow" "$direct" "$closure" "${crates_value:-0}" \
        || fail=$((fail + 1))

    local n
    n="$(grep -vcE '^#|^$' "$ratios")"
    if [ "$fail" -eq "$local_fail_before" ]; then
        note "  ratios: $n derivation rows; every ratio-shaped token in the record is covered"
    else
        note "  ratios: $n derivation rows; $(($fail - local_fail_before)) ratio check(s) failed"
    fi
}

# Resolve an operand: a number is itself, a name is looked up in the substitution list.
resolve_quantity() {
    local want="$1" subst="$2" pair
    case "$want" in
        ''|-) printf ''; return ;;
        *[!0-9]*) : ;;
        *) printf '%s' "$want"; return ;;
    esac
    for pair in $subst; do
        case "$pair" in
            "$want"=*) printf '%s' "${pair#*=}"; return ;;
        esac
    done
    printf ''
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
}

run_check() {
    local manifest="$1" lib="$2" adr="$3" ratios="${4:-$RATIOS}"
    fail=0
    check_structure "$manifest"
    check_subjects "$manifest" "$lib"
    check_counts "$manifest" "$lib"
    check_no_restatement "$manifest" "$adr" "$lib"
    check_ratios "$manifest" "$lib" "$adr" "$ratios"
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
    # mutate <from> <to> <out> — a substitution that changed nothing is a **failure**, not a
    # skip. Every seed below is anchored on a literal from the record, so a re-record moves the
    # literal out from under the seed and the seed quietly stops firing: that happened five
    # times in one commit when the counts were re-derived by #1443, and the guard reported
    # "exit 0, wanted 1" for seeds that had never been applied.
    mutate() {
        local from="$1" to="$2" out="$3"
        sed "s/$from/$to/" "$ADR" > "$out"
        if cmp -s "$ADR" "$out"; then
            printf '  %b❌%b seed inert: %s matches nothing in the record\n' "$RED" "$NC" "$from"
            ko=$((ko + 1))
            return 1
        fi
        return 0
    }

    expect() { # expect <label> <wanted-exit> <manifest> <lib> <adr> [ratios]
        local label="$1" want="$2" m="$3" l="$4" a="$5" r="${6:-$RATIOS}"
        run_check "$m" "$l" "$a" "$r" >/dev/null 2>&1
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
    # 22 excluded of 38 declared: the 16 that stay are placeholders, because the
    # ratio's denominator is the count of `pub mod` lines, not the exclusion list.
    for i in $(seq 1 16); do printf 'pub mod kept_%s;\n' "$i"; done >> "$tmp/lib.rs"

    expect "the tree's own manifest" 0 "$MANIFEST" "$LIB" "$ADR"

    # 1. a summary that undercounts the denominator
    mods=''; declared=''; wrong=''
    read -r _mods declared <<< "$(grep -oE '[0-9]+ of [0-9]+ public modules' "$MANIFEST" | head -1 | awk -F' of | public' '{print $1" "$2}')"
    mods="$_mods"
    wrong=$((declared + 1))
    sed -e "s/${mods} of the crate's ${declared} public modules/${mods} of ${wrong} public modules/" \
        -e "s/(${mods} of ${declared} public modules)/(${mods} of ${wrong} public modules)/" "$MANIFEST" > "$tmp/m22.tsv"
    if cmp -s "$MANIFEST" "$tmp/m22.tsv"; then
        printf '  %b❌%b seed inert: the summary line matches nothing\n' "$RED" "$NC"
        ko=$((ko + 1))
    fi
    expect "summary says ${mods} of ${wrong}" 1 "$tmp/m22.tsv" "$LIB" "$ADR"

    # 2. the bucket collapse that caused #1361 (six narrowed modules counted as closure).
    #    The sentence is re-recorded whenever the counts move — it moved in #1545 — so the
    #    seed reads its operands out of the record instead of writing them here: a seed keyed
    #    to `14 direct, 6 by closure` went inert the moment the record said 7.
    bucket="$(grep -oE '= [0-9]+ direct, [0-9]+ by closure,' "$MANIFEST" | head -1)"
    bucket_direct="$(printf '%s' "$bucket" | grep -oE '[0-9]+ direct' | grep -oE '[0-9]+' || true)"
    bucket_closure="$(printf '%s' "$bucket" | grep -oE '[0-9]+ by closure' | grep -oE '[0-9]+' || true)"
    if [ -z "$bucket_direct" ] || [ -z "$bucket_closure" ]; then
        printf '  %b❌%b seed inert: the bucket sentence matches nothing\n' "$RED" "$NC"
        ko=$((ko + 1))
    else
        sed "s/= ${bucket_direct} direct, ${bucket_closure} by closure,/= $((bucket_direct - 3)) indirect,/" \
            "$MANIFEST" > "$tmp/mbucket.tsv"
        if cmp -s "$MANIFEST" "$tmp/mbucket.tsv"; then
            printf '  %b❌%b seed inert: the bucket sentence matches nothing\n' "$RED" "$NC"
            ko=$((ko + 1))
        fi
        expect "summary collapses three buckets into two" 1 "$tmp/mbucket.tsv" "$LIB" "$ADR"
    fi

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

    # 8. #1363 seed A — the prose ratio, inside markdown emphasis, with the table
    #    ratio left correct. Emphasis is stripped before matching, so this cannot hide.
    destated="$(grep -oE '\*\*[0-9]+ of [0-9]+\*\*' "$ADR" | head -1 | tr -d '*')"
    mutate "\*\*${destated}\*\*" "**${mods} of $((declared + 1))**" "$tmp/aprose.md"
    expect "prose ratio wrong while the table is right (#1363 seed A)" 1 "$MANIFEST" "$LIB" "$tmp/aprose.md"

    # 9. #1363 seed B — the table ratio, prose untouched.
    mutate "${mods} of ${declared} public modules (8 forbidden crate" "${mods} of ${wrong} public modules (8 forbidden crate" \
        "$tmp/atable.md"
    expect "table ratio wrong while the prose is right (#1363 seed B)" 1 "$MANIFEST" "$LIB" "$tmp/atable.md"

    # 10. The right percentage with the wrong denominator: the emitted share in a cell whose
    #     subject is the shipped module. Both tokens are read from the ratios file rather than
    #     written here. A seed keyed to a literal went inert three times (#1363 stitched
    #     `34.2 %`, #1408 moved `30.0 %` and then `31.22 %`), each time reporting only that it
    #     was inert, and each time while the tree it ran against was failing for an unrelated
    #     reason. What the seed asserts is the *substitution*, so the substitution is what it
    #     derives; a re-record can move the numbers without disarming the seed.
    # #1385 moved the section-table rows from `data_emitted`/`engine_emitted` to
    # `data_shipped`/`engine_shipped`, so the seed reads the operands the record actually
    # has: the shipped share of `data`, and a share whose denominator is the *emitted*
    # module (`name_section`). Substituting one for the other is still the defect.
    shipped_token="$(awk -F'\t' '$1 == "share" && $3 == "data_shipped" && $4 == "engine_shipped" { print $2; exit }' "$RATIOS")"
    other_token="$(awk -F'\t' '$1 == "share" && $3 == "name_section" && $4 == "engine_emitted" { print $2; exit }' "$RATIOS")"
    if [ -z "$shipped_token" ] || [ -z "$other_token" ] || ! grep -qF -- "$shipped_token" "$ADR" 2>/dev/null; then
        printf '  %b❌%b %s (the seed token matches nothing in the record)\n' "$RED" "$NC" \
            "percentage whose denominator is the wrong module (#1363)"
        ko=$((ko + 1))
    else
        sed "s/${shipped_token//./\\.}/${other_token//./\\.}/g" "$ADR" > "$tmp/adenom.md"
        expect "percentage whose denominator is the wrong module (#1363)" 1 "$MANIFEST" "$LIB" "$tmp/adenom.md"
    fi

    # 11. a ratio-shaped claim with no derivation at all — the rule is a failure
    #     with a reason, never a skip.
    { cat "$ADR"; printf '\nHalf the excluded modules are not dependency statements.\n'; } > "$tmp/aword.md"
    expect "an underivable ratio-shaped claim" 1 "$MANIFEST" "$LIB" "$tmp/aword.md"

    # 12. #1365 seed H — a manifest-derived count, off by one. The guard used to
    #     compute it, print it, and never look at the record's copy.
    mutate '29 inner items' '28 inner items' "$tmp/acountH.md"
    expect "derived count off by one: inner items (#1365 seed H)" 1 "$MANIFEST" "$LIB" "$tmp/acountH.md"

    # 13. #1365 seed I — the dependency count, likewise.
    sed 's/9 dependencies/10 dependencies/' "$ADR" > "$tmp/acountI.md"
    expect "derived count off by one: dependencies (#1365 seed I)" 1 "$MANIFEST" "$LIB" "$tmp/acountI.md"

    # 14. #1377 seed A — a pronoun count that never moved with #1374. It sat next to the
    #     ratios that did move, in one arithmetically self-consistent paragraph.
    those="$(grep -oE 'Of those [0-9]+' "$ADR" | head -1 | grep -oE '[0-9]+$' || true)"
    if [ -z "$those" ]; then
        printf '  %b❌%b seed inert: Of those <n> matches nothing in the record\n' "$RED" "$NC"
        ko=$((ko + 1))
    else
        mutate "Of those ${those}" "Of those $((those + 5))" "$tmp/abareA.md"
        expect "bare count: 'Of those $((those + 5))' (#1377 seed A)" 1 "$MANIFEST" "$LIB" "$tmp/abareA.md"
    fi

    # 15. #1377 seed B — the elliptical form, wrapped across two lines: `only 10` on one,
    #     `leave for a forbidden crate` on the next. Per-line matching read it as a bare
    #     number with no subject, which is how it survived the first version of this check.
    mutate 'only 8' 'only 11' "$tmp/abareB.md"
    expect "bare count: 'only 11 leave for a forbidden crate' (#1377 seed B)" 1 \
        "$MANIFEST" "$LIB" "$tmp/abareB.md"

    # 16. #1377 seed C — a plain noun count with a stale value.
    sed 's/and \*\*9 dependencies\*\* removed/and **8 dependencies** removed/' "$ADR" > "$tmp/abareC.md"
    expect "bare count: '8 dependencies' (#1377 seed C)" 1 "$MANIFEST" "$LIB" "$tmp/abareC.md"

    printf '  self-test: %d/%d assertions\n' "$ok" "$((ok + ko))"
    [ "$ko" -eq 0 ] || exit 1
    exit 0
    ;;
gates)
    # ── ADR-014 §1 versus its implementation (#1374) ──────────────────────
    # The manifest is only a decision until the code carries it. This mode asks the two
    # questions the compiler cannot: does every `pub mod` the slice excludes have a row,
    # and does every `cfg(not(feature = "portable"))` site sit in a module the manifest
    # names? A gate with no row is an exclusion nobody reviewed; a row with no gate is a
    # claim about a build that still compiles the module.
    python3 - "$MANIFEST" "$LIB" "$ROOT/engine/src" <<'PYGATES'
import os, re, sys

manifest, lib, src = sys.argv[1], sys.argv[2], sys.argv[3]

mod_rows, item_rows = set(), set()
for line in open(manifest, encoding='utf-8'):
    if line.startswith('#'):
        continue
    cols = line.rstrip('\n').split('\t')
    if len(cols) < 2:
        continue
    if cols[0] == 'module':
        mod_rows.add(cols[1])
    elif cols[0] == 'item':
        item_rows.add(cols[1])

GATE = re.compile(r'#\[cfg\(not\(feature = "portable"\)\)\]')
sites = []           # (path, lineno, name)
for root, _dirs, files in os.walk(src):
    for fn in files:
        if not fn.endswith('.rs'):
            continue
        path = os.path.join(root, fn)
        lines = open(path, encoding='utf-8').read().splitlines()
        for i, line in enumerate(lines):
            if not GATE.search(line):
                continue
            for j in range(i + 1, min(i + 4, len(lines))):
                m = re.match(r'\s*(?:pub )?(mod|use|struct|enum|fn|impl|trait|const|static|type)\s+'
                             r'([A-Za-z0-9_]+)', lines[j])
                if m:
                    sites.append((path, i + 1, m.group(2), m.group(1)))
                    break
                if lines[j].strip():
                    break

problems = []

lib_gated = set()
lines = open(lib, encoding='utf-8').read().splitlines()
for i, line in enumerate(lines):
    if GATE.search(line):
        for j in range(i + 1, min(i + 3, len(lines))):
            m = re.match(r'\s*pub mod ([A-Za-z0-9_]+);', lines[j])
            if m:
                lib_gated.add(m.group(1))
                break

for m in sorted(lib_gated - mod_rows):
    problems.append(f"engine/src/lib.rs: `pub mod {m};` is gated but has no `module` row in the manifest")
for m in sorted(mod_rows - lib_gated):
    problems.append(f"{manifest}: module row `{m}` has no cfg(not(feature = \"portable\")) gate in "
                    f"engine/src/lib.rs — the module still compiles into the slice")

# Gates are judged per FILE, not per site. A file's gates are one decision: `use std::path::Path`
# next to `pub use runner::ProcessPreconditionRunner` is the same exclusion seen twice, and the
# manifest records the subject once. An internal import gate on its own can exclude nothing.
by_file = {}
for path, lineno, name, kind in sites:
    rel = os.path.relpath(path, src)
    parts = rel[:-3].split(os.sep)
    if parts[-1] == 'lib':
        parts = []                       # the crate root names top-level modules
    elif parts[-1] == 'mod':
        parts = parts[:-1]
    base = {'::'.join(parts[:k]) for k in range(1, len(parts) + 1)} if parts else set()
    if not parts:
        candidates = {name}
    else:
        # `x/mod.rs` gating `mod name;` or `pub use name::…` names its child: x::name.
        candidates = base | {f"{m}::{name}" for m in base}
    by_file.setdefault(rel, []).append((lineno, name, candidates, base))

for rel, entries in sorted(by_file.items()):
    covered = False
    for _lineno, _name, candidates, base in entries:
        if (candidates & mod_rows) or (candidates & item_rows) or (base & mod_rows) or (base & item_rows):
            covered = True
            break
    if covered:
        continue
    lineno, name, candidates, _base = entries[0]
    problems.append(f"engine/src/{rel}:{lineno}: gate on `{name}` has no manifest row "
                    f"(looked for {sorted(candidates)})")

print(f"gates: {len(lib_gated)} gated pub mod, {len(sites)} gate sites, "
      f"{len(mod_rows)} module rows, {len(item_rows)} item rows")
for p in problems:
    print(f"❌ {p}")
sys.exit(1 if problems else 0)
PYGATES
    rc=$?
    if [ "$rc" -eq 0 ]; then
        printf '%b✅%b every excluded module is gated and every gate has a row\n' "$GREEN" "$NC"
        exit 0
    fi
    printf '%bsee the rows above%b\n' "$RED" "$NC" >&2
    exit 1
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
