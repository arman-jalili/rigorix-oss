# Tool System Architecture

<!--
Canonical Reference: .pi/architecture/modules/tool-system.md
Blueprint Source: Domain Exploration Session 63c25384
-->

## Overview

Defines the execution primitives for the task graph. Provides the Tool trait, shared types (ToolInput, ToolResult, ToolError), the ToolRegistry, and execute_with_risk_gate helper that enforces RiskLevel policies.

## Responsibilities

- Define the Tool trait with typed execute interface
- Implement concrete tools: FileRead, FileWrite, FileAppend, FilePatch, RunCommand, LspQuery, GitRead, GitStage, GitCommit
- Maintain ToolRegistry with lookup by name
- Enforce risk gates before tool execution
- Validate tool inputs and file paths (no writes outside workspace)
- Support atomic write-rename pattern for file writes
- Emit ToolExecuted events for observability

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| Tool (trait) | `rigorix/src/tools/tool_trait.rs` | Core trait for all tools | #trait |
| ToolRegistry | `rigorix/src/tools/mod.rs` | Registry by tool name | #registry |
| ToolInput | `rigorix/src/tools/mod.rs` | JSON parameters input | #input |
| ToolResult | `rigorix/src/tools/mod.rs` | Output with text, exit_code, side_effects | #result |
| FileReadTool | `rigorix/src/tools/file_read.rs` | Read file contents | #file-read |
| FileWriteTool | `rigorix/src/tools/file_write.rs` | Write/overwrite with atomic rename | #file-write |
| FileAppendTool | `rigorix/src/tools/file_write.rs` | Append to existing files | #file-append |
| FilePatchTool | `rigorix/src/tools/file_patch.rs` | AST-aware file patching | #file-patch |
| RunCommandTool | `rigorix/src/tools/run_command.rs` | Execute shell commands | #run-cmd |
| LspQueryTool | `rigorix/src/tools/lsp_query.rs` | Query language server | #lsp |
| GitReadTool | `rigorix/src/tools/git_read.rs` | Read git log/diff | #git-read |
| GitStageTool | `rigorix/src/tools/git_stage.rs` | Stage files in git | #git-stage |
| GitCommitTool | `rigorix/src/tools/git_commit.rs` | Create git commits | #git-commit |

---

## Component Details

### Tool Trait

**Purpose:** Core abstraction for all tool implementations

**Implementation File:** `rigorix/src/tools/tool_trait.rs`

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    async fn execute(&self, input: &ToolInput) -> Result<ToolResult, ToolError>;
    fn name(&self) -> &str;
}
```

### ToolRegistry

**Purpose:** Registry of all available tools by name with lookup

**Implementation File:** `rigorix/src/tools/mod.rs`

```rust
pub struct ToolRegistry { /* tools: HashMap<String, Box<dyn Tool>> */ }

