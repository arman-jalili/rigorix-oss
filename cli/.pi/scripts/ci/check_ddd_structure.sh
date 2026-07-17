#!/usr/bin/env bash
# ============================================================================
# check_ddd_structure.sh — Validate DDD Clean Architecture layer structure
#
# Ensures every module in src/ follows the correct 4-layer DDD structure:
#   src/[module]/domain/
#   src/[module]/application/
#   src/[module]/infrastructure/
#   src/[module]/interfaces/
#
# Also checks NO contracts/ wrapper exists (v0.1.10+ pattern) and validates
# dependency direction (domain must not import from outer layers).
#
# Usage:
#   bash .pi/scripts/ci/check_ddd_structure.sh              # Run all checks
#   bash .pi/scripts/ci/check_ddd_structure.sh --module X   # Single module
#   bash .pi/scripts/ci/check_ddd_structure.sh --fix        # Suggest fixes
#
# Exit: 0 = all modules conform, 1 = violations found
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
SRC_DIR="${PROJECT_ROOT}/src"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

PASS_COUNT=0
FAIL_COUNT=0
ERRORS=()
CHECK_MODE="full"
SINGLE_MODULE=""

# ── Options ──
while [[ $# -gt 0 ]]; do
    case $1 in
        --module) SINGLE_MODULE="$2"; shift 2 ;;
        --fix) CHECK_MODE="fix"; shift ;;
        --help)
            sed -n '3,14p' "$0" | sed 's/^#//'
            exit 0
            ;;
        *) shift ;;
    esac
done

pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; FAIL_COUNT=$((FAIL_COUNT + 1)); ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; }

echo "============================================"
echo "  DDD Layer Structure Check"
echo "============================================"
echo ""

# ── Language Detection ──
detect_language() {
    if [[ -f "${PROJECT_ROOT}/Cargo.toml" ]]; then echo "rust"
    elif [[ -f "${PROJECT_ROOT}/go.mod" ]]; then echo "go"
    elif [[ -f "${PROJECT_ROOT}/package.json" || -f "${PROJECT_ROOT}/tsconfig.json" ]]; then echo "typescript"
    elif [[ -f "${PROJECT_ROOT}/pyproject.toml" || -f "${PROJECT_ROOT}/requirements.txt" ]]; then echo "python"
    elif [[ -f "${PROJECT_ROOT}/pom.xml" || -f "${PROJECT_ROOT}/build.gradle" ]]; then echo "java"
    else echo "unknown"; fi
}

PROJECT_LANG=$(detect_language)
echo "Language: ${BLUE}${PROJECT_LANG}${NC}"
echo ""

# ── File extension mapping ──
get_source_extensions() {
    case "$1" in
        rust) echo "rs" ;;
        go) echo "go" ;;
        typescript) echo "ts" ;;
        python) echo "py" ;;
        java) echo "java" ;;
        nextjs) echo "ts tsx" ;;
        angular) echo "ts" ;;
        *) echo "rs go ts py java tsx" ;;
    esac
}

get_import_pattern() {
    # Returns regex patterns for import statements that violate dependency direction
    case "$1" in
        rust)
            # Domain importing from infrastructure/application/interfaces is wrong
            echo "use crate::.*infrastructure|use crate::.*application|use crate::.*interfaces"
            ;;
        go)
            echo '"\.\./infrastructure|"\.\./application|"\.\./interfaces'
            ;;
        typescript)
            echo "from.*infrastructure|from.*application|from.*interfaces"
            ;;
        python)
            echo "from.*infrastructure|from.*application|from.*interfaces|import.*infrastructure|import.*application|import.*interfaces"
            ;;
        java)
            echo "import.*infrastructure|import.*application|import.*interfaces"
            ;;
        *)
            echo "infrastructure|application|interfaces"
            ;;
    esac
}

SRC_EXTS=$(get_source_extensions "$PROJECT_LANG")
IMPORT_PATTERN=$(get_import_pattern "$PROJECT_LANG")

# ── Discover modules ──
declare -a MODULES
if [[ -n "$SINGLE_MODULE" ]]; then
    if [[ -d "${SRC_DIR}/${SINGLE_MODULE}" ]]; then
        MODULES=("${SINGLE_MODULE}")
    else
        echo -e "${RED}Module '${SINGLE_MODULE}' not found in src/${NC}"
        exit 1
    fi
else
    # Find top-level directories in src/ — these are modules/bounded contexts
    while IFS= read -r -d '' dir; do
        module=$(basename "$dir")
        # Skip shared/ common/ config/ if they exist
        case "$module" in
            shared|common|config|lib) continue ;;
        esac
        MODULES+=("$module")
    done < <(find "$SRC_DIR" -mindepth 1 -maxdepth 1 -type d -print0 2>/dev/null | sort -z)
fi

