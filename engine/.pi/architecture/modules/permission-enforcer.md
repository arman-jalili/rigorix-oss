# Permission Enforcement Architecture

<!--
Canonical Reference: .pi/architecture/modules/permission-enforcer.md
Blueprint Source: claw-code-parity analysis (2026-06-19)
Rationale: Three-tier permission mode hierarchy with workspace-boundary awareness and bash command classification
-->

## Overview

The Permission Enforcer provides a three-tier permission mode hierarchy (`ReadOnly` → `WorkspaceWrite` → `DangerFullAccess`) that gates every tool invocation. It extends Rigorix's existing `RiskLevel` system with mode-aware gating: the active permission mode caps the maximum risk level a tool can execute. Tools requesting a higher `required_permission` than the active mode are denied with a structured reason before execution.

## Adoption Rationale

Rigorix currently has `RiskLevel` (Low/Medium/High) per tool and `GatingAction` (Auto, Confirm, DryRun). The Permission Enforcer adds:

- **Three-tier mode hierarchy**: a simple, intuitive restriction model that users understand
- **Mode caps risk**: in `ReadOnly` mode, only `RiskLevel::Low` tools execute; `Medium` and `High` are denied
- **Workspace boundary enforcement**: file writes outside the repo root are denied even in `WorkspaceWrite` mode
- **Bash command classification**: read-only commands (ls, cat, grep) auto-allow; destructive commands (rm, shred) auto-deny
- **Structured deny reasons**: every denial includes `tool`, `active_mode`, `required_mode`, and a human-readable `reason` — fed back to the LLM
- **Prompt mode**: interactive confirmation flow for tools requiring human approval

## Responsibilities

- Define three-tier `PermissionMode`: ReadOnly, WorkspaceWrite, DangerFullAccess
- Gating: deny tools whose `required_permission` exceeds the active mode
- Workspace boundary check: deny file writes outside workspace root
- Bash command classification: allow read-only commands in ReadOnly mode, deny mutating ones
- Structured deny reasons for LLM feedback
- Interactive prompt mode for tool confirmation
- Integration with risk gating for per-tool risk level awareness

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| PermissionMode | `engine/src/permission/domain/mode.rs` | Enum: ReadOnly, WorkspaceWrite, DangerFullAccess | #mode |
| PermissionPolicy | `engine/src/permission/domain/policy.rs` | Active mode + authorization logic | #policy |
| PermissionOutcome | `engine/src/permission/domain/outcome.rs` | Allow or Deny { reason } | #outcome |
| PermissionContext | `engine/src/permission/domain/context.rs` | Override context from hooks or user | #context |
| PermissionEnforcer | `engine/src/permission/application/enforcer.rs` | Mode check, file write check, bash command check | #enforcer |
| PermissionPrompter | `engine/src/permission/domain/prompter.rs` | Trait for interactive confirmation | #prompter |
| BashClassifier | `engine/src/permission/domain/bash_classifier.rs` | Classifies bash commands as read-only, write, or destructive | #bash-classifier |
| PermissionConfig | `engine/src/permission/domain/config.rs` | Config: default mode, allow/deny/ask rules | #config |
| PermissionError | `engine/src/permission/domain/error.rs` | Typed error enum | #error |

---

## Component Details

### PermissionMode

