//! Domain types for GitHub Action input parsing.
//!
//! @canonical actions/.pi/architecture/modules/action-input.md#types
//! Implements: Contract Freeze — ActionInputs, ActionConfig, CommentCommand, CiEnvironment
//! Issue: issue-contract-freeze
//!
//! These are the core domain types that represent parsed GitHub Action inputs,
//! configuration, CI environment detection, and slash command parsing.
//! They serve as the frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All types are serializable (Serialize + Deserialize) where applicable

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ActionInputs
// ---------------------------------------------------------------------------

/// All inputs parsed from the GitHub Action environment.
///
/// Fields marked `Option` are optional and have sensible defaults
/// when not provided (applied by `ConfigLoader`).
///
/// ## Source
///
/// These values originate from the workflow YAML `with:` block and are
/// passed by GitHub Actions as `INPUT_<NAME>` environment variables
/// (uppercased, hyphens replaced with underscores).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActionInputs {
    /// Natural-language intent for the engine.
    /// Required for `run`, `plan`, and `validate` modes.
    pub intent: Option<String>,

    /// Execution mode: auto, governance, run, validate, plan, status.
    /// Default: "auto" (detect from event context).
    pub mode: Option<String>,

    /// Permission mode for engine tool execution.
    /// Default: "workspace_write" (in CI), "prompt" (local).
    pub permission_mode: Option<String>,

    /// Path to policy.toml for Mode A governance.
    /// Default: ".rigorix/policy.toml".
    pub policy_file: Option<String>,

    /// Mode A: fail workflow on policy violations.
    /// Default: false (warn-only for early adoption).
    pub fail_on_violation: Option<bool>,

    /// Fail workflow if the action itself encounters an error.
    /// Default: false (fail-open).
    pub fail_on_action_error: Option<bool>,

    /// Maximum LLM API calls per execution.
    pub max_llm_calls: Option<u32>,

    /// Maximum LLM tokens per execution.
    pub max_llm_tokens: Option<u64>,

    /// Maximum validation loop iterations.
    /// Default: 3.
    pub max_validation_iterations: Option<u32>,

    /// Maximum retries for transient API failures.
    /// Default: 3.
    pub max_retries: Option<u32>,

    /// Base retry delay in milliseconds (exponential backoff with ±25% jitter).
    /// Default: 1000.
    pub retry_delay_ms: Option<u64>,

    /// Whether to post results as a PR comment.
    pub post_pr_comment: Option<bool>,

    /// Configuration profile to use.
    pub profile: Option<String>,
}

// ---------------------------------------------------------------------------
// ActionConfig
// ---------------------------------------------------------------------------

/// Final merged configuration after resolving defaults, YAML, and environment overrides.
///
/// Produced by `ConfigLoader` from merging:
/// 1. `INPUT_*` environment variables (runtime overrides, highest priority)
/// 2. CLI arguments (if run outside GitHub Actions)
/// 3. `action.yml` default values
/// 4. Engine defaults (from rigorix-engine configuration module)
///
/// Unlike `ActionInputs` (which preserves `Option` for all fields),
/// `ActionConfig` has all values resolved to concrete types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionConfig {
    /// Natural-language intent for the engine.
    pub intent: Option<String>,

    /// Execution mode: auto, governance, run, validate, plan, status.
    pub mode: String,

    /// Permission mode: read_only, workspace_write, dangerous_full_access.
    pub permission_mode: String,

    /// Path to policy.toml for Mode A governance.
    pub policy_file: String,

    /// Mode A: fail workflow on policy violations.
    pub fail_on_violation: bool,

    /// Fail workflow if the action itself encounters an error.
    pub fail_on_action_error: bool,

    /// Maximum LLM calls per execution.
    pub max_llm_calls: u32,

    /// Maximum LLM tokens per execution.
    pub max_llm_tokens: u64,

    /// Maximum validation loop iterations.
    pub max_validation_iterations: u32,

    /// Maximum retries for transient API failures.
    pub max_retries: u32,

    /// Base retry delay in milliseconds.
    pub retry_delay_ms: u64,

    /// Whether to post results as a PR comment.
    pub post_pr_comment: bool,

    /// Configuration profile to use.
    pub profile: Option<String>,
}

impl Default for ActionConfig {
    fn default() -> Self {
        Self {
            intent: None,
            mode: "auto".to_string(),
            permission_mode: "workspace_write".to_string(),
            policy_file: ".rigorix/policy.toml".to_string(),
            fail_on_violation: false,
            fail_on_action_error: false,
            max_llm_calls: 50,
            max_llm_tokens: 50000,
            max_validation_iterations: 3,
            max_retries: 3,
            retry_delay_ms: 1000,
            post_pr_comment: true,
            profile: None,
        }
    }
}

// ---------------------------------------------------------------------------
// CommentCommand
// ---------------------------------------------------------------------------

/// Parsed `/rigorix` slash command from an issue or PR comment.
///
/// Supported commands:
/// - `/rigorix run <intent>` — full execution
/// - `/rigorix validate <intent>` — validation loop
/// - `/rigorix plan <intent>` — plan only
/// - `/rigorix status` — current status
/// - `/rigorix retry <execution_id>` — retry a failed execution
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommentCommand {
    /// Full Mode B execution: plan → execute → validate → persist.
    Run {
        /// Natural-language intent describing the work to be done.
        intent: String,
    },

    /// Self-correcting validation loop.
    Validate {
        /// Natural-language intent for validation.
        intent: String,
    },

    /// Planning phase only — no execution.
    Plan {
        /// Natural-language intent for planning.
        intent: String,
    },

    /// Show current execution status.
    Status,

    /// Retry a previously failed execution.
    Retry {
        /// The execution ID to retry.
        execution_id: String,
    },

    /// Show help/usage information.
    Help,
}

// ---------------------------------------------------------------------------
// CiEnvironment
// ---------------------------------------------------------------------------

/// Detected CI environment type.
///
/// Determines permission defaults and output formatting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CiEnvironment {
    /// Running inside GitHub Actions.
    GitHubActions {
        /// Value of `GITHUB_WORKSPACE`.
        workspace: String,
        /// Value of `GITHUB_EVENT_NAME`.
        event_name: String,
        /// Value of `GITHUB_ACTOR`.
        actor: String,
    },

    /// Running locally (not in CI).
    Local,
}

impl CiEnvironment {
    /// Whether the current environment is CI.
    pub fn is_ci(&self) -> bool {
        matches!(self, CiEnvironment::GitHubActions { .. })
    }
}

// ---------------------------------------------------------------------------
// GitHubEvent
// ---------------------------------------------------------------------------

/// Parsed GitHub event context from `GITHUB_EVENT_PATH`.
///
/// Not all fields are present — the structure depends on the event type
/// (`GITHUB_EVENT_NAME`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum GitHubEvent {
    /// Triggered by `workflow_dispatch` — manual trigger via UI or API.
    WorkflowDispatch {
        /// Branch/tag/ref that was dispatched.
        ref_name: String,
        /// Inputs passed to the workflow_dispatch event.
        inputs: std::collections::HashMap<String, serde_json::Value>,
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
        /// PR body text.
        body: Option<String>,
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
        /// Commit message.
        message: String,
        /// Pusher username.
        pusher: String,
    },

    /// Unknown or unsupported event type.
    Unknown {
        /// The raw event name from `GITHUB_EVENT_NAME`.
        event_name: String,
    },
}
