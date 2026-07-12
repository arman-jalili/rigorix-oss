# Disaster Recovery Plan: template-tools

## Overview

This document covers recovery procedures for the template-tools module.
Templates are stored as TOML files in `.rigorix/templates/` — the DR strategy
focuses on filesystem backup and restore.

## RTO/RPO Targets

| Metric | Target | Notes |
|--------|--------|-------|
| RTO (Recovery Time Objective) | 5 minutes | Time to restore template access |
| RPO (Recovery Point Objective) | 1 hour | Maximum data loss window |

## Backup Strategy

### Schedule

- **Automatic**: Templates are immutable-on-write (atomic temp-file + rename).
  Backups happen via standard filesystem snapshots.
- **Recommended**: Snapshot `.rigorix/templates/` hourly.

### Backup Contents

- All `.toml` files in `.rigorix/templates/`
- Directory structure (flat — no subdirectories expected)

### Backup Command

```bash
# Snapshot template directory
tar czf "template-backup-$(date +%Y%m%d-%H%M).tar.gz" .rigorix/templates/
```

## Restore Procedure

### Full Restore

1. Stop any ongoing template write operations
2. Restore from backup:
   ```bash
   tar xzf template-backup-20260101-1200.tar.gz
   ```
3. Verify templates are accessible:
   ```bash
   ls -la .rigorix/templates/*.toml
   ```
4. Spot-check a template:
   ```bash
   cat .rigorix/templates/example.toml
   ```

### Partial Restore (Single Template)

If a single template file is corrupted or accidentally deleted:

1. Extract the specific file from backup:
   ```bash
   tar xzf template-backup.tar.gz ".rigorix/templates/template-name.toml"
   ```
2. No restart needed — templates are read on-demand from disk.

## Failover

### Scenario: Disk Failure

If the disk containing `.rigorix/templates/` fails:

1. **Impact**: All template operations return errors
2. **Mitigation**:
   - Restore from latest backup to a new disk
   - Update `base_path` configuration to point to new location
   - Update `TemplateRepositoryConfig` programmatically

### Scenario: Corrupted Template

If a template file becomes corrupted:

1. **Impact**: `rigorix_get_template` for that specific template fails with
   `DeserializationFailed`
2. **Mitigation**:
   - Delete corrupted file: `rm .rigorix/templates/bad-template.toml`
   - Restore from backup or recreate

### Scenario: Catastrophic Loss (Full Repository)

If all templates are lost:

1. **Impact**: All template operations return empty or error
2. **Mitigation**:
   - Full restore from latest backup
   - If no backup: recreate templates from scratch using `rigorix_create_template`

## Backward Compatibility

- Template TOML format is versioned via the `version` field in `PlanTemplate`
- Old format templates are readable as long as they match `PlanTemplate` schema
- Structural changes require migration scripts (rare)

## Testing the Plan

Test the DR plan quarterly:

1. **Restore test**: Restore backup to a temp directory and verify all files
2. **Access test**: After restore, run `rigorix_list_templates` and verify
   template count matches
3. **Content test**: After restore, run `rigorix_get_template` on a key
   template and verify its structure

## Incident Response

| Severity | Definition | Response Time |
|----------|-----------|---------------|
| P1 | All templates lost | 15 minutes |
| P2 | Single template corrupted | 1 hour |
| P3 | Able to work around | Next business day |
