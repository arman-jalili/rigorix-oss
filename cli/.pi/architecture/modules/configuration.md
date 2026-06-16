# Configuration

## Module Status

**Status:** Planned — CLI integration over engine contracts
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
| Component | File (planned) | Purpose |
|-----------|---------------|---------|
| ConfigMergeService | `cli/src/config.rs` | Merges CLI flags + env vars + rigorix.toml into engine Config |
| ConfigValidator | `cli/src/config.rs` | Pre-flight validation: API key present, template dir exists, etc. |

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
| `cli/src/config.rs` | CLI-side config loading and merging |
| `engine/src/configuration/domain/config.rs` | Config aggregate, EnforcementPreset |
| `engine/src/configuration/domain/secret.rs` | Secret value object |
| `engine/src/configuration/application/service.rs` | ConfigService, SecretService traits |

## ADRs

| ADR | Title | Status |
|-----|-------|--------|
| ADR-001 | Domain-Driven Design with Bounded Contexts | Proposed |
