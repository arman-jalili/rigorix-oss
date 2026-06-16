# Disaster Recovery Plan: Configuration Module

> **Module:** `cli/src/configuration/`
> **Version:** 0.1.0
> **Last Updated:** 2026-06-16
> **RTO:** < 1 minute (stateless config loader)
> **RPO:** N/A (no mutable state)

## Overview

The Configuration module is stateless — it reads configuration from files, env vars,
and CLI flags at startup and produces an immutable `CliConfig`. There is no database,
no persistent state, and no long-lived connections. Recovery is trivial: restart the CLI.

## Failure Scenarios

### Scenario 1: Config file corrupted or missing

**Symptom:** `CliError::ConfigNotFound` or `CliError::ConfigParseError` on startup

**Impact:** CLI cannot start

**Recovery:**
1. Run `rigorix init` to create a fresh `.rigorix/` directory with default config
2. Or restore from git: `git checkout <commit> -- rigorix.toml`
3. Or use environment variables only (no config file needed)

**Verification:** CLI starts successfully

### Scenario 2: Module code corruption

**Symptom:** Compilation error in configuration module

**Impact:** All CLI commands unavailable

**Recovery:**
1. Revert: `git revert <bad-commit>`
2. Rebuild: `cargo build`
3. Restart CLI

**Verification:** `cargo build` succeeds, CLI starts

### Scenario 3: API key missing

**Symptom:** `CliError::MissingConfig` on `run`/`plan`/`generate` commands

**Impact:** LLM-powered commands unavailable

**Recovery:**
1. Set env var: `export RIGORIX_API_KEY=sk-...`
2. Or add to config file: `echo 'api_key = "sk-..."' >> rigorix.toml`
3. Or run `rigorix init --interactive` and follow prompts

**Verification:** `rigorix plan "test"` succeeds

## Backup Strategy

| Asset | Backup Method | Frequency | Retention |
|-------|--------------|-----------|-----------|
| Config file (rigorix.toml) | Git repository | Every commit | Full git history |
| .rigorix/ directory | Git repository | Every commit | Full git history |

## Restore Procedure

```bash
# Restore config file
git checkout <commit> -- rigorix.toml

# Restore entire .rigorix/ directory
git checkout <commit> -- .rigorix/

# Rebuild after restoring
cd cli && cargo build
```

## RTO/RPO Targets

| Metric | Target | Notes |
|--------|--------|-------|
| RTO | < 1 minute | Stateless — just restart CLI |
| RPO | N/A | No mutable state to lose |

## Testing the DR Plan

```bash
# Verify config loads without file
RIGORIX_API_KEY=test rigorix --help

# Verify config loads with explicit path
rigorix --config /dev/null --help

# Verify config module tests pass
cargo test -p rigorix-cli -- configuration
```
