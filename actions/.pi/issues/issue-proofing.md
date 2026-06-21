---
guardian_issue:
  id: "ISSUE-PROOFING"
  epic: ""action-entrypoint""
  component: "Proofing & CI Enforcement"
  module: "action-entrypoint"
  status: planned
  priority: critical
  dependencies: []

  in_scope:
    - Create deterministic validation scripts for each contract
    - Verify all interfaces have matching implementations
    - Check test coverage meets thresholds
    - Integrate proofing scripts into .pi/scripts/ci/
    - Scripts must be self-contained shell scripts (zero token cost)

  out_of_scope:
    - Implementation changes
    - New features
    - Production deployment

  affected_layers:
    ci:
      - New proofing scripts in .pi/scripts/ci/
      - Updated CI stage configuration

  canonical_references:
    - module: ".pi/architecture/modules/action-entrypoint.md"

  acceptance_criteria:
    - "All proofing scripts created and executable"
    - "Each contract has at least one validation check"
    - "Scripts pass on current implementation"
    - "Scripts fail if implementation is removed"
    - "Scripts integrated into CI pipeline (stage in run_hardening_stages.sh)"

  validators:
    - ci
    - tests
    - canonical

  implementation_notes: |
    Create deterministic shell scripts that validate: each defined interface has an
    implementation, each implementation has tests, test coverage meets threshold,
    contracts are not violated. These escape the LLM ad-hoc check trap — they run
    every build for zero token cost.

  file_changes:
    - "create: .pi/scripts/ci/check_action-entrypoint_contracts.sh"
    - "create: .pi/scripts/ci/check_action-entrypoint_coverage.sh"
    - "modify: .pi/scripts/ci/run_hardening_stages.sh"
---

# Proofing & CI Enforcement: action-entrypoint

## Intent

Create deterministic, automated validation scripts that prove every contract from the
freeze phase is correctly implemented and tested. These scripts make compliance
automatic — no human review needed for routine checks.

## What Each Script Does

### Contract Implementation Check
- Reads each interface from the contract freeze
- Verifies a concrete implementation class exists
- Verifies all interface methods are implemented
- Reports violations with file:line references

### Coverage Threshold Check
- Runs the project's coverage tool
- Asserts each module meets minimum coverage (default 80%)
- Fails the build if coverage drops

### CI Integration
Each check becomes a CI stage in the hardening pipeline — it runs automatically
on every PR. No LLM cost. No human review. Just pass or fail.

## Scripts To Create

| Script | Purpose | Location |
|--------|---------|----------|
| check_action-entrypoint_contracts.sh | Validate contract implementation | .pi/scripts/ci/ |
| check_action-entrypoint_coverage.sh | Enforce coverage thresholds | .pi/scripts/ci/ |
| stage_action-entrypoint_proofing.sh | CI stage wrapper | .pi/scripts/ci/ |

## CI Pipeline Update

Add the new stage to `run_hardening_stages.sh`:

```bash
run_stage "11" "action-entrypoint_proofing" \
    "${SCRIPTS_DIR}/stage_action-entrypoint_proofing.sh" \
    "always"
```

## Acceptance Criteria

| # | Criterion | Script |
|---|-----------|--------|
| 1 | All interfaces have implementations | check_contracts.sh |
| 2 | Coverage ≥ 80% per module | check_coverage.sh |
| 3 | CI runs checks on every PR | run_hardening_stages.sh |
| 4 | All scripts exit 0 on pass, 1 on fail | self-validating |

## Implementation

> **Agent:** Create shell scripts. Keep them simple — grep, find, awk.
> No frameworks, no dependencies. Each script should be:
> 1. Runnable standalone (bash script.sh)
> 2. Runnable as a CI stage
> 3. Self-documenting with --help
> 4. Exit 0 for pass, 1 for fail
>
> End by running the full CI pipeline to verify integration:
> `bash .pi/scripts/ci/run_hardening_stages.sh`