impl ToolRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, name: impl Into<String>, tool: Box<dyn Tool>);
    pub fn get(&self, name: &str) -> Option<&dyn Tool>;
    pub fn execute_with_risk_gate(&self, name: &str, input: &ToolInput, risk_config: &RiskConfig)
        -> Result<ToolResult, ToolError>;
}
```

---

## Risk Level Mapping

| Tool | Risk Level | Reason |
|------|-----------|--------|
| FileReadTool | Low | Read-only |
| LspQueryTool | Low | Read-only |
| GitReadTool | Low | Read-only |
| FileWriteTool | Medium | Modifies files |
| FileAppendTool | Medium | Modifies files |
| FilePatchTool | Medium | Modifies files |
| GitStageTool | Medium | Modifies git index |
| RunCommandTool | High | Arbitrary execution |
| GitCommitTool | High | Irreversible git action |

---

## Data Flow

```mermaid
flowchart TB
    EXEC["Execution Engine
requests tool execution"] --> REG["ToolRegistry.lookup(name)"]
    REG -->|found| GATE["execute_with_risk_gate
(name, input, risk_config)"]
    REG -->|not found| ERR["ToolError::NotFound"]
    
    GATE --> CHECK{"Risk level?"]
    CHECK -->|Low| RUN["Tool::execute(&input)"]
    CHECK -->|Medium| CONFIRM{"Confirmed?"]
    CONFIRM -->|yes| RUN
    CONFIRM -->|no| SKIP
    CHECK -->|High| SKIP["Dry-run: skipped
return empty"]
    
    RUN --> RESULT["ToolResult
{ output, exit_code, side_effects }"]
    RUN --> ERROR["ToolError
{ InvalidInput, ExecutionFailed,
PathDenied, RequiresConfirmation }"]
```

**Flow Description:**
1. Execution Engine requests tool by name from ToolRegistry
2. execute_with_risk_gate enforces RiskLevel policy before execution
3. FileWriteTool validates all paths against repo_root
4. RunCommandTool checks the command for shell control operators, then against the allowlist
5. Tools return ToolResult with output text, exit code, and side effects for audit

<details>
<summary>Command execution environment</summary>

Both shell-executing routes (`RunCommandTool` in the registry, and
`ParallelExecutionServiceImpl::exec_run_command` on the dispatch path) call the
same `tools::application::shell_policy` helpers:

- `find_shell_control_operator` — refuses `;` `&&` `||` `|` `&` backticks `$(`
  `<` `>` and newlines. A prefix match is not evidence that the rest of the
  string is inert, so the compound form is rejected rather than split-and-checked.
- `command_from_intent` — the bash classifier and the executor extract the
  command the same way, so a JSON intent cannot hide a command from the
  classifier.
- `apply_child_env` — `env_clear()` plus an explicit allowlist
  (`PATH`, `HOME`, `LANG`, `LC_ALL`, `TMPDIR`, `SHELL`, `TERM`) and per-call
  overrides. This removes the engine's secret material (`RIGORIX_HMAC_KEY`,
  anchor credentials) from the child's environment. It is **not** a sandbox: the
  child still runs as the engine's user with the engine's filesystem access.

</details>

## Dependencies

### Depends On
- **Risk Gating**: RiskLevel, RiskConfig for gating decisions
- **Configuration**: Repo root path, path allowlists

### Used By
- **Execution Engine**: ParallelExecutor resolves tool via registry
- **Orchestrator**: Registers tools during OrchestratorBuilder::build()

---

## Security Considerations

| Concern | Mitigation | Validator |
|---------|------------|-----------|
| Writing outside workspace | FileWriteTool validates all paths against repo_root | security-validator |
| Arbitrary command execution | RunCommandTool constrained by allowlist; High risk = dry-run default | security-validator |
| Compound shell string bypassing a prefix allowlist | A command containing `;` `&&` `||` `|` `&` backticks `$(` `<` `>` or a newline is refused before the prefix check; the classifier and the executor share one intent→command extraction | `shell_policy` tests |
| Child command reading engine secrets | `apply_child_env` clears the inherited environment; only `PATH HOME LANG LC_ALL TMPDIR SHELL TERM` plus explicit per-call overrides reach the child, so `RIGORIX_HMAC_KEY` is not handed to executed commands | `test_child_does_not_inherit_the_engine_environment` |
| Git commit with wrong message | GitCommitTool requires Medium+ confirmation | security-validator |
| A **new** spawn site that skips the policy | `clippy.toml` `disallowed-types` denies `std::process::Command` and `tokio::process::Command` workspace-wide (`disallowed_types = "deny"` in `[workspace.lints.clippy]`); a site compiles only with a documented `#![allow]`, so it cannot be added silently | `cargo clippy --workspace --all-targets` |

### Process spawning sinks {#process-spawning-sinks}

Spawning is a privileged sink: it runs something outside the engine's type system
with whatever environment and arguments the caller assembled. Two `sh -c` sites
once existed side by side here — one with an allowlist and a control-operator
refusal, one with neither — and the unguarded one was in the real dispatch path
(#1067). Nothing declared spawning privileged, so the default for a new sink was
*ungoverned* and a reviewer had to notice by reading.

**The default is now inverted (#1083).** The lint fires for any new site; the
list below is the complete set that has opted in, and each entry says which policy
applies. Adding a row without adding the corresponding `#![allow]` reason (or the
reverse) is a review defect — the two are meant to move together.

| Sink | Policy |
|---|---|
| `tools/application/run_command_tool.rs` | governed shell: allowlist + control-operator refusal + cleared env |
| `tools/application/shell_policy.rs` | the policy itself (`apply_child_env` takes the child `Command` by reference) |
| `tools/application/git_stage_tool.rs`, `git_commit_tool.rs` | fixed argv, no caller string |
| `tools/application/git_read_tool.rs` | fixed argv `git` (no shell) + write/exec flag refusal (#1082, class C2) |
| `execution_engine/application/service_impl/tools.rs` | governed shell (`exec_run_command`) + fixed argv `git` |
| `precondition/infrastructure/runner.rs` | operator argv, never a shell; trust boundary + explicit env (ADR-017) |
| `hooks/application/hook_runner_impl.rs` | operator-configured hook command (shell); child env cleared to the allowlist (#1082, class C3) |
| `approval/infrastructure/effect_scope.rs`, `orchestrator/.../helpers.rs` | fixed argv `git` |
| `scored_evaluation/.../local_backend.rs` | operator-configured scorer script, fixed argv |
| `cli/cli_boundary/dispatch.rs`, `mcp/.../engine_facade_impl.rs` | fixed argv `git remote/config` |
| `cli/tui/mod.rs` | fixed argv clipboard helper |
| `actions/.../router_impl.rs` | fixed argv `git` |
| test-only: `execution_engine/approval_binding_tests.rs`, `cli/tests/projection_surface.rs`, `mcp/tests/{stdio_integration,e2e_execute_to_audit,e2e_enterprise_proxy}_test.rs` | runs the binary under test |

The lint confines *where* spawning happens, not *what* the command does with
caller input. Those are separate properties and both are now enforced: the two
former gaps (`git_read_tool` interpolating a caller path into `sh -c`, and the
hook child inheriting the engine environment) were closed in #1082, and
`engine/tests/guard_suite.rs` fails if a new `sh -c` site appears without the
policy that validates its string.

---

## Testing Requirements

| Test Type | Coverage Target | Files |
|-----------|-----------------|-------|
| Unit | 90% | Per-tool test modules |

**Key Test Scenarios:**
- FileReadTool reads existing file → Ok(content)
- FileWriteTool writes new file → file created with content
- FileWriteTool path outside workspace → ToolError
- RunCommandTool with allowlisted command → Ok
- RunCommandTool with non-allowlisted command → ToolError

---

## Error Handling

```rust
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Tool not found: {0}")]
    NotFound(String),
    #[error("Path denied: {0}")]
    PathDenied(String),
    #[error("Requires confirmation")]
    RequiresConfirmation,
}
```

---

Last updated: 2026-06-15
*Module version: 1.0.0*

---

**Status:** Implemented  
**Last verified:** 2026-06-15  
**Module version:** 1.0.0
