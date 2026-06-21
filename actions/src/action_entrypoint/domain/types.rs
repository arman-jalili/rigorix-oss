//! Domain types for the Action Entrypoint bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md#types
//! Implements: Contract Freeze — ActionContext, ActionMode, ActionOutput, GitHubEvent
//! Issue: issue-contract-freeze
//!
//! These are the core domain types that represent the GitHub Action execution
//! context, execution mode, routing output, and GitHub event types. They serve
//! as the frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces
//! - All types are serializable (Serialize + Deserialize) where applicable

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ActionMode
// ---------------------------------------------------------------------------

/// The execution mode determining what the engine should do.
///
/// This is distinct from `security_config::domain::ActionMode` which controls
/// GitHub token permission scopes. This enum describes the action's execution
/// intent passed to the engine orchestrator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionMode {
    /// Full lifecycle: plan → execute → persist → emit.
    Run {
        /// Natural-language intent describing the work to be done.
        intent: String,
    },

    /// Plan only: generate template without executing.
    Plan {
        /// Natural-language intent for planning.
        intent: String,
    },

    /// Run with validation loop (self-correcting, max `max_iterations`).
    Validate {
        /// Natural-language intent for validation.
        intent: String,
    },

    /// Show current execution status.
    Status,
}

impl ActionMode {
    /// Whether this mode requires a user intent string.
    pub fn requires_intent(&self) -> bool {
        matches!(
            self,
            ActionMode::Run { .. } | ActionMode::Plan { .. } | ActionMode::Validate { .. }
        )
    }

    /// Get the intent string if this mode carries one.
    pub fn intent(&self) -> Option<&str> {
        match self {
            ActionMode::Run { intent }
            | ActionMode::Plan { intent }
            | ActionMode::Validate { intent } => Some(intent.as_str()),
            ActionMode::Status => None,
        }
    }

    /// Human-readable mode name for logging and event metadata.
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionMode::Run { .. } => "run",
            ActionMode::Plan { .. } => "plan",
            ActionMode::Validate { .. } => "validate",
            ActionMode::Status => "status",
        }
    }
}

// ---------------------------------------------------------------------------
// GitHubEvent
// ---------------------------------------------------------------------------

/// GitHub event types that the action entrypoint can route.
///
/// Parsed from `GITHUB_EVENT_NAME` and `GITHUB_EVENT_PATH` by the
/// `ActionContext` builder. Represents the trigger that started
/// the workflow execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum GitHubEvent {
    /// Triggered by `workflow_dispatch` — manual trigger via UI or API.
    WorkflowDispatch {
        /// Branch/tag/ref that was dispatched.
        ref_name: String,
    },

    /// Triggered by `issue_comment` on an issue or PR.
    IssueComment {
        /// The issue or PR number.
        issue_number: u64,
        /// The raw comment body text.
        comment_body: String,
        /// The username that posted the comment.
        commenter: String,
    },

    /// Triggered by `pull_request` events.
    PullRequest {
        /// The PR number.
        pr_number: u64,
        /// The PR action (opened, synchronize, closed, labeled, etc.).
        action: String,
        /// PR title.
        title: String,
        /// Base branch name.
        base_branch: String,
        /// Head branch name.
        head_branch: String,
        /// Head commit SHA.
        head_sha: String,
    },

    /// Triggered by `push` events.
    Push {
        /// Branch name.
        branch: String,
        /// Commit SHA.
        sha: String,
        /// Pusher username.
        pusher: String,
    },

    /// Unknown or unsupported event type.
    Unknown {
        /// The raw event name from `GITHUB_EVENT_NAME`.
        event_name: String,
    },
}

impl GitHubEvent {
    /// Human-readable event type name.
    pub fn event_type(&self) -> &'static str {
        match self {
            GitHubEvent::WorkflowDispatch { .. } => "workflow_dispatch",
            GitHubEvent::IssueComment { .. } => "issue_comment",
            GitHubEvent::PullRequest { .. } => "pull_request",
            GitHubEvent::Push { .. } => "push",
            GitHubEvent::Unknown { .. } => "unknown",
        }
    }

    /// Whether this event type supports routing to engine execution.
    pub fn is_routable(&self) -> bool {
        matches!(
            self,
            GitHubEvent::WorkflowDispatch { .. }
                | GitHubEvent::IssueComment { .. }
                | GitHubEvent::PullRequest { .. }
        )
    }
}

// ---------------------------------------------------------------------------
// ActionContext
// ---------------------------------------------------------------------------

/// Typed representation of the GitHub Action execution context.
///
/// Parsed from environment variables set by GitHub Actions:
/// - `GITHUB_WORKSPACE` — workspace root
/// - `GITHUB_EVENT_NAME` — trigger event type
/// - `GITHUB_EVENT_PATH` — path to event payload JSON
/// - `INPUT_*` — workflow inputs from action.yml
///
/// This is the primary input to `ActionRouter::dispatch()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionContext {
    /// Absolute path to the repository workspace.
    pub workspace_root: String,

    /// The event that triggered this workflow.
    pub event: GitHubEvent,

    /// The resolved execution mode.
    pub mode: ActionMode,

    /// GitHub token for API calls (PR comments, status checks).
    pub github_token: Option<String>,

    /// Maximum validation loop iterations (default: 3).
    pub max_validation_iterations: u32,

    /// Maximum LLM API calls per execution.
    pub max_llm_calls: Option<u32>,

    /// Maximum LLM tokens per execution.
    pub max_llm_tokens: Option<u64>,

    /// Configuration profile to use.
    pub profile: Option<String>,

    /// Permission mode for engine tool execution.
    pub permission_mode: Option<String>,
}

