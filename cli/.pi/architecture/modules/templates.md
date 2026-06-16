# Templates

## Module Status

**Status:** ✅ Implemented — contract freeze complete, proofing scripts active
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d
**Issues:** #266 (contract freeze), #268 (proofing), #269 (architecture readiness)

## Description

TOML-based workflow template definitions, parsing, validation, and runtime registry. Templates define reusable DAG patterns that the Planning Pipeline classifies against and instantiates into executable TaskGraphs.

Templates are stored in `.rigorix/templates/*.toml` and loaded at startup. The engine provides `TemplateParser`, `TemplateEngine`, and `BuiltinTemplates` (13 built-in templates). The CLI exposes `rigorix template list` and `rigorix template show` for inspection.

## Architecture

### Clean Architecture Layers

```
templates/
├── domain/           # TemplateCliError, TemplateCliEvent
│   ├── mod.rs
│   ├── error.rs      # TemplateCliError enum (typed CLI template errors)
│   └── event/        # TemplateCliEvent payload schemas
│       └── mod.rs
├── application/      # Service traits, DTO schemas
│   ├── mod.rs
│   ├── service.rs    # TemplateCommandService trait (frozen contract)
│   └── dto/          # TemplateListInput/Output, TemplateShowInput/Output
│       └── mod.rs    # + From conversions to cli_boundary DTOs
├── infrastructure/   # Trait implementations, repository interfaces
│   ├── mod.rs
│   ├── service.rs                    # Re-exports TemplateCommandService
│   ├── template_handler_impl.rs       # TemplateEngineHandler impl
│   └── repository/                   # TemplateCliRepository trait
│       └── mod.rs
└── interfaces/       # HTTP API contracts
    ├── mod.rs
    └── http/         # Endpoint definitions, request/response schemas
        └── mod.rs
```

### Data Flow

```
User CLI → dispatch_command() → TemplateCommandService(list|show)
                                     ↓
                              TemplateEngineHandler
                                     ↓
                          Engine::TemplateEngineService
                                     ↓
                          TemplateEngineImpl (registry + generation)
```

### CI Proofing (Stage 14)

The following scripts run automatically in the hardening pipeline:

| Script | Checks | Exit |
|--------|--------|------|
| `check_template_contracts.sh` | 15 checks — all interfaces have implementations | 0/1 |
| `check_template_coverage.sh` | 9 checks — coverage across all 4 layers | 0/1 |
| `stage_template_proofing.sh` | Wrapper: runs both + CI validation | 0/1 |

## Components

**CLI-facing (contract freeze):**
| Component | File | Module | Purpose |
|-----------|------|--------|---------|
| TemplateCommandService (trait) | `cli/src/templates/application/service.rs` | application | Service trait for template list/show (frozen) |
| TemplateEngineHandler | `cli/src/templates/infrastructure/template_handler_impl.rs` | infrastructure | Implements TemplateCommandService via engine |
| TemplateCliRepository (trait) | `cli/src/templates/infrastructure/repository/mod.rs` | infrastructure | Repository interface for template data (frozen) |
| TemplateCliError | `cli/src/templates/domain/error.rs` | domain | Typed CLI template error enum (frozen) |
| TemplateCliEvent | `cli/src/templates/domain/event/mod.rs` | domain | CLI template event schemas (frozen) |
| TemplateListInput/Output | `cli/src/templates/application/dto/mod.rs` | application | List command DTOs (frozen) |
| TemplateShowInput/Output | `cli/src/templates/application/dto/mod.rs` | application | Show command DTOs (frozen) |
| ListCliTemplatesResponse | `cli/src/templates/interfaces/http/mod.rs` | interfaces | HTTP list response (frozen) |
| ShowCliTemplateResponse | `cli/src/templates/interfaces/http/mod.rs` | interfaces | HTTP show response (frozen) |

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| Template (root aggregate) | `engine/src/templates/domain/template.rs` | `# Contract (Frozen)` |
| TemplateNode | `engine/src/templates/domain/template.rs` | Single node with action, dependencies, retry |
| TemplateAction | `engine/src/templates/domain/template.rs` | Tagged union: FileRead, FileWrite, RunCommand, etc. |
| ParameterDef | `engine/src/templates/domain/template.rs` | Parameter schema (name, type, required, default) |
| TemplateEngineService (trait) | `engine/src/templates/application/service.rs` | Registry service: register, list, get, remove |
| TemplateParserService (trait) | `engine/src/templates/application/service.rs` | Parses TOML strings/files into Template |
| BuiltinTemplates | `engine/src/templates/` | 13 built-in template definitions |
| TemplateError | `engine/src/templates/domain/error.rs` | Typed error enum |

