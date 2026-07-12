# Runbook: template-tools

## Overview

Template-tools bridges MCP tool calls to template filesystem operations: discover
templates (`rigorix_list_templates`), read templates (`rigorix_get_template`),
create templates (`rigorix_create_template`), and validate template structure
(`rigorix_validate_template`).

Templates are stored as TOML files in `.rigorix/templates/` directory.

## Startup Sequence

1. **Dependencies available**: MCP Server must be running (provides ToolRegistry,
   RequestRouter)
2. **EngineFacade optional**: Validation handler can work without engine, but
   enforcement validation requires EngineFacade
3. **Template directory**: `.rigorix/templates/` is created lazily on first write

### Startup Order

```
MCP Server → EngineFacade (optional) → template-tools handlers
```

## Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `template_tools.base_path` | `.rigorix/templates` | Directory for template TOML files |

## Graceful Shutdown

Template-tools uses no persistent connections or background tasks. No special
shutdown sequence is needed. Template files persist on disk across restarts.

## Health Check

Template-tools health is verified by:
1. Accessing the template directory (must be readable/writable)
2. Running `exists()` on a known template name

## Common Failure Modes

### Template Not Found

**Symptom**: `rigorix_get_template` returns error "Template not found: ..."

**Cause**: Template name misspelled or template file deleted.

**Recovery**:
1. List available templates: `rigorix_list_templates`
2. Verify template name spelling
3. If deleted, recreate: `rigorix_create_template`

### Template Already Exists

**Symptom**: `rigorix_create_template` returns error "Template already exists"

**Cause**: Template name already in use and `overwrite: false`.

**Recovery**:
1. Set `overwrite: true` to replace
2. Or choose a different template name

### Invalid Template Name

**Symptom**: `rigorix_create_template` returns error containing "invalid characters"

**Cause**: Template name contains characters outside `[a-zA-Z0-9_-]`.

**Recovery**: Use only alphanumeric characters, underscores, and hyphens.

### Filesystem Error

**Symptom**: Repository returns `RepositoryError`

**Cause**: Disk full, permission denied, or directory deleted.

**Recovery**:
1. Check disk space: `df -h .rigorix/templates/`
2. Verify permissions: `ls -la .rigorix/templates/`
3. Create directory if missing: `mkdir -p .rigorix/templates/`

## Monitoring

### Key Metrics

- `template_tools.templates_count`: Number of templates on disk
- `template_tools.list_duration_ms`: Duration of list operations
- `template_tools.create_duration_ms`: Duration of create operations
- `template_tools.errors_total`: Total template tool errors

### Logging

All template operations log at `info` level with:
- `template_name`: The template involved
- `operation`: list / get / create / delete / validate
- `duration_ms`: Operation duration
- `error`: Error details (if applicable)

## Dependencies

| Component | Type | Impact if Down |
|-----------|------|---------------|
| Filesystem | Runtime | No templates available |
| EngineFacade | Optional | No enforcement validation |

## Restart Procedure

1. No special procedure needed — template data persists on disk
2. Verify template directory exists after restart
3. This is documented in the [DR Plan](./dr-plan-template-tools.md)
