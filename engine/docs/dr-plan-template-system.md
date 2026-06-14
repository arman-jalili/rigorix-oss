# Disaster Recovery Plan: template-system Module

<!--
Canonical Reference: .pi/architecture/modules/template-system.md
Last Updated: 2026-06-14
-->

## Scope

This DR plan covers the `template-system` module — the TOML-based workflow
template parser and engine. The module manages template definitions as TOML
files and an in-memory template registry. Since templates are defined on disk
and parsed at startup, the primary risk is template file corruption or loss.

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 2 minutes | Templates can be re-parsed from TOML files on restart |
| RPO (Recovery Point Objective) | < 5 minutes | Template TOML files change infrequently; Git history provides backup |

## Backup Strategy

### What to Back Up

Template definitions are stored as TOML files. The module has no persistent
runtime state — the in-memory template registry is rebuilt on each startup.

| Item | Location | Backup Priority |
|------|----------|-----------------|
| Custom template TOML files | `templates/*.toml` | High — user-defined workflow definitions |
| Project-local templates | `.rigorix/templates/*.toml` | High — project-specific definitions |
| Built-in templates | Embedded in binary | None — restored from binary on reinstall/rebuild |
| Architecture docs | `.pi/architecture/modules/template-system.md` | Medium — restore module design context |
| Contract freeze files | `src/templates/` | High — source of truth for all interfaces |

### Backup Schedule

| Type | Frequency | Retention | Method |
|------|-----------|-----------|--------|
| Version control | Every commit | Indefinite | Git — all template files and source code |
| Periodic archive | Weekly | 90 days | Git bundle of template files |
| Built-in templates | With release | Per release | Embedded in compiled binary |

### Backup Automation

```bash
#!/usr/bin/env bash
# Backup template definitions
BACKUP_DIR="/var/backups/rigorix/templates"
mkdir -p "$BACKUP_DIR"
git archive HEAD:templates/ --format=tar | tar -x -C "$BACKUP_DIR"
echo "Templates backed up to $BACKUP_DIR at $(date)"
```

## Restore Procedure

### Scenario 1: Template File Lost or Corrupted

**Severity:** Low — single template unavailable

**Steps:**

1. **Identify the affected template:**
   ```bash
   # Check if template file exists
   ls -la templates/my-template.toml
   # Check error log for parse failures
   grep "Template parse failed" /var/log/rigorix.log
   ```

2. **Restore from Git:**
   ```bash
   git checkout HEAD -- templates/my-template.toml
   ```

3. **Validate the restored template:**
   ```bash
   # The template will be re-parsed on next engine load
   # Or trigger a reload via API:
   curl -X POST /api/v1/templates/load
   ```

### Scenario 2: All Template Files Lost

**Severity:** Medium — all custom templates unavailable

**Steps:**

1. **Restore from latest Git commit:**
   ```bash
   git checkout HEAD -- templates/
   ```

2. **If Git history unavailable, restore from backup:**
   ```bash
   cp /var/backups/rigorix/templates/*.toml templates/
   ```

3. **Restart or reload:**
   ```bash
   # Full restart (if orchestrator manages templates):
   systemctl restart rigorix
   
   # Or API reload (if hot-reload endpoint exists):
   curl -X POST /api/v1/templates/load -d '{"source": "filesystem"}'
   ```

4. **Verify built-in templates are still available:**
   ```bash
   curl -X GET /api/v1/templates | jq '.templates[].id'
   # Should see 13 built-in template IDs
   ```

### Scenario 3: Built-in Template Corruption (Binary Level)

**Severity:** Critical — core templates unavailable

**Steps:**

1. **Rebuild from source:**
   ```bash
   cd rigorix-oss/engine
   cargo build --release
   ```

2. **Verify built-in templates:**
   ```bash
   ./target/release/rigorix template list
   ```

3. **If code-level corruption, restore from Git:**
   ```bash
   git checkout HEAD -- src/templates/infrastructure/builtin_templates.rs
   cargo build --release
   ```

### Scenario 4: Complete Module Failure

**Severity:** Critical — template system unavailable

**Steps:**

1. **Verify the root cause:**
   ```bash
   # Check service logs
   journalctl -u rigorix --no-pager | grep -i template
   
   # Check for dependency issues
   cargo check 2>&1 | grep template
   ```