## Domain Events

| Event | Description | Triggered By |
|-------|-------------|-------------|
| TemplateCliEvent::TemplateListRequested | List operation initiated | CLI dispatch |
| TemplateCliEvent::TemplateListCompleted | List completed successfully | TemplateEngineHandler |
| TemplateCliEvent::TemplateShowRequested | Show operation initiated | CLI dispatch |
| TemplateCliEvent::TemplateShowCompleted | Show completed successfully | TemplateEngineHandler |
| TemplateCliEvent::TemplateOperationFailed | Template operation failed | TemplateEngineHandler |
| TemplateCliEvent::TemplateEngineInitialized | Engine handler initialized | TemplateEngineHandler::new() |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| Template | TOML-defined workflow definition with parameters, nodes, and action graph. Root aggregate. |
| TemplateNode | A single step in a template with an action, dependencies, and retry configuration. |
| TemplateAction | The action a node performs: file_read, file_write, file_append, file_patch, run_command, lsp_query, git_read, git_stage, git_commit. |
| ParameterDef | Schema for a template parameter: name, type, required flag, default, constraints. |
| TemplateEngine | Registry service managing template lifecycle: register, list, get, remove. |
| TemplateCommandService | CLI-side trait for template list/show commands (frozen contract). |
| TemplateEngineHandler | CLI-side implementation wrapping the engine's TemplateEngineImpl. |
| TemplateCliRepository | Repository interface for CLI-level template data caching. |

## Dependencies

- Depends on: `engine::templates` (parsing, engine, builtins)
- Depends on: `Configuration` (template directory path)
- Depends on: `CLI Boundary` (CliError, CliConfig, output formatting)
- Used by: `Planning Pipeline` (classification and graph generation)
- Used by: `Template Generation` (validates generated TOML)
- Used by: `CLI Boundary` (exposes `rigorix template list/show`)

## Key Files

| File | Purpose |
|------|---------|
| `cli/src/templates/application/service.rs` | TemplateCommandService trait — canonical contract |
| `cli/src/templates/application/dto/mod.rs` | DTO schemas for list/show operations |
| `cli/src/templates/domain/error.rs` | TemplateCliError — typed error enum |
| `cli/src/templates/domain/event/mod.rs` | TemplateCliEvent — event payload schemas |
| `cli/src/templates/infrastructure/repository/mod.rs` | TemplateCliRepository — repository interface |
| `cli/src/templates/infrastructure/template_handler_impl.rs` | TemplateEngineHandler implementation |
| `cli/src/templates/interfaces/http/mod.rs` | HTTP API endpoint contracts |
| `cli/docs/runbook-template.md` | Operations runbook |
| `cli/docs/dr-plan-template.md` | Disaster recovery plan |
| `.pi/scripts/ci/check_template_contracts.sh` | Contract implementation proofing script |
| `.pi/scripts/ci/check_template_coverage.sh` | Coverage threshold proofing script |
| `.pi/scripts/ci/stage_template_proofing.sh` | CI stage wrapper for template proofing |
| `engine/src/templates/domain/template.rs` | Template aggregate, TemplateNode, TemplateAction, ParameterDef |
| `engine/src/templates/application/` | TemplateParser, TemplateEngine service traits and impls |
| `engine/src/templates/` | BuiltinTemplates (13 built-in definitions) |

## ADRs

| ADR | Title | Status |
|-----|-------|--------|
| ADR-001 | Domain-Driven Design with Bounded Contexts | Proposed |
| ADR-002 | CLI/Engine Split | Proposed |

## Related Issues

| Issue | Description | Status |
|-------|-------------|--------|
| #266 | Contract freeze — define interfaces and contracts | ✅ Merged (PR #270) |
| #268 | Proofing — validation scripts + CI integration | ✅ Merged (PR #271) |
| #269 | Architecture readiness — runbook, DR, docs, CI enforcement | ✅ In progress |
