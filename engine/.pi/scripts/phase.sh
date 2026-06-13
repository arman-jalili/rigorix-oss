#!/usr/bin/env bash
# ============================================================================
# phase.sh — Query the Phase Manifest
#
# Usage:
#   bash .pi/scripts/phase.sh list [phase]    — List modules in a phase
#   bash .pi/scripts/phase.sh module <name>   — Show which phase a module belongs to
#   bash .pi/scripts/phase.sh status          — Show implementation status
#   bash .pi/scripts/phase.sh help            — Show this help
#
# Examples:
#   bash .pi/scripts/phase.sh list phase0
#   bash .pi/scripts/phase.sh module cancellation
#   bash .pi/scripts/phase.sh status
# ============================================================================
set -uo pipefail

PI_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="${PI_DIR}/architecture/PHASE_MANIFEST.toml"

if [ ! -f "$MANIFEST" ]; then
    echo "Error: Phase manifest not found at $MANIFEST"
    echo "Run 'guardian init' to scaffold architecture."
    exit 1
fi

# Extract module entries from a TOML section using sed
# Each module entry spans: [[phaseN.modules]] ... (until next [[ or [)
extract_modules() {
    local phase="$1"
    # Find the phase section, then extract all [[phaseN.modules]] blocks
    awk -v phase="$phase" '
        BEGIN { in_section = 0; in_module = 0 }
        # Detect phase section header
        /^\['"$phase"'\]/ { in_section = 1; next }
        # Another phase section ends this one
        /^\[phase[0-9]\]/ && !/^\['"$phase"'\]/ { in_section = 0 }
        # Non-phase section ends this one
        /^\[milestones\]/ { in_section = 0 }
        /^\[status\]/ { in_section = 0 }
        /^\[metadata\]/ { in_section = 0 }

        # Inside the phase section, detect module entries
        in_section && /^\[\['"$phase"'\.modules\]\]/ {
            if (id != "") print_entry()
            in_module = 1; id=""; name=""; mod=""; status=""; desc=""; deps=""
            next
        }

        in_module && /^id/    { id = extract_value($0) }
        in_module && /^name/  { name = extract_value($0) }
        in_module && /^module/ { mod = extract_value($0) }
        in_module && /^status/ { status = extract_value($0) }
        in_module && /^description/ { desc = extract_value($0) }

        function extract_value(s) {
            gsub(/^[^=]*= /, "", s)
            gsub(/^"/, "", s)
            gsub(/"$/, "", s)
            return s
        }
        function print_entry() {
            if (status == "implemented") mark = "✓"
            else mark = "○"
            printf "  [%2s] %-35s %-12s %s\n", id, name, "("status")", mod
        }
        END {
            if (id != "") print_entry()
        }
    ' "$MANIFEST"
}

find_module() {
    local module_name="$1"
    for phase in phase0 phase1 phase2 phase3 phase4; do
        local result
        result=$(awk -v phase="$phase" -v mod="$module_name" '
            BEGIN { in_section = 0; in_module = 0; found = 0 }
            /^\['"$phase"'\]/ { in_section = 1; next }
            /^\[phase[0-9]\]/ && !/^\['"$phase"'\]/ { in_section = 0 }
            /^\[milestones\]/ { in_section = 0 }
            /^\[status\]/ { in_section = 0 }
            /^\[metadata\]/ { in_section = 0 }

            in_section && /^\[\['"$phase"'\.modules\]\]/ {
                if (found) exit
                in_module = 1; id=""; name=""; mod=""; status=""; desc=""; deps=""
                next
            }
            in_module && /^id/ { id = extract_value($0) }
            in_module && /^name/ { name = extract_value($0) }
            in_module && /^module/ { mod = extract_value($0) }
            in_module && /^status/ { status = extract_value($0) }
            in_module && /^description/ { desc = extract_value($0) }
            in_module && /^depends_on/ {
                deps = $0
                gsub(/^[^=]*= /, "", deps)
                gsub(/^\[/, "", deps); gsub(/\]/, "", deps)
                gsub(/"/, "", deps)
            }
            in_module && /^\[\[/ && mod == mod {
                found = 1
                printf "Phase:   %s\n", phase
                printf "ID:      %s\n", id
                printf "Name:    %s\n", name
                printf "Status:  %s\n", status
                printf "Folder:  src/%s/\n", mod
                printf "Desc:    %s\n", desc
                printf "Depends: %s\n", deps
                exit
            }
            function extract_value(s) {
                gsub(/^[^=]*= /, "", s)
                gsub(/^"/, "", s)
                gsub(/"$/, "", s)
                return s
            }
            function print_entry() {
                if (status == "implemented") mark = "✓"
                else mark = "○"
                printf "  [%2s] %-35s %-12s %s\n", id, name, "("status")", mod
            }
            END { if (id != "" && !found) print_entry() }
        ' "$MANIFEST")
        if [ -n "$result" ]; then
            echo ""
            echo "$result"
            echo ""
            return 0
        fi
    done
    return 1
}

list_phase() {
    local phase="$1"
    case "$phase" in
        phase0|0) section="phase0" ;;
        phase1|1) section="phase1" ;;
        phase2|2) section="phase2" ;;
        phase3|3) section="phase3" ;;
        phase4|4) section="phase4" ;;
        *)
            echo "Unknown phase: $phase"
            echo "Valid: phase0, phase1, phase2, phase3, phase4"
            exit 1
            ;;
    esac

    local name
    name=$(grep "^name" <(sed -n "/^\[$section\]/,/^\[phase/p" "$MANIFEST") | head -1 | sed 's/name = "\(.*\)"/\1/')
    echo ""
    echo "═══ $section: $name ═══"
    echo ""
    extract_modules "$section"
    echo ""

    local gate
    gate=$(sed -n "/^\[$section\]/,/^\[/p" "$MANIFEST" | grep "^gate" | sed 's/gate = "\(.*\)"/\1/')
    if [ -n "$gate" ]; then
        echo "  Gate: $gate"
        echo ""
    fi
}

show_module() {
    local module_name="$1"
    if find_module "$module_name"; then
        return 0
    else
        echo "Module '$module_name' not found in manifest."
        echo "Run 'phase.sh list' to see all modules."
        exit 1
    fi
}

show_status() {
    echo ""
    echo "═══ Implementation Status ═══"
    echo ""

    for phase in phase0 phase1 phase2 phase3 phase4; do
        local name
        name=$(grep "^name" <(sed -n "/^\[$phase\]/,/^\[/p" "$MANIFEST") | head -1 | sed 's/name = "\(.*\)"/\1/')
        echo "  $phase ($name):"
        extract_modules "$phase"
        echo ""
    done

    local coverage
    coverage=$(grep "^coverage" "$MANIFEST" | sed 's/.*= "\(.*\)"/\1/')
    echo "  Coverage: $coverage"
    echo ""
}

show_help() {
    echo "Usage: bash .pi/scripts/phase.sh <command> [args]"
    echo ""
    echo "Commands:"
    echo "  list [phase]    List modules in a phase (phase0-phase4)"
    echo "  module <name>   Show which phase a module belongs to"
    echo "  status          Show implementation status across all phases"
    echo "  help            Show this help"
    echo ""
    echo "Examples:"
    echo "  bash .pi/scripts/phase.sh list phase0"
    echo "  bash .pi/scripts/phase.sh module cancellation"
    echo "  bash .pi/scripts/phase.sh status"
}

case "${1:-help}" in
    list)
        list_phase "${2:-}"
        ;;
    module)
        if [ -z "${2:-}" ]; then
            echo "Usage: bash .pi/scripts/phase.sh module <name>"
            exit 1
        fi
        show_module "$2"
        ;;
    status)
        show_status
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        echo "Unknown command: $1"
        show_help
        exit 1
        ;;
esac
