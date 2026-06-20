# Action Input Architecture

<!--
Canonical Reference: .pi/architecture/modules/action-input.md
Blueprint Source: Rigorix design session (2026-06-20)
Rationale: Parse GitHub Action inputs and environment context into engine-compatible types
-->

## Overview

The Action Input module parses GitHub Actions environment variables, event payloads, and workflow inputs into typed Rust structs that the engine can consume. It handles the impedance mismatch between GitHub's string-based `INPUT_*` environment variables and the engine's structured types.

## Responsibilities

- Parse `INPUT_*` environment variables into typed action inputs
- Parse GitHub event payload JSON (`GITHUB_EVENT_PATH`) into event types
- Parse `/rigorix` slash commands from issue/PR comments
- Resolve workspace root from `GITHUB_WORKSPACE`
- Extract intent from multiple sources (workflow inputs, comment body, PR body)
- Detect CI environment and set appropriate permission mode
- Validate required inputs and emit GitHub Action error annotations

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| InputParser | `actions/src/action_input/parser.rs` | Main parser: env vars → ActionInputs | #parser |
| EventPayloadParser | `actions/src/action_input/event_parser.rs` | Parses `GITHUB_EVENT_PATH` JSON | #event-parser |
| CommentParser | `actions/src/action_input/comment_parser.rs` | Parses `/rigorix` commands from text | #comment-parser |
| CiDetector | `actions/src/action_input/ci_detector.rs` | Detects CI environment, sets permissions | #ci-detector |
| ConfigLoader | `actions/src/action_input/config_loader.rs` | Loads action.yml config and merges with CLI overrides | #config-loader |
| ActionInputs | `actions/src/action_input/types.rs` | Typed container of all parsed inputs | #types |

---

## Component Details

### ActionInputs

**Purpose:** Typed container of all GitHub Action inputs

```rust
/// All inputs parsed from the GitHub Action environment.
///
/// Fields marked `Option` are optional and have sensible defaults
/// when not provided.
#[derive(Debug, Clone)]
pub struct ActionInputs {
    /// Natural-language intent for the engine.
    /// Required for `run`, `plan`, and `validate` modes.
    pub intent: Option<String>,

    /// Execution mode: run, plan, validate, status.
    /// Default: run (if intent provided), status (otherwise).
    pub mode: Option<String>,

    /// Permission mode for the engine.
    /// Default: workspace_write.
    pub permission_mode: Option<String>,

    /// Maximum LLM calls (budget enforcement).
    pub max_llm_calls: Option<u32>,

    /// Maximum LLM tokens (budget enforcement).
    pub max_llm_tokens: Option<u64>,

    /// Validation loop max iterations.
    /// Default: 3.
    pub max_validation_iterations: Option<u32>,

    /// Quality level required for validation success.
    pub required_quality: Option<String>,

    /// Whether to post results as a PR comment.
    pub post_pr_comment: Option<bool>,

    /// Configuration profile to use.
    pub profile: Option<String>,
}
```

### InputParser

**Purpose:** Parse all GitHub Action inputs from environment variables

```rust
/// Parses GitHub Action inputs from environment variables.
///
/// GitHub Actions passes workflow inputs as `INPUT_<NAME>` environment
/// variables (uppercased, hyphens replaced with underscores).
pub struct InputParser;

impl InputParser {
    /// Parse all inputs from the environment.
    pub fn parse() -> Result<ActionInputs, ActionInputError> {
        Ok(ActionInputs {
            intent: Self::read_input("INTENT"),
            mode: Self::read_input("MODE"),
            permission_mode: Self::read_input("PERMISSION_MODE"),
            max_llm_calls: Self::read_input_opt("MAX_LLM_CALLS")?.map(|v| v.parse().ok()).flatten(),
            max_llm_tokens: Self::read_input_opt("MAX_LLM_TOKENS")?.map(|v| v.parse().ok()).flatten(),
            max_validation_iterations: Self::read_input_opt("MAX_VALIDATION_ITERATIONS")?.map(|v| v.parse().ok()).flatten(),
            required_quality: Self::read_input("REQUIRED_QUALITY"),
            post_pr_comment: Self::read_input_opt("POST_PR_COMMENT")?.map(|v| v == "true"),
            profile: Self::read_input("PROFILE"),
        })
    }

    /// Read an optional input (returns None if empty/missing).
    fn read_input(name: &str) -> Option<String> {
        let key = format!("INPUT_{}", name);
        std::env::var(&key).ok().filter(|v| !v.is_empty())
    }

    /// Read an input that may not exist (returns Ok(None) if env var missing).
    fn read_input_opt(name: &str) -> Result<Option<String>, ActionInputError> {
        Ok(Self::read_input(name))
    }

    /// Read a required input, error if missing.
    fn read_required(name: &str) -> Result<String, ActionInputError> {
        Self::read_input(name)
            .ok_or_else(|| ActionInputError::MissingRequiredInput(name.to_string()))
    }
}
```

