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
4. RunCommandTool checks command against allowlist
5. Tools return ToolResult with output text, exit code, and side effects for audit

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
| Git commit with wrong message | GitCommitTool requires Medium+ confirmation | security-validator |

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
