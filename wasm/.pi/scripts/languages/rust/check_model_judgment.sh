#!/usr/bin/env bash
# ============================================================================
# check_model_judgment.sh — Rust (ADR-008/009)
# Deterministic hard gates:
#   dip-domain           domain/ (crate/module) imports sqlx/diesel/web frameworks
#   interface-ownership  (tactical) infra impl without a domain/-owned trait
#   adapter-purity       (adapter modules) model code in generic subdomains
# Usage: bash check_model_judgment.sh [--root <dir>]
# ============================================================================
set -uo pipefail
ROOT="$(pwd)"
if [ "${1:-}" = "--root" ] && [ -n "${2:-}" ]; then ROOT="$2"; fi
cd "$ROOT" || exit 1
VIOLATIONS=0
report() { echo "$1:$2: [$3] $4"; VIOLATIONS=$((VIOLATIONS + 1)); }
[ -d src ] || { echo "model-judgment (rust): no src/"; exit 0; }

while IFS= read -r f; do
    [ -n "$f" ] || continue
    case "$f" in */domain/*)
        while IFS=: read -r ln rest; do
            case "$rest" in
                *sqlx*|*diesel*|*sea_orm*|*axum*|*actix*|*tokio-postgres*|*mongodb*|*redis*)
                    report "$f" "$ln" "dip-domain" "domain imports framework/persistence crate: $(echo "$rest" | sed 's/^use //; s/;.*//')" ;;
            esac
        done < <(grep -nE "^use |^extern crate " "$f" || true) ;;
    esac
done < <(find src -type f -name "*.rs" 2>/dev/null | grep -vE "_test\.rs$|/tests?/" || true)

if [ -d ".pi/architecture/modules" ]; then
    for doc in .pi/architecture/modules/*.md; do
        [ -f "$doc" ] || continue
        doc_name="$(basename "$doc" .md)"
        depth="$(grep -m1 '^\*\*Modeling Depth:\*\*' "$doc" | sed 's/.*\*\*Modeling Depth:\*\* *//' | cut -d' ' -f1 || true)"
        docnorm="$(echo "$doc_name" | sed 's/^module[-_]//' | tr '[:upper:]' '[:lower:]' | tr '-' '_')"
        match_dir=""
        while IFS= read -r d; do
            norm="$(basename "$d" | tr '[:upper:]' '[:lower:]' | tr '-' '_')"
            if [ "$norm" = "$docnorm" ]; then match_dir="$d"; break; fi
        done < <(find src -mindepth 1 -maxdepth 1 -type d 2>/dev/null || true)
        [ -n "$match_dir" ] || continue
        if [ "$depth" = "adapter" ]; then
            for layer in domain application; do
                while IFS= read -r f; do [ -n "$f" ] || continue; report "$f" "1" "adapter-purity" "generic subdomain '${doc_name}' must not contain ${layer}/ model code (ADR-008)"; done < <(find "$match_dir/$layer" -type f -name "*.rs" 2>/dev/null || true)
            done
        elif [ "$depth" = "tactical" ]; then
            while IFS= read -r impl; do
                [ -n "$impl" ] || continue
                simple="$(basename "$impl" .rs)"
                stem="$(echo "$simple" | sed -E 's/_impl$//')"
                if ! grep -rqiE "(pub )?trait ${stem}\\b|trait ${simple}\\b|${stem}" "$match_dir/domain" 2>/dev/null; then
                    report "$impl" "1" "interface-ownership" "repository/adapter impl without a domain/-owned trait (${simple})"
                fi
            done < <(find "$match_dir/infrastructure" -type f \( -name "*repository*.rs" -o -name "*repo*.rs" -o -name "*gateway*.rs" \) 2>/dev/null || true)
        fi
    done
fi

if [ "$VIOLATIONS" -gt 0 ]; then echo "model-judgment (rust): ${VIOLATIONS} violation(s)" >&2; exit 1; fi
echo "model-judgment (rust): all gates passed (dip-domain, interface-ownership, adapter-purity)"
exit 0
