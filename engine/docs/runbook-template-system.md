# Runbook: template-system Module

<!--
Canonical Reference: .pi/architecture/modules/template-system.md
Last Updated: 2026-06-14
-->

## Overview

The `template-system` module manages workflow template definitions stored as TOML files.
It handles parsing (TemplateParser), schema validation, template engine instantiation
(TOML → executable graph) (TemplateEngine), and loading of built-in templates.

## Components

| Component | Type | Description |
|-----------|------|-------------|
| `Template` | Domain entity | Workflow template aggregate with metadata, parameters, and node definitions |
| `TemplateNode` | Domain entity | Single DAG node with action, dependencies, retry config, and validation rules |
| `TemplateAction` | Domain enum | 9 action variants (FileRead, FileWrite, FileAppend, FilePatch, RunCommand, LspQuery, GitRead, GitStage, GitCommit) |
| `ParameterDef` | Domain entity | Parameter definition with name, type, required flag, default, and constraints |
| `TemplateParserImpl` | Application service | TOML deserialization, structural validation, cycle detection, directory loading |
| `TemplateEngineImpl` | Application service | In-memory registry, parameter substitution, topological sort, graph generation |
| `InMemoryTemplateRepository` | Infrastructure | In-memory template storage (test double) |
| `FileSystemTemplateRepository` | Infrastructure | (Planned) Filesystem-backed template storage |

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| tokio runtime | Yes | Async I/O for template file operations |
| serde + toml | Yes | TOML deserialization for template definitions |
| serde_json | Yes | JSON value types for parameter substitution |
| async-trait | Yes | Async trait support for service interfaces |
| uuid | Yes | Execution identifiers for graph generation |
| chrono (dev only) | Yes | Event timestamp generation |

### Initialization

1. Create an `InMemoryTemplateRepository` or `FileSystemTemplateRepository`
2. Create a `TemplateParserImpl` wrapping the repository
3. Create a `TemplateEngineImpl` for the runtime registry
4. Load built-in templates via `TemplateParserService::load_builtins()`
5. Register parsed templates in the engine via `TemplateEngineService::register()`
6. Templates are now ready for graph generation

```rust
use rigorix::templates::application::*;
use rigorix::templates::infrastructure::repository::*;

// Create repository and parser
let repo = InMemoryTemplateRepository::new();
let parser = TemplateParserImpl::new(repo);
let engine = TemplateEngineImpl::new();

// Load built-in templates
let builtins = parser.load_builtins(LoadBuiltinsInput {
    categories: None,
    overwrite: false,
}).await?;

// Register built-in templates in the engine
for id in builtins.loaded {
    if let Ok(Some(summary)) = engine.get_template(GetTemplateInput {
        template_id: id.clone(),
    }).await {
        // Template is registered
    }
}

// Parse a custom template
let output = parser.parse_str(ParseStrInput {
    toml_content: template_toml.to_string(),
    source: Some("custom".to_string()),
    validate: true,
}).await?;

// Register it
if output.valid {
    engine.register(RegisterInput {
        template: output.template,
        overwrite: false,
    }).await?;
}
```

## Configuration Reference

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RIGORIX_TEMPLATE_DIRS` | `templates/, .rigorix/templates/` | Comma-separated directories to scan for template files |
| `RIGORIX_TEMPLATE_EXTENSION` | `toml` | File extension for template files |
| `RIGORIX_TEMPLATE_LOAD_BUILTINS` | `true` | Whether to load built-in templates on startup |

### Template Directory Layout

```
project/
├── templates/
│   ├── read-file.toml             # Custom template overrides
│   └── git-commit.toml
└── .rigorix/
    └── templates/
        └── project-custom.toml    # Project-local templates
```

## Graceful Shutdown

### Procedure

1. **Complete current operations:** Wait for any in-flight `parse_file`, `parse_str`,
   or `generate` calls to complete.
2. **Save engine state (optional):** Serialize the template registry if hot-reload
   on next startup is desired.
3. **No explicit cleanup needed:** The template engine is stateless between
   executions — templates are loaded from files on each startup.

### Signal Handling

| Signal | Behaviour | State Recovery |
|--------|-----------|----------------|
| SIGTERM | Graceful shutdown | All in-flight parses complete; engine state is ephemeral |
| SIGINT (Ctrl+C) | Interrupt | Partial parse results discarded; no data loss |
| SIGKILL | Immediate termination | Template files on disk unaffected |

## Common Failure Modes and Recovery

### Failure: Invalid TOML Template File

**Symptoms:** `TemplateParserService::parse_file()` returns `ParseOutput` with
`valid: false` and parse errors.

**Recovery:**

1. Check the error message for the specific parse error (line number provided)
2. Validate the TOML file manually:
   ```bash
   cargo run -- validate-template --file templates/my-template.toml
   ```
3. Fix the TOML syntax error and reload

### Failure: Template Missing Required Parameter

**Symptoms:** `TemplateEngineService::generate()` returns
`TemplateError::MissingParameter`.

**Recovery:**

1. Check the required parameters for the template via `TemplateEngineService::get_template()`
2. Provide the missing parameter in the `params` map
3. If the parameter should be optional, update the template definition to set
   `required = false` and provide a `default` value

### Failure: Cycle Detected in Template Dependencies

**Symptoms:** `TemplateEngineService::generate()` returns a `GenerateOutput` with
`valid: false` and "Cycle detected" error.

**Recovery:**

1. List the template's nodes to inspect dependency relationships:
   ```bash
   cargo run -- inspect-template --id my-template
   ```
2. Identify the circular dependency from the cycle detection output
3. Break the cycle by removing or restructuring `depends_on` references
4. Re-register the fixed template

### Failure: Template Not Found in Registry

**Symptoms:** `TemplateEngineService::generate()` returns
`TemplateError::NotFound`.

**Recovery:**

1. List available templates via `TemplateEngineService::list_templates()`
2. Verify the template was registered
3. If not registered:
   ```bash
   cargo run -- register-template --file templates/my-template.toml
   ```

### Failure: Parameter Type Mismatch

**Symptoms:** Template generation fails with invalid parameter errors.

**Recovery:**

1. Check the parameter type in the template definition (`param_type` field)
2. Ensure the provided value matches the expected type
3. Supported types: `path`, `string`, `int`, `float`, `bool`, `enum`, `json`

## Observability

### Metrics

| Metric | Source | Description |
|--------|--------|-------------|
| `templates.parsed` | Counter | Total template parse operations |
| `templates.registered` | Counter | Total template registrations |
| `templates.generated` | Counter | Total graph generation operations |
| `templates.parse_errors` | Counter | Failed parse operations |
| `templates.generate_errors` | Counter | Failed graph generation operations |
| `templates.registered_count` | Gauge | Number of templates in the registry |
| `templates.parse_duration_ms` | Histogram | Parse operation latency |
| `templates.generate_duration_ms` | Histogram | Graph generation latency |

### Health Check

The `/api/v1/templates/health` endpoint returns:

```json
{
  "status": "ok",
  "registered_templates": 13,
  "builtin_templates": 13
}
```

### Structured Logging

Key log events:

| Event | Level | Context |
|-------|-------|---------|
| Template parsed | INFO | template_id, node_count, param_count |
| Template registered | INFO | template_id, total_templates |
| Graph generated | INFO | template_id, node_count, execution_id |
| Template parse failed | ERROR | source_path, error |
| Template validation failed | WARN | template_id, errors |
| Parameter validation failed | WARN | template_id, param, expected, actual |

---
*Last updated: 2026-06-14*
