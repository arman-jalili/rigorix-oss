# Action Entrypoint Architecture

<!--
Canonical Reference: .pi/architecture/modules/action-entrypoint.md
Blueprint Source: Rigorix design session (2026-06-20)
Rationale: GitHub Actions entry point that maps workflow events to engine orchestrator calls
-->

## Overview

The Action Entrypoint is the main binary for the `rigorix-action` crate. It handles GitHub Action event routing — mapping workflow triggers (`workflow_dispatch`, `issue_comment`, `pull_request`) to engine orchestrator calls. All business logic lives in `rigorix-engine`; this module is a thin dispatch layer.

## Philosophy

The action crate **does not rebuild engine functionality**. It is a presentation adapter: GitHub Action inputs → engine calls → GitHub Action outputs. Every engine module (planning, execution, validation, hooks, permissions) is reused as-is.

## Responsibilities

- Parse GitHub Action event context (event name, payload, inputs)
- Route events to the appropriate engine entry point (run, plan, validate, status)
- Determine execution mode from inputs (plan-only, run, validate-loop)
- Pass workspace root and configuration to engine services
- Handle GitHub Action lifecycle (set output, set failed, post annotations)
- Support both `workflow_dispatch` and `issue_comment` triggers

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| ActionMain | `actions/src/action_entrypoint/main.rs` | Binary entry point — parses env, dispatches to engine | #main |
| ActionRouter | `actions/src/action_entrypoint/router.rs` | Maps event type + inputs → engine call | #router |
| ActionContext | `actions/src/action_entrypoint/context.rs` | GitHub Action context (env, event, inputs) | #context |
| ActionMode | `actions/src/action_entrypoint/mode.rs` | Execution mode: Plan, Run, Validate, Status | #mode |
| ActionError | `actions/src/action_entrypoint/error.rs` | Typed error → GitHub Action annotations + exit codes | #error |

---

## Component Details

### ActionRouter

**Purpose:** Map GitHub Action event types to engine service calls

```rust
/// Routes GitHub Action events to engine orchestrator calls.
///
/// The router is stateless — all state lives in the engine.
pub struct ActionRouter {
    orchestrator: Arc<dyn OrchestratorService>,
    validation_loop: Option<Arc<dyn ValidationLoopService>>,
}

impl ActionRouter {
    /// Dispatch based on event type and action mode.
    pub async fn dispatch(&self, ctx: &ActionContext) -> Result<ActionOutput, ActionError> {
        match &ctx.mode {
            ActionMode::Run { intent } => {
                let input = RunInput {
                    intent: intent.clone(),
                    repo_root: ctx.workspace_root.clone(),
                    config: ctx.to_engine_config(),
                    enforcement_preset: None,
                };
                let output = self.orchestrator.run(input).await?;
                Ok(ActionOutput::from_run(output))
            }
            ActionMode::Plan { intent } => {
                let input = PlanOnlyInput {
                    intent: intent.clone(),
                    repo_root: ctx.workspace_root.clone(),
                    config: ctx.to_engine_config(),
                };
                let output = self.orchestrator.plan_only(input).await?;
                Ok(ActionOutput::from_plan(output))
            }
            ActionMode::Validate { intent } => {
                // Uses validation loop if available; falls back to direct run
                if let Some(ref svc) = self.validation_loop {
                    let input = ValidateInput {
                        intent: crate::planning::domain::intent::UserIntent::new(
                            intent.clone(), None,
                        ),
                        execution_id: None,
                        config: ValidationLoopConfig {
                            max_iterations: ctx.inputs.max_validation_iterations.unwrap_or(3),
                            ..ValidationLoopConfig::default()
                        },
                        existing_template: None,
                    };
                    let output = svc.validate(input).await?;
                    Ok(ActionOutput::from_validation(output))
                } else {
                    // Fallback: run without validation loop
                    self.dispatch(&ctx.with_mode(ActionMode::Run { intent: intent.clone() })).await
                }
            }
            ActionMode::Status => {
                let output = self.orchestrator.status().await?;
                Ok(ActionOutput::from_status(output))
            }
        }
    }
}
```

### ActionContext

**Purpose:** GitHub Actions environment parsed into a typed context

```rust
/// Typed representation of the GitHub Action execution context.
///
/// Parsed from environment variables set by GitHub Actions:
/// - GITHUB_WORKSPACE — workspace root
/// - GITHUB_EVENT_NAME — trigger event type
/// - GITHUB_EVENT_PATH — path to event payload JSON
/// - INPUT_* — workflow inputs from action.yml
pub struct ActionContext {
    /// Absolute path to the repository workspace.
    pub workspace_root: String,

    /// The event that triggered this workflow.
    pub event: GitHubEvent,

    /// Action inputs parsed from environment variables.
    pub inputs: ActionInputs,

    /// The resolved execution mode.
    pub mode: ActionMode,

    /// GitHub token for API calls (PR comments, status checks).
    pub github_token: Option<String>,
}

/// Supported GitHub event types.
pub enum GitHubEvent {
    WorkflowDispatch,
    IssueComment { issue_number: u64, comment_body: String },
    PullRequest { pr_number: u64, action: String },
    Push { ref_name: String, commit_sha: String },
    Unknown(String),
}
```

### ActionMode

