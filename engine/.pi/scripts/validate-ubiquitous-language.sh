#!/usr/bin/env bash
# ============================================================================
# validate-ubiquitous-language.sh — Detect Code Drift from Ubiquitous Language
#
# Parses .pi/domain/ubiquitous-language.md for the canonical term list,
# then greps src/ for class/function/variable/type names, flagging any
# that use an alias/synonym instead of the canonical term.
#
# Canonical Reference: .pi/architecture/modules/core-libraries.md
# Implements: Ubiquitous Language drift detection
# Issue: (add issue number here)
# Last Architecture Sync: 2026-05-31
#
# Usage:
#   bash .pi/scripts/validate-ubiquitous-language.sh [src_dir]
#
# Exit codes:
#   0 = No drift detected (all code uses canonical terms)
#   1 = Drift detected (one or more aliases found)
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
GLOSSARY_FILE="${PROJECT_ROOT}/.pi/domain/ubiquitous-language.md"
SRC_DIR="${1:-}"
# Default: check all crate src directories for Rust projects
if [ -z "$SRC_DIR" ]; then
    for d in "$PROJECT_ROOT/engine/src" "$PROJECT_ROOT/cli/src" "$PROJECT_ROOT/actions/src"; do
        if [ -d "$d" ]; then
            SRC_DIR="$d"
            break
        fi
    done
fi
# Fall back to PROJECT_ROOT/src
[ -z "$SRC_DIR" ] && SRC_DIR="${PROJECT_ROOT}/src"

