# Disaster Recovery Plan: quality-gates Module

<!--
Canonical Reference: .pi/architecture/modules/quality-gates.md
Last Updated: 2026-06-19
-->

## Overview

The quality-gates module is a **stateless, configuration-driven** system that
evaluates test scope quality levels against contracts. Configuration is ephemeral
in the in-memory implementation.

## Recovery Objectives

| Metric | Target |
|--------|--------|
| RTO | < 5 minutes |
| RPO | 0 (no persistent state) |

## Failure Scenarios

### F1: Configuration corruption
**Symptoms:** Wrong default level or invalid template overrides.

**Recovery:** Recreate config via `QualityGateConfig::new(level)` and re-register overrides.

### F2: Service evaluation failure
**Symptoms:** `QualityGateService::evaluate_gate()` returns errors.

**Recovery:** Verify the `GreenContract` and `QualityLevel` are valid. Recreate service.

## Backup and Restore

| Data | Backup Strategy |
|------|----------------|
| Configuration | Source control (`.rigorix/quality.toml`) |

## Testing

- CI Stage 30 validates all contracts and coverage
- Run `check_quality-gates_contracts.sh` to verify implementations