**Purpose:** Three-tier hierarchy controlling tool execution scope

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// Only read operations: file_read, grep, glob, git_log, lsp_query
    /// Bash: only read-only commands (ls, cat, grep, find, etc.)
    ReadOnly = 0,
    /// Read + write within workspace boundary
    /// Bash: all commands allowed
    WorkspaceWrite = 1,
    /// No restrictions — full system access
    DangerousFullAccess = 2,
}
```

**Mode capability matrix:**

| Operation | ReadOnly | WorkspaceWrite | DangerousFullAccess |
|-----------|----------|----------------|---------------------|
| read_file | ✅ | ✅ | ✅ |
| grep_search | ✅ | ✅ | ✅ |
| lsp_query | ✅ | ✅ | ✅ |
| write_file (in workspace) | ❌ | ✅ | ✅ |
| write_file (outside workspace) | ❌ | ❌ | ✅ |
| edit_file (in workspace) | ❌ | ✅ | ✅ |
| bash: `ls`, `cat`, `grep` | ✅ | ✅ | ✅ |
| bash: `cargo build` | ❌ | ✅ | ✅ |
| bash: `rm -rf /` | ❌ | ❌ | ✅ |
| git_commit | ❌ | ✅ | ✅ |
| git_push | ❌ | ❌ | ✅ |

### PermissionPolicy

**Purpose:** Authorization decisions based on active mode and tool requirements

```rust
pub struct PermissionPolicy {
    active_mode: PermissionMode,
    tool_permissions: HashMap<String, PermissionMode>,
    allow_rules: Vec<String>,
    deny_rules: Vec<String>,
    ask_rules: Vec<String>,
}

impl PermissionPolicy {
    pub fn authorize(
        &self,
        tool_name: &str,
        tool_input: &str,
        prompter: Option<&mut dyn PermissionPrompter>,
    ) -> PermissionOutcome {
        // 1. Check explicit deny rules
        if self.matches_any_rule(tool_name, &self.deny_rules) {
            return PermissionOutcome::Deny {
                reason: format!("'{tool_name}' is explicitly denied by config"),
            };
        }

        // 2. Check allow rules (override mode)
        if self.matches_any_rule(tool_name, &self.allow_rules) {
            return PermissionOutcome::Allow;
        }

        // 3. Get required mode for this tool
        let required = self.required_mode_for(tool_name);

        // 4. Mode check
        if self.active_mode < required {
            return PermissionOutcome::Deny {
                reason: format!(
                    "'{tool_name}' requires '{required}' mode, but active mode is '{}'",
                    self.active_mode.as_str()
                ),
            };
        }

        // 5. Ask rules (prompt for confirmation)
        if self.matches_any_rule(tool_name, &self.ask_rules) {
            if let Some(prompter) = prompter {
                return prompter.prompt(tool_name, tool_input);
            }
            return PermissionOutcome::Deny {
                reason: format!("'{tool_name}' requires confirmation but no prompter available"),
            };
        }

        PermissionOutcome::Allow
    }

    pub fn required_mode_for(&self, tool_name: &str) -> PermissionMode {
        self.tool_permissions
            .get(tool_name)
            .copied()
            .unwrap_or(PermissionMode::WorkspaceWrite) // safe default
    }
}
```

### PermissionEnforcer

**Purpose:** Three specific checks: general tool gating, file write boundaries, bash command classification

```rust
pub struct PermissionEnforcer {
    policy: PermissionPolicy,
}

impl PermissionEnforcer {
    /// General tool permission check
    pub fn check(&self, tool_name: &str, input: &str) -> EnforcementResult;

    /// Workspace boundary check for file writes
    pub fn check_file_write(&self, path: &str, workspace_root: &str) -> EnforcementResult {
        match self.policy.active_mode() {
            PermissionMode::ReadOnly => EnforcementResult::Denied {
                tool: "write_file".into(),
                active_mode: "read_only".into(),
                required_mode: "workspace_write".into(),
                reason: "file writes are not allowed in read_only mode".into(),
            },
            PermissionMode::WorkspaceWrite => {
                if is_within_workspace(path, workspace_root) {
                    EnforcementResult::Allowed
                } else {
                    EnforcementResult::Denied {
                        tool: "write_file".into(),
                        active_mode: "workspace_write".into(),
                        required_mode: "danger_full_access".into(),
                        reason: format!("path '{path}' is outside workspace root"),
                    }
                }
            }
            PermissionMode::DangerousFullAccess => EnforcementResult::Allowed,
        }
    }