**Purpose:** Determines what the engine should do

```rust
pub enum ActionMode {
    /// Full lifecycle: plan → execute → persist → emit.
    Run { intent: String },
    /// Plan only: generate template without executing.
    Plan { intent: String },
    /// Run with validation loop (self-correcting, max 3 iterations).
    Validate { intent: String },
    /// Show current execution status.
    Status,
}

impl ActionContext {
    /// Convert action context into a JSON-compatible engine configuration value.
    ///
    /// Extracts engine-relevant fields (permission mode, budget limits, repo root)
    /// and serializes them as `serde_json::Value` for the engine's `RunInput.config` field.
    pub fn to_engine_config(&self) -> serde_json::Value {
        serde_json::json!({
            "repo_root": self.workspace_root,
            "permission_mode": self.inputs.permission_mode.clone().unwrap_or_else(|| "workspace_write".to_string()),
            "max_llm_calls": self.inputs.max_llm_calls,
            "max_llm_tokens": self.inputs.max_llm_tokens,
            "profile": self.inputs.profile,
        })
    }

    /// Return a new ActionContext with a different mode (used for fallback dispatch).
    fn with_mode(&self, mode: ActionMode) -> Self {
        Self { mode, ..self.clone() }
    }
}
```

---

## Data Flow

```
GitHub Action triggered (workflow_dispatch, issue_comment, etc.)
        │
        ▼
ActionContext parsed from:
  - GITHUB_WORKSPACE → workspace_root
  - GITHUB_EVENT_NAME → event type
  - GITHUB_EVENT_PATH → event payload JSON
  - INPUT_INTENT, INPUT_MODE → action mode + intent
        │
        ▼
ActionRouter::dispatch(ctx)
        │
        ├─ ActionMode::Run
        │     → OrchestratorService::run(RunInput { intent, repo_root, config })
        │     → RunOutput { execution_id, record }
        │
        ├─ ActionMode::Validate
        │     → ValidationLoopService::validate(ValidateInput { intent, config })
        │     → ValidateOutput { outcome, iterations, validated_template }
        │
        ├─ ActionMode::Plan
        │     → OrchestratorService::plan_only(PlanOnlyInput { ... })
        │     → PlanOnlyOutput { plan, template }
        │
        └─ ActionMode::Status
              → OrchestratorService::status()
              → StatusOutput { current_execution, history }
        │
        ▼
ActionOutput formatted as:
  - GitHub step summary (markdown)
  - Workflow annotations (::error file=...::message)
  - Output variables (echo "result=..." >> $GITHUB_OUTPUT)
  - PR comments (if token provided)
```

---

## Dependencies

### Depends On
- **rigorix-engine::orchestrator**: `OrchestratorService` for all execution entry points
- **rigorix-engine::plan_validation**: `ValidationLoopService` for self-correcting mode
- **action-input**: Parses GitHub context into typed inputs
- **action-output**: Formats engine results as GitHub Action outputs
- **action-config**: Loads action.yml configuration and workspace detection

### Used By
- Nothing (this is the top-level entry point)
- GitHub Actions workflow YAML (`uses: rigorix/action@v1`)

---

## Event Routing Table

| GitHub Event | ActionMode | Engine Call |
|-------------|-----------|-------------|
| `workflow_dispatch` with `mode: run` | Run | `OrchestratorService::run()` |
| `workflow_dispatch` with `mode: validate` | Validate | `ValidationLoopService::validate()` |
| `workflow_dispatch` with `mode: plan` | Plan | `OrchestratorService::plan_only()` |
| `issue_comment` containing `/rigorix run` | Run | `OrchestratorService::run()` |
| `issue_comment` containing `/rigorix validate` | Validate | `ValidationLoopService::validate()` |
| `pull_request` opened/synchronize | Validate | `ValidationLoopService::validate()` |
| `push` | Status | `OrchestratorService::status()` |

---

## Security Considerations

| Concern | Mitigation |
|---------|------------|
| GitHub token exposure | Token read from `secrets.GITHUB_TOKEN`, never logged |
| Arbitrary intent injection | Intent is user-provided; engine's permission enforcer gates tool execution |
| Workspace escape | Engine's workspace boundary check applies to all file operations |
| Malicious event payload | Event JSON parsed through serde; unknown events rejected |

---

## Related ADRs

- **Engine ADR-001** (`engine/.pi/architecture/decisions/ADR-001-architecture-pattern.md`): Clean Architecture pattern — actions are pure adapters
- **Engine ADR-004** (`engine/.pi/architecture/decisions/ADR-004-autonomy-presets.md`): `ActionMode::Validate` maps to autonomy preset
- **Engine ADR-007** (`engine/.pi/architecture/decisions/ADR-007-risk-gating-model.md`): Permission delegation to engine's `PermissionEnforcer`
- **Actions ADR-101** (`actions/.pi/architecture/decisions/ADR-101-actions-as-thin-adapter.md`): Actions crate must not rebuild engine logic
- **Actions ADR-102** (`actions/.pi/architecture/decisions/ADR-102-github-event-routing.md`): Stateless event routing table

---

*Last updated: 2026-06-20*
*Module version: 1.0.0 (Planned)*

---

**Status:** Planned
**Engine modules reused:** orchestrator, plan_validation, permission, hooks, quality_gates
