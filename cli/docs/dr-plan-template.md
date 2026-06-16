# Disaster Recovery Plan: Templates Module

> **Module:** `cli/src/templates/`
> **Version:** 0.1.0
> **Last Updated:** 2026-06-16
> **RTO:** < 1 minute (stateless module)
> **RPO:** N/A (no mutable state)

## Overview

The Templates module is stateless — it delegates all template storage and
management to the engine crate. There is no database, no persistent state,
and no long-lived connections. Recovery is therefore trivial: restart the CLI.

## Failure Scenarios

### Scenario 1: Module code corruption

**Symptom:** `TemplateCommandService::new()` fails, `CliError::Internal` on startup

**Impact:** `rigorix template list` and `rigorix template show` unavailable

**Recovery:**
1. Revert the offending change: `git revert <bad-commit>`
2. Rebuild: `cargo build`
3. Restart: Re-run the CLI

**Verification:** `rigorix template list` returns expected templates

### Scenario 2: Engine crate failure

**Symptom:** `CliError::Engine(...)` on all template operations

**Impact:** All template operations fail

**Recovery:**
1. Check engine crate status: `cd ../engine && cargo build`
2. Fix engine issue independently
3. Rebuild CLI with fixed engine: `cd ../cli && cargo build`

**Verification:** Engine unit tests pass: `cargo test -p rigorix_engine`

### Scenario 3: Config corruption

**Symptom:** Templates not found, wrong directories scanned

**Impact:** User templates are invisible

**Recovery:**
1. Check config: `cat rigorix.toml` — verify `template_dirs` setting
2. Restore default config: Remove or correct `template_dirs` in rigorix.toml
3. Verify template directory: `ls -la .rigorix/templates/`

**Verification:** Templates appear in `rigorix template list`

### Scenario 4: File system issue

**Symptom:** Cannot read template files

**Impact:** User templates unavailable (built-ins still work)

**Recovery:**
1. Check permissions: `ls -la .rigorix/templates/`
2. Restore from backup if template files are lost
3. Built-in templates (13 shipped templates) are always available

## Backup Strategy

| Asset | Backup Method | Frequency | Retention |
|-------|--------------|-----------|-----------|
| User template files (`*.toml`) | Git repository | Every commit | Full git history |
| Config (`rigorix.toml`) | Git repository | Every commit | Full git history |
| Built-in templates | Embedded in binary | N/A | N/A (compiled in) |

## Restore Procedure

### Restore user templates from git:

```bash
# Restore specific template file
git checkout <commit> -- .rigorix/templates/<template>.toml

# Restore all templates
git checkout <commit> -- .rigorix/templates/
```

### Restore config:

```bash
git checkout <commit> -- rigorix.toml
```

## Failover Plan

The Templates module has no failover mechanism needed — it is stateless and
ephemeral. If the CLI binary is corrupted:

1. Rebuild from source: `cargo build` in the `cli/` directory
2. Or download a pre-built binary from CI artifacts
3. Restart the CLI

## RTO/RPO Targets

| Metric | Target | Notes |
|--------|--------|-------|
| RTO (Recovery Time Objective) | < 1 minute | Stateless module — just restart CLI |
| RPO (Recovery Point Objective) | N/A | No mutable state to lose |

## Testing the DR Plan

Run quarterly:

```bash
# Verify built-in templates work (engine available)
rigorix template list

# Verify with custom config
rigorix --config test-config.toml template list

# Verify module loads even with empty template directory
mkdir -p /tmp/empty-templates
rigorix template list  # Should show built-ins
```
