#!/usr/bin/env bash
# Stage 1: Docs Policy
#
# Verifies:
# - MR traceability: Every changed file has a canonical reference to architecture
# - Docs sync guard: Architecture docs are updated when implementation changes

set -euo pipefail

PI_DIR=".pi"
ARCH_DIR="${PI_DIR}/architecture"

PASS=0
FAIL=0

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1 — $2"; FAIL=$((FAIL + 1)); }

echo "  Checking MR traceability..."

# Check 1: MR Traceability
# Every changed implementation file should have a canonical reference
if command -v git &>/dev/null; then
    changed_files=$(git diff --name-only HEAD~1 HEAD 2>/dev/null | grep -E '\.(py|ts|rs|go)$' || true)
    if [[ -n "$changed_files" ]]; then
        files_without_ref=0
        while IFS= read -r file; do
            if ! grep -q "Canonical Reference\|canonical.*reference" "$file" 2>/dev/null; then
                ((files_without_ref++))
                log_fail "MR traceability" "${file} missing canonical reference to architecture"
            fi
        done <<< "$changed_files"
        if [[ $files_without_ref -eq 0 ]]; then
            log_pass "MR traceability (all changed files have canonical references)"
        fi
    else
        log_pass "MR traceability (no implementation files changed)"
    fi
else
    log_fail "MR traceability" "git not available"
fi

echo "  Checking docs sync guard..."

# Check 2: Docs Sync Guard
# If implementation files changed in a module, the module doc should have been updated
if command -v git &>/dev/null; then
    module_dirs=$(ls -d "${ARCH_DIR}"/modules/ 2>/dev/null || true)
    if [[ -n "$module_dirs" ]]; then
        docs_updated=true
        for module_file in "${ARCH_DIR}"/modules/*.md; do
            [[ -f "$module_file" ]] || continue
            module_name=$(basename "$module_file" .md)
            # Check if files related to this module changed
            if git diff --name-only HEAD~1 HEAD 2>/dev/null | grep -qi "${module_name}" 2>/dev/null; then
                # Module files changed — check if the module doc was also touched
                if ! git diff --name-only HEAD~1 HEAD 2>/dev/null | grep -q "$(basename "$module_file")" 2>/dev/null; then
                    # Check if the module doc has a recent "Last Updated" that matches
                    if grep -q "Last Updated.*$(date +%Y-%m-%d)" "$module_file" 2>/dev/null; then
                        docs_updated=true
                    else
                        docs_updated=false
                        log_fail "Docs sync guard" "Files in ${module_name} changed but module doc not updated"
                    fi
                fi
            fi
        done
        if [[ "$docs_updated" == "true" ]]; then
            log_pass "Docs sync guard (architecture docs in sync with implementation)"
        fi
    else
        log_pass "Docs sync guard (no architecture modules found)"
    fi
fi

if [[ $FAIL -gt 0 ]]; then
    echo "  Docs policy FAILED (${FAIL} failure(s))"
    exit 1
fi

echo "  Docs policy passed (${PASS} check(s))"
exit 0