### CommentParser

**Purpose:** Parse `/rigorix` slash commands from issue and PR comments

```rust
/// Parses `/rigorix` commands from issue/PR comment text.
///
/// Supported commands:
/// - `/rigorix run <intent>` — full execution
/// - `/rigorix validate <intent>` — validation loop
/// - `/rigorix plan <intent>` — plan only
/// - `/rigorix status` — current status
/// - `/rigorix retry <execution_id>` — retry a failed execution
pub struct CommentParser;

impl CommentParser {
    /// Parse a comment body for rigorix commands.
    /// Returns None if no command is found.
    pub fn parse(comment: &str) -> Option<CommentCommand> {
        let trimmed = comment.trim();
        if !trimmed.starts_with("/rigorix") {
            return None;
        }

        let parts: Vec<&str> = trimmed.splitn(3, ' ').collect();
        match parts.get(1).copied() {
            Some("run") => Some(CommentCommand::Run {
                intent: parts.get(2).unwrap_or(&"").to_string(),
            }),
            Some("validate") => Some(CommentCommand::Validate {
                intent: parts.get(2).unwrap_or(&"").to_string(),
            }),
            Some("plan") => Some(CommentCommand::Plan {
                intent: parts.get(2).unwrap_or(&"").to_string(),
            }),
            Some("status") => Some(CommentCommand::Status),
            Some("retry") => {
                let id = parts.get(2).unwrap_or(&"").to_string();
                Some(CommentCommand::Retry { execution_id: id })
            }
            _ => Some(CommentCommand::Help),
        }
    }
}

pub enum CommentCommand {
    Run { intent: String },
    Validate { intent: String },
    Plan { intent: String },
    Status,
    Retry { execution_id: String },
    Help,
}
```

### CiDetector

**Purpose:** Detects the CI environment and adjusts engine configuration

```rust
/// Detects whether we're running in CI and adjusts permissions accordingly.
///
/// In CI, the default permission mode is elevated to `workspace_write`
/// (since there's no human to confirm prompts). PR comments replace
/// interactive confirmation.
pub struct CiDetector;

impl CiDetector {
    /// Detect the CI environment type.
    pub fn detect() -> CiEnvironment {
        if std::env::var("GITHUB_ACTIONS").is_ok() {
            CiEnvironment::GitHubActions {
                workspace: std::env::var("GITHUB_WORKSPACE").unwrap_or_default(),
                event_name: std::env::var("GITHUB_EVENT_NAME").unwrap_or_default(),
                actor: std::env::var("GITHUB_ACTOR").unwrap_or_default(),
            }
        } else {
            CiEnvironment::Local
        }
    }

    /// Get the appropriate permission mode for the current environment.
    /// In CI: workspace_write (no human to confirm).
    /// Local: prompt (interactive confirmation).
    pub fn default_permission_mode(&self) -> &str {
        match self.detect() {
            CiEnvironment::GitHubActions { .. } => "workspace_write",
            CiEnvironment::Local => "prompt",
        }
    }
}

pub enum CiEnvironment {
    GitHubActions { workspace: String, event_name: String, actor: String },
    Local,
}
```

### ConfigLoader

**Purpose:** Loads `action.yml` defaults and merges with environment overrides

The action crate's configuration is split across two sources: `action.yml` defaults (for the GitHub Actions UI) and `INPUT_*` environment variables (runtime overrides). The `ConfigLoader` merges them with environment variables taking precedence.