2. **Restore the module from source control:**
   ```bash
   git checkout HEAD -- src/templates/
   cargo check
   cargo test --lib templates
   ```

3. **Rebuild and redeploy:**
   ```bash
   cargo build --release
   systemctl restart rigorix
   ```

4. **Verify recovery:**
   ```bash
   curl -X GET /api/v1/templates | jq '.total'
   # Expected: 13 (built-in) + custom templates
   ```

## Failover Plan

The template-system module is stateless and single-instance. Failover is
achieved by simply starting a new instance, which automatically loads all
templates from disk.

### Single-Instance to New Instance

1. **Provision new instance** with same template directory structure
2. **Copy or sync template files**: `rsync -av templates/ user@new-host:templates/`
3. **Start new instance**: `systemctl start rigorix`
4. **Verify templates loaded**: `curl http://new-host:8080/api/v1/templates`
5. **Switch traffic** to the new instance

### Multi-Region Considerations

For cross-region deployments:
- Template files should be stored in a shared, replicated filesystem (NFS, S3)
- Or each region maintains its own template set with CI/CD sync
- Built-in templates are identical across all instances

## Data Integrity Verification

### Automated Checks

| Check | Frequency | Script |
|-------|-----------|--------|
| Template syntax validation | On every parse | `check_template-system_contracts.sh` |
| All required parameters have defaults | On template registration | TemplateParser validation |
| No dependency cycles | On graph generation | Kahn's algorithm cycle detection |
| Template files exist on disk | Startup | TemplateRepository scanning |

### Manual Verification

```bash
# List all templates and verify counts
curl -X GET /api/v1/templates | jq '. | { total, templates: [.templates[].id] }'

# Validate a specific template
curl -X POST /api/v1/templates/validate \
  -H "Content-Type: application/json" \
  -d '{"toml_content": "...", "check_cycles": true}'
```

## Incident Response

### Detection

| Indicator | Source | Alert Severity |
|-----------|--------|----------------|
| Template parse errors in logs | Structured logging | WARNING |
| Template registration failures | Metric `templates.parse_errors` | WARNING |
| Graph generation failures | Metric `templates.generate_errors` | ERROR |
| Template file not found | Health check endpoint | CRITICAL |

### Escalation

| Severity | Response Time | Escalation Path |
|----------|---------------|-----------------|
| WARNING | < 1 hour | Template author fixes TOML syntax |
| ERROR | < 30 minutes | Developer restores from Git |
| CRITICAL | < 15 minutes | Operations restores entire module |

## Testing the DR Plan

### DR Test Scenarios

| Test | Frequency | Success Criteria |
|------|-----------|------------------|
| Single template file deletion | Monthly | Template restored from Git in < 5 minutes |
| All template files deleted | Quarterly | Templates restored and engine running in < 10 minutes |
| Built-in template corruption | Quarterly | Binary rebuilt and templates verified in < 30 minutes |
| Complete module recovery | Per release | All templates loaded and generation works |

### DR Test Script

```bash
#!/usr/bin/env bash
# DR Test: Simulate template file loss and recovery
set -euo pipefail

echo "=== DR Test: Template File Loss ==="

# Phase 1: Backup current templates
BACKUP=$(mktemp -d)
cp -r templates/* "$BACKUP/"
echo "✓ Backed up templates to $BACKUP"

# Phase 2: Delete templates
rm -f templates/*.toml
echo "✓ Deleted all template files"

# Phase 3: Verify engine handles missing files gracefully
cargo test --lib templates::application::template_parser_impl::tests::test_load_directory_empty
echo "✓ Empty directory handled gracefully"

# Phase 4: Restore from backup
cp "$BACKUP"/*.toml templates/
echo "✓ Templates restored from backup"

# Phase 5: Verify templates are valid
cargo test --lib templates::application::template_parser_impl::tests::test_parse_valid_toml
echo "✓ Templates valid after restore"

# Phase 6: Verify engine works with restored templates
cargo test --lib templates::application::template_engine_impl::tests
echo "✓ Engine works with restored templates"

rm -rf "$BACKUP"
echo "=== DR Test PASSED ==="
```

---
*Last updated: 2026-06-14*