if [[ ${#MODULES[@]} -eq 0 ]]; then
    warn "No modules found in src/ — nothing to check"
    echo ""
    echo "============================================"
    echo "  Summary"
    echo "============================================"
    echo -e "  Passed:   ${GREEN}${PASS_COUNT}${NC}"
    echo -e "  Failed:   ${RED}${FAIL_COUNT}${NC}"
    echo ""
    echo -e "${YELLOW}No modules to validate.${NC}"
    exit 0
fi

echo "Found ${#MODULES[@]} module(s): ${MODULES[*]}"
echo ""

# ── Required DDD layers ──
DDD_LAYERS=("domain" "application" "infrastructure" "interfaces")

# ── Check 1: Every module has the 4 DDD layers ──
echo "--- Layer Structure ---"

for module in "${MODULES[@]}"; do
    module_dir="${SRC_DIR}/${module}"
    missing=()
    contracts_wrapper=false

    # Check for contracts/ anti-pattern
    if [[ -d "${module_dir}/contracts" ]]; then
        contracts_wrapper=true
    fi

    for layer in "${DDD_LAYERS[@]}"; do
        if [[ ! -d "${module_dir}/${layer}" ]]; then
            missing+=("$layer")
        fi
    done

    if [[ ${#missing[@]} -eq 0 ]] && [[ "$contracts_wrapper" == false ]]; then
        pass "${module} — all 4 DDD layers present"
    elif [[ ${#missing[@]} -eq 0 ]] && [[ "$contracts_wrapper" == true ]]; then
        fail "${module} — has contracts/ wrapper (should be domain/ application/ infrastructure/ interfaces/)"
    else
        fail "${module} — missing layers: ${missing[*]}"
    fi
done

echo ""

# ── Check 2: No contracts/ wrapper ──
echo "--- Anti-Pattern: contracts/ Wrapper ---"
CONTRACTS_COUNT=0
for module in "${MODULES[@]}"; do
    if [[ -d "${SRC_DIR}/${module}/contracts" ]]; then
        CONTRACTS_COUNT=$((CONTRACTS_COUNT + 1))

        # In --fix mode, suggest the rename
        if [[ "$CHECK_MODE" == "fix" ]]; then
            echo -e "${YELLOW}   Would fix: git mv ${SRC_DIR}/${module}/contracts/* ${SRC_DIR}/${module}/ && rmdir ${SRC_DIR}/${module}/contracts${NC}"
        fi
    fi
done

if [[ $CONTRACTS_COUNT -eq 0 ]]; then
    pass "No contracts/ wrapper directories found"
else
    fail "Found ${CONTRACTS_COUNT} module(s) with contracts/ wrapper — should use direct DDD layers"
fi

echo ""

# ── Check 3: Dependency direction (domain must not import outer layers) ──
echo "--- Dependency Direction ---"
VIOLATIONS=0

for module in "${MODULES[@]}"; do
    domain_dir="${SRC_DIR}/${module}/domain"
    if [[ ! -d "$domain_dir" ]]; then
        continue
    fi

    # Find source files in domain/ and check for illegal imports
    while IFS= read -r -d '' file; do
        if grep -Eq "$IMPORT_PATTERN" "$file" 2>/dev/null; then
            violations=$(grep -En "$IMPORT_PATTERN" "$file" 2>/dev/null || true)
            if [[ -n "$violations" ]]; then
                echo -e "${RED}   Illegal import in ${file}${NC}"
                while IFS= read -r line; do
                    echo "     ${line}"
                done <<< "$violations"
                VIOLATIONS=$((VIOLATIONS + 1))
            fi
        fi
    done < <(find "$domain_dir" -type f \( -name "*.${SRC_EXTS%% *}" $(for ext in ${SRC_EXTS#* }; do echo -o -name "*.$ext"; done) \) -print0 2>/dev/null)
done

if [[ $VIOLATIONS -eq 0 ]]; then
    pass "No dependency direction violations (domain does not import outer layers)"
else
    fail "Found ${VIOLATIONS} dependency direction violation(s) — domain/ must not import from outer layers"
fi

echo ""

# ── Check 4: Repository interfaces defined in domain, implemented in infrastructure ──
echo "--- Repository Pattern ---"
REPO_INFRA_ONLY=0
REPO_MISSING_INFRA=0

for module in "${MODULES[@]}"; do
    infra_repo_dir="${SRC_DIR}/${module}/infrastructure/repository"
    domain_repo_file=""
    
    # Check for repository file in domain/
    domain_repo=$(find "${SRC_DIR}/${module}/domain" -maxdepth 2 -name "*repository*" -o -name "*repo*" 2>/dev/null | head -1)
    
    # Check for repository implementation in infrastructure/
    infra_repo=$(find "${SRC_DIR}/${module}/infrastructure" -maxdepth 3 -name "*repository*" -o -name "*repo*" 2>/dev/null | head -1)
    
    if [[ -n "$domain_repo" ]] && [[ -z "$infra_repo" ]]; then
        warn "${module} — repository interface in domain/ but no implementation in infrastructure/"
        REPO_MISSING_INFRA=$((REPO_MISSING_INFRA + 1))
    fi
done

if [[ $REPO_MISSING_INFRA -eq 0 ]]; then
    pass "Repository interfaces have implementations in infrastructure/ (where applicable)"
else
    warn "${REPO_MISSING_INFRA} module(s) with unreferenced repository interfaces"
fi

echo ""

# ── Summary ──
echo "============================================"
echo "  Summary"
echo "============================================"
echo -e "  Language:       ${BLUE}${PROJECT_LANG}${NC}"
echo -e "  Modules:        ${BLUE}${#MODULES[@]}${NC}"
echo -e "  Passed:         ${GREEN}${PASS_COUNT}${NC}"
echo -e "  Failed:         ${RED}${FAIL_COUNT}${NC}"
echo ""

if [[ ${#ERRORS[@]} -gt 0 ]]; then
    echo "FAILURES:"
    for err in "${ERRORS[@]}"; do
        echo "  - $err"
    done
    echo ""
    echo -e "${RED}DDD structure violations found. Fix before merging.${NC}"
    exit 1
fi

echo -e "${GREEN}All modules conform to DDD Clean Architecture layer structure.${NC}"
exit 0
