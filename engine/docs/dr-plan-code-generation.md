# Disaster Recovery Plan: code-generation Module

<!--
Canonical Reference: .pi/architecture/modules/code-generation.md
Last Updated: 2026-06-19
-->

## Scope

This DR plan covers the `code-generation` module — the subsystem that converts LLM-generated code into correctly-positioned file edits. It is stateless: configuration is loaded at startup and each edit is an independent filesystem operation.

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 30 seconds | Module is stateless — re-initialize tools with config |
| RPO (Recovery Point Objective) | N/A (stateless) | No internal state to recover; file edits are persisted on disk by the filesystem |

## Backup Strategy

### No Internal State to Back Up

The code-generation module is **stateless**:
- `SyntaxGateImpl` holds only configuration (loaded at startup)
- `EditFileTool` and `FileReadTool` hold only workspace_root
- No database, no queue, no persistent storage is managed by this module
- File edits are written directly to disk — filesystem backup covers them

### Configuration

| Data | Backup Strategy | Frequency | Retention |
|------|----------------|-----------|-----------|
| SyntaxGate config | Version control | On change | Full git history |
| Tool registration | Version control | On change | Full git history |

## Restore Procedure

### Scenario: Module Fails to Initialize

1. **Detect**: Engine logs error during tool registration or syntax gate creation
2. **Isolate**: Check tree-sitter parser availability (Cargo.toml deps)
3. **Restore**: 
   ```bash
   # Verify parsers are available
   cargo tree | grep tree-sitter
   
   # Rebuild if missing
   cargo build
   ```
4. **Verify**:
   ```bash
   # Run syntax gate test
   bash engine/.pi/scripts/ci/check_code-generation_contracts.sh
   ```

### Scenario: EditFileTool Corrupts a File

1. **Detect**: User reports incorrect file content
2. **Isolate**: Check `edit_file` audit trail (session logs)
3. **Restore**: 
   ```bash
   # Restore from version control
   git checkout HEAD -- path/to/corrupted/file
   ```
4. **Verify**: Re-run the edit with correct old_string

### Scenario: Syntax Gate Not Available

1. **Detect**: Syntax gate returns `Skipped` for all languages
2. **Isolate**: Check tree-sitter parser initialization
3. **Restore**: Rebuild with `cargo build`
4. **Verify**: 
   ```rust
   let gate = SyntaxGateImpl::new(SyntaxGateConfig::default());
   let result = gate.verify(SyntaxGateInput {
       file_path: "test.rs".into(),
       content: "fn main() {}".into(),
   });
   ```

## Failover Plan

### Single Instance (Default Deployment)

The code-generation module runs in-process within the engine. Failover follows the engine's failover plan:

1. Engine process fails → OS restarts (systemd/supervisor)
2. Engine re-initializes → tools are re-registered, syntax gate re-created
3. File edits are preserved on disk (atomic write-rename prevents partial writes)

### High-Availability Deployment

Not currently supported — tools run in-process and file edits go to local filesystem.
If HA is required:

1. Deploy engine behind a load balancer (multiple replicas)
2. Each replica registers the same tools
3. File edits must go to shared filesystem (NFS) or use network storage
4. Atomic write-rename pattern works on any POSIX filesystem

## Disaster Scenarios

### Scenario 1: EditFileTool Writes Wrong Content

| Aspect | Detail |
|--------|--------|
| **Impact** | File contains incorrect code; build may break |
| **Detection** | LLM self-verification via unified_diff; syntax gate detects errors |
| **Containment** | Edit is already applied — no automatic rollback |
| **Recovery** | LLM issues corrective edit in next turn; git checkout for manual fix |
| **Prevention** | Exact-string matching prevents hallucinated positions; diff enables LLM verification |

### Scenario 2: File System Full During Edit

| Aspect | Detail |
|--------|--------|
| **Impact** | Write fails; file may be in inconsistent state |
| **Detection** | `fs::write` or `fs::rename` returns IO error |
| **Recovery** | Engine reports `ToolError::ExecutionFailed`; original file is preserved (atomic write writes to .tmp first) |
| **Prevention** | Set disk usage alerts; configure file size limits |

### Scenario 3: Race Condition (Concurrent Edits to Same File)

| Aspect | Detail |
|--------|--------|
| **Impact** | One edit may overwrite another |
| **Detection** | old_string may not exist if another edit already changed it |
| **Recovery** | LLM receives `OldStringNotFound` and re-reads the file |
| **Prevention** | LLM-driven execution is sequential by nature; no concurrent edits to same file in practice |

## Testing DR Readiness

| Test | Frequency | Procedure |
|------|-----------|-----------|
| EditFileTool not found | Quarterly | Remove old_string from file → verify `OldStringNotFound` |
| Identity rejection | Quarterly | Set old_string == new_string → verify `IdentityEdit` |
| Binary file rejection | Quarterly | Create file with NUL byte → verify `BinaryFile` |
| Path escape rejection | Quarterly | Use `../` path → verify `PathDenied` |
| Syntax gate validation | Quarterly | Edit valid Rust to invalid → verify `Failed` result |
| Syntax gate skip | Quarterly | Edit Markdown file → verify `Skipped` result |

## DR Kit Contents

| Item | Location | Purpose |
|------|----------|---------|
| Tool system architecture docs | `.pi/architecture/modules/code-generation.md` | System understanding |
| Runbook | `docs/runbook-code-generation.md` | Operations guide |
| Contract check scripts | `.pi/scripts/ci/check_code-generation_contracts.sh` | Automated verification |
| Coverage check script | `.pi/scripts/ci/check_code-generation_coverage.sh` | Coverage enforcement |

## Related Documents

- [Runbook: code-generation](runbook-code-generation.md)
- [Architecture: code-generation](../.pi/architecture/modules/code-generation.md)
- [Tool System Architecture](../.pi/architecture/modules/tool-system.md)