    /// Bash command classification check
    pub fn check_bash(&self, command: &str) -> EnforcementResult {
        match self.policy.active_mode() {
            PermissionMode::ReadOnly => {
                let classification = BashClassifier::classify(command);
                if classification.is_read_only() {
                    EnforcementResult::Allowed
                } else {
                    EnforcementResult::Denied {
                        tool: "bash".into(),
                        active_mode: "read_only".into(),
                        required_mode: "workspace_write".into(),
                        reason: format!(
                            "'{command}' is classified as '{classification}' — not allowed in read_only mode"
                        ),
                    }
                }
            }
            _ => EnforcementResult::Allowed,
        }
    }
}
```

### BashClassifier

**Purpose:** Classifies bash commands into intent categories for mode-aware gating

```rust
pub struct BashClassifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandIntent {
    ReadOnly,          // ls, cat, grep, find, head, tail, wc, ...
    Write,             // cp, mv, mkdir, touch, tee, ...
    Destructive,       // rm, shred, truncate, dd, ...
    Network,           // curl, wget, ssh, ...
    ProcessManagement, // kill, pkill, ...
    PackageManagement, // apt, brew, pip, npm, cargo install, ...
    SystemAdmin,       // sudo, chmod, chown, mount, ...
    Unknown,
}

impl BashClassifier {
    pub fn classify(command: &str) -> CommandIntent {
        let base = extract_base_command(command);
        // Check against known command lists
        if READ_ONLY_COMMANDS.contains(&base) { return CommandIntent::ReadOnly; }
        if DESTRUCTIVE_COMMANDS.contains(&base) { return CommandIntent::Destructive; }
        if PACKAGE_MANAGERS.contains(&base) { return CommandIntent::PackageManagement; }
        if SYSTEM_ADMIN_COMMANDS.contains(&base) { return CommandIntent::SystemAdmin; }
        if WRITE_COMMANDS.contains(&base) { return CommandIntent::Write; }
        if NETWORK_COMMANDS.contains(&base) { return CommandIntent::Network; }
        if PROCESS_COMMANDS.contains(&base) { return CommandIntent::ProcessManagement; }
        CommandIntent::Unknown
    }
}
```

**Classification tables:**

| Category | Command Intent | Examples |
|----------|---------------|---------|
| READ_ONLY | `CommandIntent::ReadOnly` | `ls`, `cat`, `grep`, `find`, `head`, `tail`, `wc`, `sort`, `uniq`, `diff`, `file`, `stat`, `du`, `df`, `ps`, `top`, `who`, `env`, `echo`, `printf`, `which`, `whereis`, `type`, `git log`, `git status`, `git diff`, `git show`, `cargo check`, `cargo doc` |
| WRITE | `CommandIntent::Write` | `cp`, `mv`, `mkdir`, `rmdir`, `touch`, `ln`, `tee`, `install` |
| DESTRUCTIVE | `CommandIntent::Destructive` | `rm`, `shred`, `truncate`, `mkfifo`, `mknod`, `dd`, `>`, `>>` (redirect), `git reset --hard` |
| PACKAGE_MANAGERS | `CommandIntent::PackageManagement` | `apt`, `apt-get`, `brew`, `pip`, `pip3`, `npm`, `yarn`, `pnpm`, `cargo install`, `gem`, `rustup` |
| SYSTEM_ADMIN | `CommandIntent::SystemAdmin` | `sudo`, `chmod`, `chown`, `chgrp`, `mount`, `umount`, `systemctl`, `service`, `docker`, `kill`, `pkill` |
| NETWORK | `CommandIntent::Network` | `curl`, `wget`, `ssh`, `scp`, `rsync`, `nc`, `ping`, `traceroute` |
| PROCESS | `CommandIntent::ProcessManagement` | `kill`, `pkill`, `killall`, `nice`, `renice`, `bg`, `fg`, `jobs` |

### EnforcementResult

**Purpose:** Structured outcome of permission enforcement

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome")]
pub enum EnforcementResult {
    Allowed,
    Denied {
        tool: String,
        active_mode: String,
        required_mode: String,
        reason: String,
    },
}
```

---

## Data Flow

