# Configuration

## Module Status

**Status:** Implemented — CLI integration over engine contracts
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

Multi-source configuration loading with layered merging:
1. Built-in defaults
2. `rigorix.toml` (project config file in working directory)
3. Environment variables (prefix: `RIGORIX_`)
4. CLI flags (highest precedence)

Provides validated `Config` to all other contexts. Includes `Secret` type for safe API key handling (redacted Debug/Display, excluded from logs and TUI output).

## Components

**CLI-facing:**
| Component | File | Purpose |
|-----------|------|---------|
| CliConfig | `cli/src/configuration/domain/config.rs` | CLI-specific configuration value object (output format, color, TUI, log settings) |
| CliConfigLoader (trait) | `cli/src/configuration/infrastructure/config.rs` | Config loading interface — load, load_from_path, has_default_config |
| CliConfigLoaderImpl | `cli/src/configuration/infrastructure/config_impl.rs` | Multi-source merging: CLI flags → env vars → rigorix.toml → engine defaults |
| validate_api_key_for_command | `cli/src/configuration/infrastructure/config_impl.rs` | Pre-flight validation: reports clear error for missing API key on run/plan/generate |
| build_engine_cli_overrides | `cli/src/configuration/infrastructure/config_impl.rs` | Bridges CLI config to engine's ConfigService as dot-notation overrides |

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| Config (aggregate root) | `engine/src/configuration/domain/config.rs` | `# Contract (Frozen)` |
| Secret | `engine/src/configuration/domain/secret.rs` | Redacted Debug/Display wrapper |
| ConfigSource | `engine/src/configuration/domain/config.rs` | Source enum: Default, File, Environment, Programmatic |
| EnforcementPreset | `engine/src/configuration/domain/config.rs` | Preset profiles for enforcement limits |
| ConfigurationError | `engine/src/configuration/domain/error.rs` | Typed error enum |
| ConfigService (trait) | `engine/src/configuration/application/service.rs` | ConfigService, SecretService traits |

## Domain Events

| Event | Description | Triggered By |
|-------|-------------|-------------|
| ConfigurationChanged | Configuration was loaded or reloaded | ConfigService |
| ConfigValidationFailed | Configuration validation errors detected | ConfigValidator |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| Config | Multi-source merged configuration aggregate root with all sub-configs. |
| Secret | API key wrapper with redacted Debug/Display to prevent accidental leakage. |
| ConfigSource | Enum identifying where a config value came from (Default, File, Environment, Programmatic). |
| EnforcementPreset | Named preset profiles for enforcement limits (e.g., "strict", "permissive", "ci"). |

## Dependencies

- Depends on: `engine::configuration` (config loading, merging, Secret)
- Used by: All other contexts (every module reads from Config)
- Used by: `CLI Boundary` (loads rigorix.toml, parses CLI flags)

## Key Files

| File | Purpose |
|------|---------|
| `cli/src/configuration/domain/config.rs` | CliConfig value object with all CLI settings |
| `cli/src/configuration/infrastructure/config.rs` | CliConfigLoader trait |
| `cli/src/configuration/infrastructure/config_impl.rs` | CliConfigLoaderImpl + validation helpers |
| `cli/src/cli_boundary/domain/error.rs` | CliError (ConfigNotFound, ConfigParseError, MissingConfig) — defined in cli_boundary module |
| `cli/src/main.rs` | Startup sequence: load → validate → bridge to engine → dispatch |
| `cli/.pi/scripts/ci/check_config_contracts.sh` | Automated contract validation (17 checks) |
| `cli/.pi/scripts/ci/check_config_coverage.sh` | Coverage threshold enforcement |
| `cli/.pi/scripts/ci/stage_config_proofing.sh` | CI stage wrapper (stage 12) |
| `engine/src/configuration/domain/config.rs` | Config aggregate, EnforcementPreset |
| `engine/src/configuration/domain/secret.rs` | Secret value object |
| `engine/src/configuration/application/service.rs` | ConfigService, SecretService traits |

## ADRs

| ADR | Title | Status |
|-----|-------|--------|
| ADR-001 | Domain-Driven Design with Bounded Contexts | Accepted |
| ADR-002 | CLI/Engine Architecture Split | Accepted |
| ADR-007 | Ephemeral CLI — No Daemon for v1 | Accepted |

## Proofing Scripts

| Script | Purpose | Stage |
|--------|---------|-------|
| `check_config_contracts.sh` | 17 automated checks for config contracts (traits, impls, fields, validation, wiring) | stage 12 — config_proofing |
| `check_config_coverage.sh` | Coverage thresholds (6+ loader tests, 2+ domain tests, 35+ overall) | stage 12 — config_proofing |
| `stage_config_proofing.sh` | CI stage wrapper — runs contracts + coverage + full CI validation | stage 12 — config_proofing |