```rust
/// Loads action configuration from action.yml defaults and environment.
///
/// Merging precedence (highest to lowest):
/// 1. INPUT_* environment variables (runtime overrides)
/// 2. CLI arguments (if run outside GitHub Actions)
/// 3. action.yml defaults
/// 4. Engine defaults (from rigorix-engine configuration module)
pub struct ConfigLoader;

impl ConfigLoader {
    /// Load the merged configuration.
    pub fn load() -> Result<ActionConfig, ActionInputError> {
        let env_inputs = InputParser::parse()?;
        let yaml_defaults = Self::load_action_yml_defaults()?;
        let merged = Self::merge(yaml_defaults, env_inputs);
        Ok(merged)
    }

    /// Parse action.yml to extract default input values.
    fn load_action_yml_defaults() -> Result<ActionInputs, ActionInputError> {
        let yaml_path = std::env::current_dir()
            .map(|p| p.join("action.yml"))
            .map_err(|e| ActionInputError::Io(e))?;

        if !yaml_path.exists() {
            return Ok(ActionInputs::default());
        }

        let content = std::fs::read_to_string(&yaml_path)?;
        let yaml: serde_yaml::Value = serde_yaml::from_str(&content)?;
        // Extract default values from inputs section
        Self::parse_yaml_defaults(&yaml)
    }

    /// Merge: environment overrides take precedence over YAML defaults.
    fn merge(defaults: ActionInputs, overrides: ActionInputs) -> ActionConfig {
        ActionConfig {
            intent: overrides.intent.or(defaults.intent),
            mode: overrides.mode.or(defaults.mode).unwrap_or_else(|| "run".to_string()),
            permission_mode: overrides.permission_mode.or(defaults.permission_mode),
            max_llm_calls: overrides.max_llm_calls.or(defaults.max_llm_calls),
            max_llm_tokens: overrides.max_llm_tokens.or(defaults.max_llm_tokens),
            max_validation_iterations: overrides.max_validation_iterations.or(defaults.max_validation_iterations),
            required_quality: overrides.required_quality.or(defaults.required_quality),
            post_pr_comment: overrides.post_pr_comment.or(defaults.post_pr_comment),
            profile: overrides.profile.or(defaults.profile),
        }
    }
}

/// Final merged configuration passed to the action router.
#[derive(Debug, Clone)]
pub struct ActionConfig {
    pub intent: Option<String>,
    pub mode: String,
    pub permission_mode: Option<String>,
    pub max_llm_calls: Option<u32>,
    pub max_llm_tokens: Option<u64>,
    pub max_validation_iterations: Option<u32>,
    pub required_quality: Option<String>,
    pub post_pr_comment: Option<bool>,
    pub profile: Option<String>,
}
```

---

## action.yml Input Schema

The inputs defined in `action.yml` map to the `ActionInputs` struct:

```yaml
# action.yml
inputs:
  intent:
    description: 'Natural-language intent for the engine'
    required: false
  mode:
    description: 'Execution mode: run, plan, validate, status'
    required: false
    default: 'run'
  permission-mode:
    description: 'Permission mode: read_only, workspace_write, dangerous_full_access'
    required: false
    default: 'workspace_write'
  max-llm-calls:
    description: 'Maximum LLM API calls per execution'
    required: false
    default: '50'
  max-llm-tokens:
    description: 'Maximum LLM tokens per execution'
    required: false
    default: '50000'
  max-validation-iterations:
    description: 'Max validation loop iterations'
    required: false
    default: '3'
  post-pr-comment:
    description: 'Post results as a PR comment'
    required: false
    default: 'true'
  profile:
    description: 'Configuration profile to use'
    required: false

outputs:
  execution_id:
    description: 'UUID of the execution'
  status:
    description: 'Final execution status'
  iterations:
    description: 'Validation loop iterations'
  template_id:
    description: 'ID of the generated template'
  quality_level:
    description: 'Achieved quality level'
  failure_count:
    description: 'Number of failures'
```

---

## Data Flow

```
GitHub Action workflow triggered
        │
        ▼
InputParser::parse()
  - INPUT_INTENT → intent
  - INPUT_MODE → mode
  - INPUT_PERMISSION_MODE → permission_mode
  - INPUT_MAX_LLM_CALLS → max_llm_calls
  - INPUT_MAX_LLM_TOKENS → max_llm_tokens
  - INPUT_MAX_VALIDATION_ITERATIONS → max_validation_iterations
  - INPUT_REQUIRED_QUALITY → required_quality
        │
        ▼
EventPayloadParser::parse(GITHUB_EVENT_PATH)
  - workflow_dispatch → WorkflowDispatch
  - issue_comment → IssueComment { issue_number, comment }
  - pull_request → PullRequest { pr_number, action }
        │
        ▼
CiDetector::detect()
  - GITHUB_ACTIONS present → GitHubActions
  - else → Local
        │
        ▼
If event is IssueComment:
  CommentParser::parse(comment)
    → CommentCommand::Run { intent }
    → CommentCommand::Validate { intent }
        │
        ▼
ActionInputs + GitHubEvent + CiEnvironment
  → fed to ActionRouter for engine dispatch
```

---

## Dependencies

### Depends On
- Nothing (standalone input parser, no engine dependency)
- Standard library: `std::env`, `serde_json` (for event payload parsing)

### Used By
- **action-entrypoint**: Consumes parsed inputs for dispatch decisions

---

## Related ADRs

- **Engine ADR-001** (`engine/.pi/architecture/decisions/ADR-001-architecture-pattern.md`): Clean Architecture layering
- **Engine ADR-007** (`engine/.pi/architecture/decisions/ADR-007-risk-gating-model.md`): CiDetector → PermissionMode mapping
- **Actions ADR-103** (`actions/.pi/architecture/decisions/ADR-103-ci-permission-mode.md`): CI defaults to workspace_write

---

*Last updated: 2026-06-20*
*Module version: 1.0.0 (Planned)*

---

**Status:** Planned
**Engine modules reused:** None (standalone input parsing)