impl ActionContext {
    /// Create a new ActionContext with all required fields.
    pub fn new(
        workspace_root: String,
        event: GitHubEvent,
        mode: ActionMode,
        github_token: Option<String>,
    ) -> Self {
        Self {
            workspace_root,
            event,
            mode,
            github_token,
            max_validation_iterations: 3,
            max_llm_calls: None,
            max_llm_tokens: None,
            profile: None,
            permission_mode: None,
        }
    }

    /// Convert action context into a JSON-compatible engine configuration value.
    ///
    /// Extracts engine-relevant fields (permission mode, budget limits, repo root)
    /// and serializes them as `serde_json::Value` for the engine's `RunInput.config` field.
    pub fn to_engine_config(&self) -> serde_json::Value {
        serde_json::json!({
            "repo_root": self.workspace_root,
            "permission_mode": self.permission_mode.as_deref().unwrap_or("workspace_write"),
            "max_llm_calls": self.max_llm_calls,
            "max_llm_tokens": self.max_llm_tokens,
            "profile": self.profile,
        })
    }

    /// Return a new ActionContext with a different mode (used for fallback dispatch).
    pub fn with_mode(&self, mode: ActionMode) -> Self {
        Self {
            mode,
            workspace_root: self.workspace_root.clone(),
            event: self.event.clone(),
            github_token: self.github_token.clone(),
            max_validation_iterations: self.max_validation_iterations,
            max_llm_calls: self.max_llm_calls,
            max_llm_tokens: self.max_llm_tokens,
            profile: self.profile.clone(),
            permission_mode: self.permission_mode.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// ActionOutput
// ---------------------------------------------------------------------------

/// Typed output from an action dispatch operation.
///
/// Represents the formatted result of an engine call that will be
/// written to GitHub Action outputs (step summary, annotations, variables).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionOutput {
    /// Status of the dispatch operation.
    pub status: DispatchStatus,

    /// Human-readable summary of the operation result.
    pub summary: String,

    /// Execution ID if the dispatch resulted in an execution.
    pub execution_id: Option<String>,

    /// List of annotations to emit as GitHub workflow annotations.
    pub annotations: Vec<WorkflowAnnotation>,

    /// Output variables to set via `GITHUB_OUTPUT`.
    pub output_variables: std::collections::HashMap<String, String>,
}

/// Status of a dispatch operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DispatchStatus {
    /// Operation completed successfully.
    Success,
    /// Operation failed with recoverable errors.
    Warning,
    /// Operation failed with non-recoverable errors.
    Failure,
    /// Operation was skipped (e.g., unsupported event).
    Skipped,
}

/// A GitHub workflow annotation (error, warning, notice).
///
/// Corresponds to GitHub Actions `::error file=...,line=...,title=...::message` syntax.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowAnnotation {
    /// Annotation level.
    pub level: AnnotationLevel,
    /// The annotation message.
    pub message: String,
    /// Optional file path the annotation refers to.
    pub file: Option<String>,
    /// Optional line number the annotation refers to.
    pub line: Option<u32>,
    /// Optional column number the annotation refers to.
    pub column: Option<u32>,
    /// Optional title for the annotation.
    pub title: Option<String>,
}

/// Annotation severity level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnnotationLevel {
    /// `::error` — fails the workflow step.
    Error,
    /// `::warning` — non-fatal warning.
    Warning,
    /// `::notice` — informational notice.
    Notice,
}

impl ActionOutput {
    /// Create a success output.
    pub fn success(summary: impl Into<String>, execution_id: Option<String>) -> Self {
        Self {
            status: DispatchStatus::Success,
            summary: summary.into(),
            execution_id,
            annotations: Vec::new(),
            output_variables: std::collections::HashMap::new(),
        }
    }

    /// Create a failure output.
    pub fn failure(summary: impl Into<String>) -> Self {
        Self {
            status: DispatchStatus::Failure,
            summary: summary.into(),
            execution_id: None,
            annotations: Vec::new(),
            output_variables: std::collections::HashMap::new(),
        }
    }

    /// Create a skipped output.
    pub fn skipped(summary: impl Into<String>) -> Self {
        Self {
            status: DispatchStatus::Skipped,
            summary: summary.into(),
            execution_id: None,
            annotations: Vec::new(),
            output_variables: std::collections::HashMap::new(),
        }
    }

    /// Add an annotation to this output.
    pub fn with_annotation(mut self, annotation: WorkflowAnnotation) -> Self {
        self.annotations.push(annotation);
        self
    }

    /// Add an output variable.
    pub fn with_variable(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.output_variables.insert(key.into(), value.into());
        self
    }

    /// Whether the dispatch was successful.
    pub fn is_success(&self) -> bool {
        self.status == DispatchStatus::Success
    }

    /// Whether the dispatch failed.
    pub fn is_failure(&self) -> bool {
        self.status == DispatchStatus::Failure
    }
}
