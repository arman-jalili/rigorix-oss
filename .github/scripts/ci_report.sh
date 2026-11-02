#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Print the CI stage table, always, and never give a verdict (#1289).
#
# The Final Gate is skipped when a run was superseded, so the table cannot
# live only there: a reader still needs to see *which* stages reported
# `cancelled`. This job always runs and always succeeds.
# ─────────────────────────────────────────────────────────────────────

set -uo pipefail

here=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=ci_stages.sh
. "$here/ci_stages.sh"

ci_print_stage_table
ci_print_stage_table >> "${GITHUB_STEP_SUMMARY:-/dev/null}"

exit 0
