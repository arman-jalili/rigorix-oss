# Runbook: Templates Module

> **Module:** `cli/src/templates/`
> **Version:** 0.1.0
> **Last Updated:** 2026-06-16

## Overview

The Templates module provides CLI commands for listing and inspecting TOML-based
workflow template definitions. It wraps the engine's `TemplateEngineService` for
CLI consumption via `rigorix template list` and `rigorix template show`.

## Architecture

```
User → CliArgs → dispatch_command() → TemplateCommandService → TemplateEngineHandler → Engine
                                         (trait, application/)     (impl, infrastructure/)
```

The module follows Clean Architecture with 4 layers:
- **Domain** (`domain/`): `TemplateCliError`, `TemplateCliEvent`
- **Application** (`application/`): `TemplateCommandService` trait, DTOs
- **Infrastructure** (`infrastructure/`): `TemplateEngineHandler` impl, `TemplateCliRepository` trait
- **Interfaces** (`interfaces/`): HTTP API contracts

## Startup Sequence

1. `main()` creates `TemplateEngineHandler::new(config)` → initializes engine's `TemplateEngineImpl`
2. Engine loads built-in templates (13 built-ins) and scans `.rigorix/templates/` directory
3. `dispatch_command()` routes `template list` / `template show` to the handler
4. Handler calls engine's `list_templates()` / `get_template()` and formats output

## Dependencies

| Dependency | Type | Required | Notes |
|-----------|------|----------|-------|
| rigorix-engine | Rust crate | Yes | TemplateEngineService, TemplateParserService |
| CliConfig | Config | Yes | Template directory paths, output format |
| .rigorix/templates/ | Directory | No | User-defined templates directory |

## Graceful Shutdown

The Templates module does not maintain long-lived connections or background tasks.
Shutdown is immediate and safe at any point:
1. Signal handler (Ctrl+C) → `main()` exits → handler dropped
2. Engine's `TemplateEngineImpl` drops → registered templates freed
3. No state persistence needed — templates are re-loaded on restart

## Common Failure Modes

| Failure | Symptom | Recovery |
|---------|---------|----------|
| Engine not initialized | `CliError::Internal` on list/show | Check engine crate initialization |
| Template not found | `TemplateError::NotFound` | Verify template ID with `template list` |
| Template directory missing | Empty list (not an error) | Create `.rigorix/templates/` for user templates |
| Config load failure | `CliError::ConfigNotFound/ParseError` | Run `rigorix init` to create config |

## Configuration Reference

| Setting | Source | Default | Description |
|---------|--------|---------|-------------|
| `template_dirs` | rigorix.toml | `["templates", ".rigorix/templates"]` | Directories to scan for template files |
| `output_format` | CLI flag / env / config | `pretty` | Output format: pretty, json, quiet |

## Monitoring

- **Logs**: Template operations logged at `info` level with template ID
- **Events**: `TemplateCliEvent` emitted for list/show operations (serializable)
- **Metrics**: No dedicated metrics (module is thin passthrough to engine)

## Troubleshooting

### Template list is empty
```bash
# Check if engine was initialized properly
rigorix template list
# Expected: list of built-in and user templates
# If empty: check template directories exist
ls -la .rigorix/templates/
```

### Template show fails with not found
```bash
# Verify the template exists
rigorix template list
# Use exact ID from list output
rigorix template show <exact-id>
```

### Engine errors
All engine errors are wrapped in `CliError::Engine` and include the original
error message. Check the error detail for the specific engine issue.