```
Tool invocation requested
        │
        ▼
PermissionEnforcer::check(tool_name, input)
        │
        ▼
PermissionPolicy::authorize(tool_name, input, prompter?)
        │
  ┌─────┴─────┐
  │ 1. Deny rules? ── yes ──→ Deny (explicitly denied)
  │ 2. Allow rules? ── yes ──→ Allow (override)
  │ 3. required_mode_for(tool_name)
  │ 4. active_mode < required? ── yes ──→ Deny (insufficient mode)
  │ 5. Ask rules? ── yes ──→ prompt → Allow or Deny
  │ 6. Otherwise → Allow
  └───────────┘
        │
        ▼
If tool is write_file or edit_file:
  → PermissionEnforcer::check_file_write(path, workspace_root)
        │
        ▼
If tool is bash:
  → PermissionEnforcer::check_bash(command)
  → BashClassifier::classify(command)
  → ReadOnly mode + non-read-only command → Deny
        │
        ▼
EnforcementResult::Allowed → execute tool
EnforcementResult::Denied → return denial to LLM as ToolError
```

**Flow Description:**
1. Every tool invocation passes through `PermissionEnforcer::check()`
2. The `PermissionPolicy` evaluates explicit deny/allow rules, then mode requirement
3. File writes get an additional workspace boundary check
4. Bash commands get classified by `BashClassifier` and gated by intent
5. Denials return structured reasons to the LLM so it can adapt (e.g., "I can't run that command in read-only mode, but I can run `git log` instead")

---

## Dependencies

### Depends On
- **Risk Gating**: `RiskLevel` per tool maps to `required_permission`
- **Configuration**: Permission mode and allow/deny/ask rules from `.rigorix/permissions.toml`
- **Event System**: Permission events for audit trail

### Used By
- **Execution Engine**: Gates every tool invocation
- **Orchestrator**: Sets active permission mode per session
- **TUI**: Displays permission prompts and mode status

---

## Configuration

```toml
# .rigorix/permissions.toml
[permissions]
# Default mode: read_only, workspace_write, dangerous_full_access
default_mode = "workspace_write"

# Always allow these tools (override mode)
allow = [
    "read_file",
    "grep_search",
]

# Always deny these tools
deny = [
    "git_push",
]

# Ask for confirmation before these tools
ask = [
    "git_commit",
    "run_command",
]
```

---

## Integration with CLI

```bash
# Start rigorix in read-only mode
rigorix run "audit the codebase" --permission-mode read-only

# Skip all permission checks (dangerous)
rigorix run "deploy to production" --dangerously-skip-permissions

# Set per-session mode
rigorix tui --permission-mode workspace-write
```

---

## Security Considerations

| Concern | Mitigation | Validator |
|---------|------------|-----------|
| Symlink escape from workspace | Path canonicalization before boundary check | security-validator |
| Bash command injection bypassing classification | Extract base command before pipes/redirects; classify only the first command | security-validator |
| `--dangerously-skip-permissions` abuse | Flag requires explicit user opt-in; warning displayed; audit event emitted | security-validator |
| Allow rules overriding deny rules | Allow rules checked before deny rules; deny rules are authoritative | security-validator |

---

## Testing Requirements

| Test Type | Coverage Target | Files |
|-----------|-----------------|-------|
| Unit | 95% | `engine/src/permission/` — per-component test modules |

**Key Test Scenarios:**
- `ReadOnly` mode denies `write_file` → `EnforcementResult::Denied`
- `WorkspaceWrite` mode allows `write_file` inside workspace → `Allowed`
- `WorkspaceWrite` mode denies `write_file` outside workspace → `Denied`
- `bash: ls` in `ReadOnly` mode → `Allowed`
- `bash: rm -rf` in `ReadOnly` mode → `Denied` (destructive)
- `bash: cargo build` in `ReadOnly` mode → `Denied` (package manager)
- Explicit allow rule overrides mode denial → `Allowed`
- Explicit deny rule overrides everything → `Denied`

---

*Last updated: 2026-06-19*
*Module version: 1.0.0 (Planned)*
*Adopted from: claw-code-parity analysis — permission_enforcer.rs (546 LOC), permissions.rs, bash_validation.rs (1004 LOC)*

---

**Status:** Planned  
**Blueprint Source:** claw-code-parity pattern analysis  
**Implementation priority:** P1 — extends risk gating with mode hierarchy and bash classification
