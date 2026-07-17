#!/usr/bin/env bash
# Local CI — runs ALL validation and proofing scripts locally
#
# Scans .pi/scripts/ci/ for stage_*.sh, check_*.sh, and validate-*.sh
# scripts and runs every one. This includes all epic proofing scripts.
# Use before committing to catch regressions across all modules.
#
# Usage:
#   bash .pi/scripts/ci/local-ci.sh              # Run all
#   bash .pi/scripts/ci/local-ci.sh --list       # List matching scripts only
#   bash .pi/scripts/ci/local-ci.sh --verbose    # Verbose output
#   bash .pi/scripts/ci/local-ci.sh --json       # JSON report
#
# Registration: add new scripts by placing them in .pi/scripts/ci/
# with names matching: stage_*.sh, check_*.sh, or validate-*.sh

set -euo pipefail

PI_DIR=".pi"
CI_DIR="${PI_DIR}/scripts/ci"
START_DIR="$(cd "$(dirname "$0")/../../.." && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Options
LIST_ONLY=false
VERBOSE=false
JSON=false
TOTAL=0
PASSED=0
FAILED=0
RESULTS=()
START_TIME=$(date +%s)

while [[ $# -gt 0 ]]; do
	case $1 in
		--list) LIST_ONLY=true; shift ;;
		--verbose) VERBOSE=true; shift ;;
		--json) JSON=true; shift ;;
		*) shift ;;
	esac
done

cd "$START_DIR"

# Discover all scripts via regex: stage_*.sh, check_*.sh, validate-*.sh
SCRIPTS=()
while IFS= read -r -d '' script; do
	SCRIPTS+=("$script")
done < <(find "$CI_DIR" -maxdepth 1 -type f -name 'stage_*.sh' -print0 2>/dev/null)
while IFS= read -r -d '' script; do
	SCRIPTS+=("$script")
done < <(find "$CI_DIR" -maxdepth 1 -type f -name 'check_*.sh' -print0 2>/dev/null)
while IFS= read -r -d '' script; do
	SCRIPTS+=("$script")
done < <(find "$CI_DIR" -maxdepth 1 -type f -name 'validate-*.sh' -print0 2>/dev/null)

if [[ ${#SCRIPTS[@]} -eq 0 ]]; then
	echo "No matching scripts found in $CI_DIR"
	exit 0
fi

# Sort for deterministic order
IFS=$'\n' SCRIPTS=($(sort <<<"${SCRIPTS[*]}")); unset IFS

if [[ "$LIST_ONLY" == "true" ]]; then
	echo "Scripts in $CI_DIR:"
	for script in "${SCRIPTS[@]}"; do
		echo "  $script"
	done
	exit 0
fi

if [[ "$JSON" == "false" ]]; then
	echo -e "${BLUE}══════════════════════════════════════════════${NC}"
	echo -e "${BLUE}  Local CI — Running ${#SCRIPTS[@]} scripts${NC}"
	echo -e "${BLUE}══════════════════════════════════════════════${NC}"
	echo ""
fi

for script in "${SCRIPTS[@]}"; do
	name=$(basename "$script")
	TOTAL=$((TOTAL + 1))

	if [[ "$JSON" == "false" ]]; then
		echo -e "${YELLOW}[RUN]${NC} $name"
	fi

	set +e
	output=$(bash "$script" 2>&1)
	exit_code=$?
	set -e

	if [[ $exit_code -eq 0 ]]; then
		PASSED=$((PASSED + 1))
		RESULTS+=("{\"script\":\"$name\",\"status\":\"pass\"}")
		if [[ "$JSON" == "false" ]]; then
			echo -e "  ${GREEN}✓ PASS${NC}"
		fi
	else
		FAILED=$((FAILED + 1))
		RESULTS+=("{\"script\":\"$name\",\"status\":\"fail\",\"exit_code\":$exit_code}")
		if [[ "$JSON" == "false" ]]; then
			echo -e "  ${RED}✗ FAIL (exit $exit_code)${NC}"
			if [[ "$VERBOSE" == "true" ]]; then
				echo "$output" | sed 's/^/    /'
			fi
		fi
	fi
done

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

if [[ "$JSON" == "true" ]]; then
	echo "{"
	echo "  \"total\": $TOTAL,"
	echo "  \"passed\": $PASSED,"
	echo "  \"failed\": $FAILED,"
	echo "  \"duration_seconds\": $DURATION,"
	echo "  \"results\": ["
	for i in "${!RESULTS[@]}"; do
		sep=","
		if [[ $i -eq $((${#RESULTS[@]} - 1)) ]]; then sep=""; fi
		echo "    ${RESULTS[$i]}$sep"
	done
	echo "  ]"
	echo "}"
else
	echo ""
	echo -e "${BLUE}══════════════════════════════════════════════${NC}"
	echo -e "  Total: $TOTAL  |  ${GREEN}Passed: $PASSED${NC}  |  ${RED}Failed: $FAILED${NC}  |  ${DURATION}s"
	echo -e "${BLUE}══════════════════════════════════════════════${NC}"
fi

if [[ $FAILED -gt 0 ]]; then
	exit 1
fi
exit 0