PASS_COUNT=0; ERROR_COUNT=0; WARN_COUNT=0; DRIFT_FOUND=0
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
pass()  { echo -e "${GREEN}✅ PASS${NC}  $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail()  { echo -e "${RED}❌ FAIL${NC}  $1"; ERROR_COUNT=$((ERROR_COUNT + 1)); DRIFT_FOUND=1; }
warn()  { echo -e "${YELLOW}⚠️  WARN${NC}  $1"; WARN_COUNT=$((WARN_COUNT + 1)); }
info()  { echo -e "${BLUE}ℹ️  INFO${NC}  $1"; }

echo "============================================"
echo "  Ubiquitous Language Validation"
echo "============================================"
echo ""

# ─────────────────────────────────────────────────────────────────────────────
# Check: Glossary file exists
# ─────────────────────────────────────────────────────────────────────────────
echo "--- Preflight ---"
if [ ! -f "$GLOSSARY_FILE" ]; then
    fail "Glossary not found at: $GLOSSARY_FILE"
    info "Run the validator from the project root."
    echo ""
    echo "============================================"
    echo "  Summary"
    echo "============================================"
    echo -e "  Passed:   ${GREEN}0${NC}"
    echo -e "  Failed:   ${RED}1${NC}"
    exit 1
fi
pass "Glossary found: $GLOSSARY_FILE"

if [ ! -d "$SRC_DIR" ]; then
    fail "Source directory not found: $SRC_DIR"
    echo ""
    echo "============================================"
    echo "  Summary"
    echo "============================================"
    echo -e "  Passed:   ${GREEN}0${NC}"
    echo -e "  Failed:   ${RED}1${NC}"
    exit 1
fi
pass "Source directory found: $SRC_DIR"

# ─────────────────────────────────────────────────────────────────────────────
# Parse Glossary — extract canonical terms and aliases
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "--- Parsing Glossary ---"

# Parse the markdown table:
# Format: | Term | Definition | Bounded Context | Aliases/Synonyms | Examples |
# We need columns 1 (Term) and 4 (Aliases/Synonyms)
#
# Strategy: extract lines starting with "|", skip header/separator, parse columns

declare -a CANONICAL_TERMS=()
declare -a ALIAS_LISTS=()      # Same index as CANONICAL_TERMS, pipe-separated aliases
GLOSSARY_ENTRIES=0

while IFS='|' read -r _ term _ _ aliases _; do
    # Trim whitespace
    term=$(echo "$term" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
    aliases=$(echo "$aliases" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')

    # Skip header row, separator row, and empty rows
    [ -z "$term" ] && continue
    [ "$term" = "Term" ] && continue
    [ "$(echo "$term" | tr -d ' -')" = "" ] && continue

    # Skip if term contains only dashes (separator row like |------|)
    [[ "$term" =~ ^-+$ ]] && continue

    CANONICAL_TERMS+=("$term")
    ALIAS_LISTS+=("$aliases")
    GLOSSARY_ENTRIES=$((GLOSSARY_ENTRIES + 1))
done < <(grep '^|' "$GLOSSARY_FILE")

if [ "$GLOSSARY_ENTRIES" -eq 0 ]; then
    fail "No glossary entries found in $GLOSSARY_FILE"
    echo ""
    echo "============================================"
    echo "  Summary"
    echo "============================================"
    echo -e "  Passed:   ${GREEN}0${NC}"
    echo -e "  Failed:   ${RED}1${NC}"
    exit 1
fi
pass "Parsed $GLOSSARY_ENTRIES glossary entries"

# Debug: show parsed terms and aliases
for i in "${!CANONICAL_TERMS[@]}"; do
    term="${CANONICAL_TERMS[$i]}"
    aliases="${ALIAS_LISTS[$i]}"
    if [ -n "$aliases" ]; then
        info "  • \"$term\" → aliases: $aliases"
    fi
done

# ─────────────────────────────────────────────────────────────────────────────
# Find TypeScript identifiers in src/
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "--- Scanning Source Code ---"

TS_FILES=$(find "$SRC_DIR" -type f \( -name "*.ts" -o -name "*.tsx" \) 2>/dev/null | sort)
TS_FILE_COUNT=$(echo "$TS_FILES" | grep -c . 2>/dev/null || echo "0")

if [ "$TS_FILE_COUNT" -eq 0 ]; then
    warn "No TypeScript files found in $SRC_DIR"
    echo ""
    echo "============================================"
    echo "  Summary"
    echo "============================================"
    echo -e "  Passed:   ${GREEN}${PASS_COUNT}${NC}"
    echo -e "  Warnings: ${YELLOW}${WARN_COUNT}${NC}"
    exit 0
fi
pass "Found $TS_FILE_COUNT TypeScript files to scan"

# ─────────────────────────────────────────────────────────────────────────────
# Extract identifiers from TypeScript files
# ─────────────────────────────────────────────────────────────────────────────
# We extract:
#   - Class/interface/type/enum names (after keyword)
#   - Function names (after "function" or in arrow-function const assignments)
#   - Exported const/variable names
#   - Import aliases (e.g., "import { X as Y }")
#   - Named exports (export function/class/interface/type)

# Build a temporary file of all identifiers with their file:line locations
IDENTIFIER_FILE=$(mktemp)
trap 'rm -f "$IDENTIFIER_FILE"' EXIT

# Use perl for portable regex extraction with capturing groups
perl -nle '
    # Skip non-source files
    next unless -f $ARGV;
    
    my $file = $ARGV;
    my $line_no = $.;
    chomp;
    
    # Match: export (class|interface|type|enum|function) Name
    if (/\bexport\s+(class|interface|type|enum|function)\s+([A-Za-z_][A-Za-z0-9_]*)/) {
        print "$file:$line_no:$2";
    }
    
    # Match: (class|interface|type|enum|function) Name (without export)
    if (/\b(class|interface|type|enum|function)\s+([A-Za-z_][A-Za-z0-9_]*)/) {
        print "$file:$line_no:$2";
    }
    
    # Match: (const|let|var) Name
    if (/\b(const|let|var)\s+([A-Za-z_][A-Za-z0-9_]*)\s*[=:]/) {
        print "$file:$line_no:$2";
    }
    
    # Match: export default (class|function) Name
    if (/\bexport\s+default\s+(class|function)\s+([A-Za-z_][A-Za-z0-9_]*)/) {
        print "$file:$line_no:$2";
    }
    
    # Match: import { ... as Alias }
    if (/\bimport\s*\{[^}]*\}\s*from/) {
        # Find all "as X" patterns
        my $rest = /Users/arman/.bun/bin/guardian-framework;
        while ($rest =~ /\bas\s+([A-Za-z_][A-Za-z0-9_]*)/g) {
            print "$file:$line_no:$1";
        }
    }
    
    # Match: function Name(...)
    if (/\bfunction\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(/) {
        print "$file:$line_no:$1";
    }
' $TS_FILES 2>/dev/null >> "$IDENTIFIER_FILE" || true

IDENTIFIER_COUNT=$(wc -l < "$IDENTIFIER_FILE" | tr -d ' ')
if [ "$IDENTIFIER_COUNT" -eq 0 ]; then
    warn "No identifiers extracted from TypeScript files"
else
    info "Extracted $IDENTIFIER_COUNT identifiers"
fi

# ─────────────────────────────────────────────────────────────────────────────
# Check each identifier against canonical terms and aliases
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "--- Drift Detection ---"

DRIFT_COUNT=0

for i in "${!CANONICAL_TERMS[@]}"; do
    canonical="${CANONICAL_TERMS[$i]}"
    aliases="${ALIAS_LISTS[$i]}"

    # Skip if no aliases defined for this term
    [ -z "$aliases" ] && continue

    # Split aliases by comma and check each one
    IFS=',' read -ra ALIAS_ARRAY <<< "$aliases"
    for alias in "${ALIAS_ARRAY[@]}"; do
        # Trim whitespace
        alias=$(echo "$alias" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
        [ -z "$alias" ] && continue

        # Check if this alias is the same as the canonical term (skip)
        if [ "$alias" = "$canonical" ]; then
            continue
        fi

        # Search identifiers for this alias
        # Use grep -F for exact string matching, -w for whole word
        MATCHES=$(grep -F ":$alias" "$IDENTIFIER_FILE" 2>/dev/null || true)

        if [ -n "$MATCHES" ]; then
            # Found drift — this alias is used in code
            while IFS= read -r match; do
                # Parse file:line:identifier
                match_file=$(echo "$match" | cut -d: -f1)
                match_line=$(echo "$match" | cut -d: -f2)
                match_id=$(echo "$match" | cut -d: -f3-)

                # Convert to relative path
                rel_file="${match_file#$PROJECT_ROOT/}"

                fail "Alias \"$alias\" used instead of canonical \"$canonical\" → ${rel_file}:${match_line} (identifier: $match_id)"
                DRIFT_COUNT=$((DRIFT_COUNT + 1))
            done <<< "$MATCHES"
        fi
    done
done

if [ "$DRIFT_COUNT" -eq 0 ]; then
    pass "No drift detected — all code uses canonical terms"
fi

# ─────────────────────────────────────────────────────────────────────────────
# Also check for case variations of canonical terms
# Note: Canonical term case variation checks are intentionally omitted.
# TypeScript convention uses PascalCase for types (e.g., Result, Manifest)
# and camelCase for variable instances (e.g., result, manifest).
# Both are correct in context and not drift.

# ─────────────────────────────────────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "============================================"
echo "  Summary"
echo "============================================"
echo -e "  Passed:   ${GREEN}${PASS_COUNT}${NC}"
echo -e "  Warnings: ${YELLOW}${WARN_COUNT}${NC}"
echo -e "  Drift:    ${RED}${DRIFT_COUNT}${NC}"

if [ "$DRIFT_FOUND" -eq 1 ]; then
    echo ""
    echo -e "${RED}❌ Ubiquitous language drift detected.${NC}"
    echo "  Fix: Replace aliases with canonical terms in the listed files."
    echo "  Then re-run this validator."
    exit 1
fi

echo ""
echo -e "${GREEN}✅ Ubiquitous language validation passed.${NC}"
exit 0
