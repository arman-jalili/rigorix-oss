# Template Generation

## Module Status

**Status:** Planned — CLI integration layer over engine crate
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

LLM-based template creation from natural language. Can be triggered explicitly (`rigorix generate`) or as an automatic fallback during planning. Generated templates are persisted to `.rigorix/templates/` for reuse.

The engine crate (`rigorix-engine`) provides the `TemplateGenerator` trait and `ClaudeTemplateGenerator` implementation. The CLI layer wraps these with:
- `rigorix generate <intent>` command dispatch
- `--dry-run` (preview without saving) and `--stdout` (pipe output)
- Automatic persistence of fallback-generated templates to `.rigorix/templates/`
- Repository context building (file tree, dependencies, public API)

## Components

**CLI-facing:**
| Component | File (planned) | Module | Purpose |
|-----------|---------------|--------|---------|
| GenerateCommandHandler (trait) | `cli/src/template_generation/infrastructure/service.rs` | template_generation | Service trait for template generation |
| GenerateEngineHandler | `cli/src/template_generation/infrastructure/generate_handler_impl.rs` | template_generation | Implements GenerateCommandHandler via engine TemplateGenerator |
| TemplatePersistenceService | `cli/src/template_generation/infrastructure/persist_service_impl.rs` | template_generation | Saves generated TOML with atomic write-rename |
| RepoContextBuilder | `cli/src/template_generation/infrastructure/repo_context_impl.rs` | template_generation | Builds RepoContext from project directory |

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| TemplateGenerator (trait) | `engine/src/template_generation/domain/generator.rs` | `# Contract (Frozen)` |
| ClaudeTemplateGenerator | `engine/src/template_generation/domain/generator.rs` | Uses Anthropic Messages API |
| GeneratedTemplate | `engine/src/template_generation/domain/generator.rs` | Value object with TOML content + metadata |
| RepoContext | `engine/src/template_generation/domain/generator.rs` | Repo snapshot for LLM context |
| GeneratorError | `engine/src/template_generation/domain/generator.rs` | Typed error enum |

## Domain Events

| Event | Description | Triggered By |
|-------|-------------|-------------|
| TemplateGenerationRequested | LLM-based generation triggered (fallback or explicit) | PlanningPipeline / CliGenerateHandler |
| TemplateGenerated | A new TOML template was successfully generated | TemplateGenerator |
| TemplatePersisted | Generated template saved to `.rigorix/templates/` | TemplatePersistenceService |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| TemplateGenerator | Trait for LLM-based template generation from user intent. Implemented by ClaudeTemplateGenerator. |
| GeneratedTemplate | A TOML workflow template produced by the LLM generator with suggested name, ID, and usage stats. |
| RepoContext | Snapshot of repository structure used as context for LLM template generation. |
| TemplatePersistenceService | Saves generated templates to `.rigorix/templates/` with crash-safe atomic write. |
| CliGenerateCommand | The `rigorix generate <intent>` command with `--dry-run` and `--stdout` flags. |

## Dependencies

- Depends on: `engine::template_generation` (generator trait + Claude impl)
- Depends on: `Configuration` (LLM API key, generator config, templates directory path)
- Depends on: `Repo Engine` (SymbolGraph for public API extraction in RepoContext)
- Depends on: `Templates` (validates generated TOML as a valid Template)
- Depends on: `Budget Tracking` (LLM call budget for generation)
- Used by: `CLI Boundary` (exposes `rigorix generate`)
- Used by: `Planning Pipeline` (automatic fallback when no template matches)

## Key Files

| File | Purpose |
|------|---------|
| `cli/src/template_generation/infrastructure/service.rs` | GenerateCommandHandler trait (planned) |
| `cli/src/template_generation/infrastructure/generate_handler_impl.rs` | GenerateEngineHandler implementation (planned) |
| `cli/src/template_generation/infrastructure/persist_service_impl.rs` | TemplatePersistenceService (planned) |
| `cli/src/template_generation/infrastructure/repo_context_impl.rs` | RepoContextBuilder (planned) |
| `engine/src/template_generation/domain/generator.rs` | Engine: TemplateGenerator trait, ClaudeTemplateGenerator, GeneratedTemplate, RepoContext, GeneratorError |
| `engine/src/template_generation/application/` | Engine: service traits and DTOs |
| `engine/src/template_generation/infrastructure/` | Engine: repository interfaces for template storage |

## ADRs

| ADR | Title | Status |
|-----|-------|--------|
| ADR-001 | Domain-Driven Design with Bounded Contexts | Proposed |

## See Also

- FR-019: `rigorix generate <intent>` explicit template generation
- FR-020: Automatic fallback persistence
- FR-021: `--dry-run` preview mode
- FR-022: `--stdout` pipe mode
- FR-023: Reuse generated templates immediately
