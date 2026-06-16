# Templates

## Module Status

**Status:** Planned — CLI integration over engine contracts
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

TOML-based workflow template definitions, parsing, validation, and runtime registry. Templates define reusable DAG patterns that the Planning Pipeline classifies against and instantiates into executable TaskGraphs.

Templates are stored in `.rigorix/templates/*.toml` and loaded at startup. The engine provides `TemplateParser`, `TemplateEngine`, and `BuiltinTemplates` (13 built-in templates). The CLI exposes `rigorix template list` and `rigorix template show` for inspection.

## Components

**CLI-facing:**
| Component | File (planned) | Purpose |
|-----------|---------------|---------|
| TemplateListCommand | `cli/src/template_cmd.rs` | Lists all registered templates with descriptions |
| TemplateShowCommand | `cli/src/template_cmd.rs` | Shows full template definition (TOML) |

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| Template (root aggregate) | `engine/src/templates/domain/template.rs` | `# Contract (Frozen)` |
| TemplateNode | `engine/src/templates/domain/template.rs` | Single node with action, dependencies, retry |
| TemplateAction | `engine/src/templates/domain/template.rs` | Tagged union: FileRead, FileWrite, RunCommand, etc. |
| ParameterDef | `engine/src/templates/domain/template.rs` | Parameter schema (name, type, required, default) |
| TemplateEngine | `engine/src/templates/application/` | Registry service: register, list, get, remove |
| TemplateParser | `engine/src/templates/application/` | Parses TOML strings/files into Template |
| BuiltinTemplates | `engine/src/templates/` | 13 built-in template definitions |
| TemplateError | `engine/src/templates/domain/error.rs` | Typed error enum |

## Domain Events

| Event | Description | Triggered By |
|-------|-------------|-------------|
| TemplateRegistered | A new template was added to the registry | TemplateEngine |
| TemplateRemoved | A template was removed from the registry | TemplateEngine |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| Template | TOML-defined workflow definition with parameters, nodes, and action graph. Root aggregate. |
| TemplateNode | A single step in a template with an action, dependencies, and retry configuration. |
| TemplateAction | The action a node performs: file_read, file_write, file_append, file_patch, run_command, lsp_query, git_read, git_stage, git_commit. |
| ParameterDef | Schema for a template parameter: name, type, required flag, default, constraints. |
| TemplateEngine | Registry service managing template lifecycle: register, list, get, remove. |

## Dependencies

- Depends on: `engine::templates` (parsing, engine, builtins)
- Depends on: `Configuration` (template directory path)
- Used by: `Planning Pipeline` (classification and graph generation)
- Used by: `Template Generation` (validates generated TOML)
- Used by: `CLI Boundary` (exposes `rigorix template list/show`)

## Key Files

| File | Purpose |
|------|---------|
| `cli/src/template_cmd.rs` | CLI template list/show commands |
| `engine/src/templates/domain/template.rs` | Template aggregate, TemplateNode, TemplateAction, ParameterDef |
| `engine/src/templates/application/` | TemplateParser, TemplateEngine service traits and impls |
| `engine/src/templates/` | BuiltinTemplates (13 built-in definitions) |

## ADRs

| ADR | Title | Status |
|-----|-------|--------|
| ADR-001 | Domain-Driven Design with Bounded Contexts | Proposed |
