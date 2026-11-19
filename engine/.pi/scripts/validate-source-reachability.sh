#!/usr/bin/env bash
# ============================================================================
# validate-source-reachability.sh — a file nobody declares is a claim nobody reads
# ============================================================================
# `mcp/src/engine_wiring.rs` sat in the tree for months as a second engine
# composition: `#[cfg(feature = "real-engine")]` on a feature no manifest
# defined, `build_engine_facade` with one mention in the workspace (its own
# definition), a `panic!` where the fifth composition's failure would be. No
# module declared it, so **rustc never compiled it**, which is exactly why no
# guard could see it: `one_assembly.rs` scans the code that compiles, and the
# panic baseline is regenerated from a build. A file outside every module tree
# is invisible to both, and to `cargo fmt`, `clippy` and `cargo test`.
#
# This guard is the general form (#1637): every `*.rs` under a crate's `src/`
# must be reachable from its crate root, i.e. some file in the same crate must
# declare `mod <stem>;`. The tree has no `#[path]` and no `include!` in a
# module position, so the stem is the module name — if either appears, the
# scanner's assumption is wrong and the allow-list row says why.
#
# The allow-list (`.pi/source-reachability-allowlist.tsv`) carries a *reason*
# per row, so an unreachable file is a decision with a sentence, not a silence.
#
# Usage:
#   .pi/scripts/validate-source-reachability.sh              # check (gating)
#   .pi/scripts/validate-source-reachability.sh --self-test  # prove it bites
#   .pi/scripts/validate-source-reachability.sh --list       # print allow-list
# ============================================================================
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ALLOWLIST="$ROOT/.pi/source-reachability-allowlist.tsv"
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

CRATES="engine mcp cli actions server wasm"

