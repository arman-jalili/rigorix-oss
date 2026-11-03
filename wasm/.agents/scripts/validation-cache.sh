#!/usr/bin/env bash
# ============================================================================
# validation-cache.sh — Validation Result Caching System
#
# Caches passed validation checks so retries only re-check failed items.
#
# Usage:
#   bash .pi/scripts/validation-cache.sh init [task-id]
#   bash .pi/scripts/validation-cache.sh record [task-id] [validator] [check] [pass|fail]
#   bash .pi/scripts/validation-cache.sh get-failed [task-id] [validator]
#   bash .pi/scripts/validation-cache.sh summary [task-id]
#   bash .pi/scripts/validation-cache.sh clear [task-id]
#
# Exit codes: 0 = success, 1 = error
# ============================================================================
set -euo pipefail

CACHE_DIR=".claude/validation-cache"
mkdir -p "$CACHE_DIR"

ACTION="${1:-help}"
TASK_ID="${2:-}"
VALIDATOR="${3:-}"
CHECK="${4:-}"
STATUS="${5:-}"

CACHE_FILE() { echo "${CACHE_DIR}/${TASK_ID}.cache"; }

# ---------------------------------------------------------------------------
# Init — create empty cache for a task
# ---------------------------------------------------------------------------
init_cache() {
    if [ -z "$TASK_ID" ]; then
        echo "Usage: $0 init <task-id>"
        exit 1
    fi
    echo "# Validation Cache: $TASK_ID" > "$(CACHE_FILE)"
    echo "# Format: validator|check|status|timestamp" >> "$(CACHE_FILE)"
    echo "Initialized cache for task: $TASK_ID"
}

# ---------------------------------------------------------------------------
# Record — log a validation result
# ---------------------------------------------------------------------------
record_result() {
    if [ -z "$TASK_ID" ] || [ -z "$VALIDATOR" ] || [ -z "$CHECK" ] || [ -z "$STATUS" ]; then
        echo "Usage: $0 record <task-id> <validator> <check> <pass|fail>"
        exit 1
    fi
    TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    echo "${VALIDATOR}|${CHECK}|${STATUS}|${TIMESTAMP}" >> "$(CACHE_FILE)"
}

# ---------------------------------------------------------------------------
# Get Failed — list only failed checks for a validator (for retry)
# ---------------------------------------------------------------------------
get_failed() {
    if [ -z "$TASK_ID" ] || [ -z "$VALIDATOR" ]; then
        echo "Usage: $0 get-failed <task-id> <validator>"
        exit 1
    fi
    if [ ! -f "$(CACHE_FILE)" ]; then
        echo "No cache found for task: $TASK_ID"
        exit 1
    fi
    FAILED=$(grep "^${VALIDATOR}|" "$(CACHE_FILE)" | grep "|fail|" | cut -d'|' -f2 || true)
    if [ -z "$FAILED" ]; then
        echo "ALL_PASSED"
    else
        echo "$FAILED"
    fi
}

# ---------------------------------------------------------------------------
# Summary — show full validation status for a task
# ---------------------------------------------------------------------------
show_summary() {
    if [ -z "$TASK_ID" ]; then
        echo "Usage: $0 summary <task-id>"
        exit 1
    fi
    if [ ! -f "$(CACHE_FILE)" ]; then
        echo "No cache found for task: $TASK_ID"
        exit 1
    fi

    TOTAL=$(grep -v "^#" "$(CACHE_FILE)" | wc -l | tr -d ' ')
    PASSED=$(grep -v "^#" "$(CACHE_FILE)" | grep "|pass|" | wc -l | tr -d ' ')
    FAILED=$(grep -v "^#" "$(CACHE_FILE)" | grep "|fail|" | wc -l | tr -d ' ')

    echo "============================================"
    echo "  Validation Summary: $TASK_ID"
    echo "============================================"
    echo "  Total:  $TOTAL"
    echo "  Passed: $PASSED"
    echo "  Failed: $FAILED"
    echo ""

    if [ "$FAILED" -gt 0 ]; then
        echo "Failed checks:"
        grep -v "^#" "$(CACHE_FILE)" | grep "|fail|" | while IFS='|' read -r val chk sts ts; do
            echo "  [$val] $chk"
        done
    fi
}

# ---------------------------------------------------------------------------
# Clear — remove cache for a task
# ---------------------------------------------------------------------------
clear_cache() {
    if [ -z "$TASK_ID" ]; then
        echo "Usage: $0 clear <task-id>"
        exit 1
    fi
    rm -f "$(CACHE_FILE)"
    echo "Cleared cache for task: $TASK_ID"
}

# ---------------------------------------------------------------------------
# Dispatch
# ---------------------------------------------------------------------------
case "$ACTION" in
    init)        init_cache ;;
    record)      record_result ;;
    get-failed)  get_failed ;;
    summary)     show_summary ;;
    clear)       clear_cache ;;
    *)
        echo "Usage: $0 {init|record|get-failed|summary|clear} [args...]"
        echo ""
        echo "Commands:"
        echo "  init <task-id>                              Create empty cache"
        echo "  record <task-id> <validator> <check> <status>  Record result"
        echo "  get-failed <task-id> <validator>            List failed checks"
        echo "  summary <task-id>                           Show full status"
        echo "  clear <task-id>                             Remove cache"
        exit 1
        ;;
esac
