# Configuration

## Module Status

**Status:** ✅ Implemented — contract freeze complete, proofing scripts active
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d
**Issues:** #282 (contract freeze), #284 (proofing), #285 (architecture readiness)

## Description

Multi-source configuration loading with layered merging:
1. Built-in defaults
2. `rigorix.toml` (project config file in working directory)
3. Environment variables (prefix: `RIGORIX_`)
4. CLI flags (highest precedence)

Provides validated `Config` to all other contexts. Includes `Secret` type for safe API key handling (redacted Debug/Display, excluded from logs and TUI output).

## Architecture

### Clean Architecture Layers

```
configuration/
├── domain/           # CliConfig, ConfigCliError, ConfigCliEvent
│   ├── mod.rs
│   ├── config.rs     # CliConfig value object with all CLI settings
│   ├── error.rs      # ConfigCliError enum (typed CLI config errors)
│   └── event/        # ConfigCliEvent payload schemas
│       └── mod.rs
├── application/      # Service traits, DTO schemas
│   ├── mod.rs
│   ├── service.rs    # CliConfigLoader trait (frozen contract)
│   └── dto/          # LoadConfigInput/Output, ValidateConfig types
│       └── mod.rs
├── infrastructure/   # Trait implementations, repository interfaces
│   ├── mod.rs
│   ├── config.rs                    # Re-exports CliConfigLoader
│   ├── config_impl.rs               # CliConfigLoaderImpl + validation helpers
│   └── repository/                  # ConfigCliRepository trait
│       └── mod.rs
└── interfaces/       # HTTP API contracts
    ├── mod.rs
    └── http/         # Endpoint definitions, request/response schemas
        └── mod.rs
```

### Data Flow

```
CLI flags → CliConfigLoaderImpl (infrastructure)
                ↓
         Merge: flags > env > file > defaults
                ↓
            CliConfig
                ↓
         → dispatch_command()
         → init_engine_config() → engine ConfigService
```

## Components

**CLI-facing (contract freeze):**
| Component | File | Module | Purpose |
|-----------|------|--------|---------|
| CliConfigLoader (trait) | `cli/src/configuration/application/service.rs` | application | Config loading interface (frozen contract) |
| CliConfigLoaderImpl | `cli/src/configuration/infrastructure/config_impl.rs` | infrastructure | Multi-source merging implementation |
| ConfigCliRepository (trait) | `cli/src/configuration/infrastructure/repository/mod.rs` | infrastructure | Repository interface for config data (frozen) |
| ConfigCliError | `cli/src/configuration/domain/error.rs` | domain | Typed CLI config error enum (frozen) |
| ConfigCliEvent | `cli/src/configuration/domain/event/mod.rs` | domain | CLI config event schemas (frozen) |
| LoadConfigInput/Output | `cli/src/configuration/application/dto/mod.rs` | application | Config load DTOs (frozen) |
| ValidateConfigInput/Output | `cli/src/configuration/application/dto/mod.rs` | application | Config validation DTOs (frozen) |
| ConfigResponse | `cli/src/configuration/interfaces/http/mod.rs` | interfaces | HTTP get config response (frozen) |

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
| ConfigCliEvent::ConfigLoadStarted | Config loading started | CliConfigLoaderImpl::load() |
| ConfigCliEvent::ConfigLoaded | Config loaded successfully | CliConfigLoaderImpl::load() |
| ConfigCliEvent::ConfigLoadFailed | Config loading failed | CliConfigLoaderImpl::load() |
| ConfigCliEvent::EnvVarApplied | Env var override applied | CliConfigLoaderImpl::load() |
| ConfigCliEvent::CliFlagApplied | CLI flag override applied | CliConfigLoaderImpl::load() |
| ConfigCliEvent::ApiKeyValidated | API key validation completed | validate_api_key_for_command() |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| CliConfig | CLI-specific configuration value object (output format, color, TUI, log settings). |
| CliConfigLoader | Trait for loading and merging configuration from multiple sources. |
| CliConfigLoaderImpl | Concrete implementation with multi-source merging: flags > env > file > defaults. |
| ConfigCliError | Typed CLI configuration error enum (6 variants). |
| ConfigCliEvent | Event payload schemas for CLI config operations (7 events). |
| ConfigCliRepository | Repository interface for CLI-level config data persistence. |

## Dependencies

- Depends on: `engine::configuration` (config loading, merging, Secret)
- Used by: All other contexts (every module reads from Config)
- Used by: `CLI Boundary` (loads rigorix.toml, parses CLI flags)

## Key Files

| File | Purpose |
|------|---------|
| `cli/src/configuration/application/service.rs` | CliConfigLoader trait — canonical contract |
| `cli/src/configuration/application/dto/mod.rs` | DTO schemas for load/validate operations |
| `cli/src/configuration/domain/error.rs` | ConfigCliError — typed error enum |
| `cli/src/configuration/domain/event/mod.rs` | ConfigCliEvent — event payload schemas |
| `cli/src/configuration/infrastructure/repository/mod.rs` | ConfigCliRepository — repository interface |
| `cli/src/configuration/infrastructure/config_impl.rs` | CliConfigLoaderImpl + validation helpers |
| `cli/src/configuration/interfaces/http/mod.rs` | HTTP API endpoint contracts |
| `cli/docs/runbook-configuration.md` | Operations runbook |
| `cli/docs/dr-plan-configuration.md` | Disaster recovery plan |
| `.pi/scripts/ci/check_config_contracts.sh` | Contract implementation proofing script (17 checks) |
| `.pi/scripts/ci/check_config_coverage.sh` | Coverage threshold proofing script |
| `.pi/scripts/ci/stage_config_proofing.sh` | CI stage wrapper (stage 12) |

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

## Related Issues

| Issue | Description | Status |
|-------|-------------|--------|
| #282 | Contract freeze — define interfaces and contracts | ✅ Merged (PR #286) |
| #284 | Proofing — validation scripts + CI integration | ✅ Existing stage 12 updated |
| #285 | Architecture readiness — runbook, DR, docs, CI enforcement | ✅ In progress |