scan() {
    # $1 = repo root. Prints one `path<TAB>stem` line per unreachable file.
    #
    # One grep per crate, not one per file. The first version ran `grep -rlE "mod <stem>;"` for every
    # file, which is quadratic: it took the `guards` stage from ~1m16s to over 4m and blew its ceiling
    # — a guard that makes the gate it lives in too slow is a guard that gets switched off. The
    # declarations are collected once and matched as a set.
    local root="$1" crate src file stem decls
    for crate in $CRATES; do
        src="$root/$crate/src"
        [ -d "$src" ] || continue
        decls="$(grep -rhoE '^[[:space:]]*(pub([[:space:]]*\([^)]*\))?[[:space:]]+)?mod[[:space:]]+[A-Za-z0-9_]+[[:space:]]*;' \
            "$src" 2>/dev/null | sed -E 's/.*mod[[:space:]]+([A-Za-z0-9_]+).*/\1/' | sort -u)"
        while IFS= read -r file; do
            case "$file" in
                */mod.rs|*/lib.rs|*/main.rs) continue ;;
                */src/bin/*) continue ;;   # cargo targets, declared by the manifest
            esac
            stem="$(basename "$file" .rs)"
            if ! printf '%s\n' "$decls" | grep -qx "$stem"; then
                printf '%s\t%s\n' "${file#"$root"/}" "$stem"
            fi
        done < <(find "$src" -name '*.rs' -type f | sort)
    done
}

allowlisted() {
    # $1 = path, $2 = allowlist file. A row is `path<TAB>reason`.
    local path="$1" file="$2"
    [ -f "$file" ] || return 1
    awk -F'\t' -v p="$path" '
        /^#/ { next }
        NF >= 2 && $1 == p && length($2) > 0 { found = 1 }
        END { exit(found ? 0 : 1) }
    ' "$file"
}

report() {
    local root="$1" allowlist="$2" failures=0 orphans=0 path stem
    while IFS=$'\t' read -r path stem; do
        orphans=$((orphans + 1))
        if allowlisted "$path" "$allowlist"; then
            printf '  %b·%b %s — allow-listed with a reason\n' "$YELLOW" "$NC" "$path"
        else
            printf '  %b❌%b %s — no `mod %s;` in its crate and no allow-list row\n' \
                "$RED" "$NC" "$path" "$stem"
            failures=$((failures + 1))
        fi
    done < <(scan "$root")

    if [ "$failures" -eq 0 ]; then
        printf '  %b✅%b every source file under a crate'"'"'s src/ is reachable from its root (%d allow-listed)\n' \
            "$GREEN" "$NC" "$orphans"
    fi
    printf 'source-reachability: %d unreachable file(s), %d unresolved\n' "$orphans" "$failures"
    return "$failures"
}

if [ "${1:-}" = '--list' ]; then
    if [ -f "$ALLOWLIST" ]; then
        grep -vE '^#' "$ALLOWLIST" | awk -F'\t' 'NF>=2 { printf "  %-52s %s\n", $1, $2 }'
    else
        printf '  (no allow-list)\n'
    fi
    exit 0
fi

if [ "${1:-}" = '--self-test' ]; then
    # The plant is a *real* orphan in a copy of a real crate's src/, plus a control: the same
    # copy without the plant must pass. Both halves are needed — a scanner that reports everything
    # would pass the first assertion alone.
    tmp="$(mktemp -d)"
    trap 'find "$tmp" -mindepth 1 -delete 2>/dev/null; rmdir "$tmp" 2>/dev/null' EXIT
    ko=0

    mkdir -p "$tmp/tree/.pi" "$tmp/tree/mcp"
    cp -R "$ROOT/mcp/src" "$tmp/tree/mcp/"
    : > "$tmp/tree/.pi/source-reachability-allowlist.tsv"

    if ! out="$(report "$tmp/tree" "$tmp/tree/.pi/source-reachability-allowlist.tsv" 2>&1)"; then
        printf '  %b❌%b control failed: an unmodified copy of mcp/src reports orphans\n' "$RED" "$NC"
        printf '%s\n' "$out" | grep '❌' | head -5
        ko=$((ko + 1))
    else
        printf '  %b✅%b the control passes: no orphan in mcp/src as it stands\n' "$GREEN" "$NC"
    fi

    printf '//! a composition nobody declared\n' > "$tmp/tree/mcp/src/planted_orphan.rs"
    out="$(report "$tmp/tree" "$tmp/tree/.pi/source-reachability-allowlist.tsv" 2>&1)" || status=$?
    if printf '%s' "$out" | grep -q 'mcp/src/planted_orphan.rs'; then
        printf '  %b✅%b a file no module declares is reported (fails, as it must)\n' "$GREEN" "$NC"
    else
        printf '  %b❌%b a file no module declares was not reported\n' "$RED" "$NC"
        ko=$((ko + 1))
    fi

    # And the allow-list is honoured: the same orphan with a row and a reason is not a failure.
    printf 'mcp/src/planted_orphan.rs\ta fixture the self-test keeps on purpose\n' \
        > "$tmp/tree/.pi/source-reachability-allowlist.tsv"
    if report "$tmp/tree" "$tmp/tree/.pi/source-reachability-allowlist.tsv" >/dev/null 2>&1; then
        printf '  %b✅%b an allow-listed orphan with a reason passes\n' "$GREEN" "$NC"
    else
        printf '  %b❌%b an allow-listed orphan with a reason still failed\n' "$RED" "$NC"
        ko=$((ko + 1))
    fi

    # A row with no reason is not a decision, so it must not silence anything.
    printf 'mcp/src/planted_orphan.rs\t\n' > "$tmp/tree/.pi/source-reachability-allowlist.tsv"
    if report "$tmp/tree" "$tmp/tree/.pi/source-reachability-allowlist.tsv" >/dev/null 2>&1; then
        printf '  %b❌%b an allow-list row with no reason silenced a finding\n' "$RED" "$NC"
        ko=$((ko + 1))
    else
        printf '  %b✅%b an allow-list row with no reason is refused (fails, as it must)\n' "$GREEN" "$NC"
    fi

    total=4
    printf 'self-test: %d/%d assertions\n' "$((total - ko))" "$total"
    exit $(( ko > 0 ? 1 : 0 ))
fi

report "$ROOT" "$ALLOWLIST"
exit $?