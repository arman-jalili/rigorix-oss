//! ContextBuilder implementation — builds ActionContext from environment.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md#actioncontext
//! Implements: ContextBuilder trait — reads GitHub Action env and builds typed context
//! Issue: issue-actioncontext (#615)
//!
//! Builds an `ActionContext` from environment variables, event payload files, and
//! GitHub Action inputs. Uses `ContextRepository` for all data access.

use std::sync::Arc;

use async_trait::async_trait;

use crate::action_entrypoint::domain::{ActionContext, ActionError, ActionMode, GitHubEvent};
use crate::action_entrypoint::infrastructure::repository::ContextRepository;

use super::dto::{BuildContextInput, BuildContextOutput, ParseEventOutput};
use super::service::ContextBuilder;

/// Builds an `ActionContext` from the GitHub Actions environment.
///
/// Uses a `ContextRepository` to read environment variables, event payload
/// files, and GitHub token. The repository can be mocked for testing.
pub struct ContextBuilderImpl {
    repository: Arc<dyn ContextRepository>,
}

impl ContextBuilderImpl {
    /// Create a new ContextBuilderImpl with the given repository.
    pub fn new(repository: Arc<dyn ContextRepository>) -> Self {
        Self { repository }
    }

    /// Parse a GitHub event JSON payload into a typed `GitHubEvent`.
    async fn parse_event_payload(
        &self,
        event_name: &str,
        content: &str,
    ) -> Result<ParseEventOutput, ActionError> {
        let json: serde_json::Value =
            serde_json::from_str(content).map_err(|e| ActionError::Json(e))?;

        let file_size = content.len() as u64;

        let event = match event_name {
            "workflow_dispatch" => {
                let ref_name = json
                    .get("ref")
                    .and_then(|v| v.as_str())
                    .or_else(|| {
                        json.get("inputs")
                            .and_then(|v| v.get("ref"))
                            .and_then(|v| v.as_str())
                    })
                    .unwrap_or("main")
                    .to_string();
                GitHubEvent::WorkflowDispatch { ref_name }
            }
            "issue_comment" => {
                let issue_number = json
                    .pointer("/issue/number")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let comment_body = json
                    .pointer("/comment/body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let commenter = json
                    .pointer("/comment/user/login")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                GitHubEvent::IssueComment {
                    issue_number,
                    comment_body,
                    commenter,
                }
            }
            "pull_request" | "pull_request_target" => {
                let pr_number = json
                    .pointer("/pull_request/number")
                    .or_else(|| json.pointer("/number"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let action = json
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("opened")
                    .to_string();
                let title = json
                    .pointer("/pull_request/title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let base_branch = json
                    .pointer("/pull_request/base/ref")
                    .and_then(|v| v.as_str())
                    .unwrap_or("main")
                    .to_string();
                let head_branch = json
                    .pointer("/pull_request/head/ref")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let head_sha = json
                    .pointer("/pull_request/head/sha")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                GitHubEvent::PullRequest {
                    pr_number,
                    action,
                    title,
                    base_branch,
                    head_branch,
                    head_sha,
                }
            }
            "push" => {
                let branch = json
                    .get("ref")
                    .and_then(|v| v.as_str())
                    .map(|r| r.trim_start_matches("refs/heads/"))
                    .unwrap_or("")
                    .to_string();
                let sha = json
                    .get("after")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let pusher = json
                    .pointer("/pusher/name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                GitHubEvent::Push {
                    branch,
                    sha,
                    pusher,
                }
            }
            _ => GitHubEvent::Unknown {
                event_name: event_name.to_string(),
            },
        };

        Ok(ParseEventOutput {
            event,
            raw_payload: json,
            file_size_bytes: Some(file_size),
        })
    }

    /// Resolve the execution mode from inputs.
    fn resolve_mode(
        &self,
        input_mode: Option<&str>,
        event_name: &str,
        _event_payload: &serde_json::Value,
    ) -> Result<(ActionMode, String), ActionError> {
        // Priority 1: Explicit INPUT_MODE
        if let Some(mode_str) = input_mode {
            match mode_str.to_lowercase().as_str() {
                "run" => {
                    return Ok((
                        ActionMode::Run {
                            intent: String::new(),
                        },
                        "input".to_string(),
                    ));
                }
                "plan" => {
                    return Ok((
                        ActionMode::Plan {
                            intent: String::new(),
                        },
                        "input".to_string(),
                    ));
                }
                "validate" => {
                    return Ok((
                        ActionMode::Validate {
                            intent: String::new(),
                        },
                        "input".to_string(),
                    ));
                }
                "status" => {
                    return Ok((ActionMode::Status, "input".to_string()));
                }
                "auto" => { /* fall through to event-based resolution */ }
                _ => {
                    return Err(ActionError::ModeResolutionError {
                        detail: format!("Unknown mode string: '{mode_str}'"),
                        input_mode: Some(mode_str.to_string()),
                        event_name: Some(event_name.to_string()),
                    });
                }
            }
        }

        // Priority 2: Event-based resolution
        match event_name {
            "workflow_dispatch" | "repository_dispatch" => Ok((
                ActionMode::Run {
                    intent: String::new(),
                },
                "event_type".to_string(),
            )),
            "pull_request" | "pull_request_target" => Ok((
                ActionMode::Validate {
                    intent: String::new(),
                },
                "event_type".to_string(),
            )),
            "issue_comment" => {
                // For issue_comment, we default to Run but the caller
                // can override with slash command parsing
                Ok((
                    ActionMode::Run {
                        intent: String::new(),
                    },
                    "event_type".to_string(),
                ))
            }
            "push" => Ok((ActionMode::Status, "event_type".to_string())),
            _ => {
                // Fallback to Status for unknown events
                Ok((ActionMode::Status, "default".to_string()))
            }
        }
    }
}

#[async_trait]
impl ContextBuilder for ContextBuilderImpl {
    async fn build(&self, input: BuildContextInput) -> Result<BuildContextOutput, ActionError> {
        let mut warnings = Vec::new();

        // Helper: read env var from overrides or real env
        let env_val = |name: &str| -> Option<String> {
            if let Some(ref overrides) = input.env_override {
                overrides.get(name).cloned()
            } else {
                // This is not async but used in an async context — use block_on or sync approach
                None
            }
        };

        // 1. Workspace root
        let workspace_root = if let Some(ref override_root) = input.workspace_override {
            override_root.clone()
        } else if let Some(overrides) = &input.env_override {
            overrides.get("GITHUB_WORKSPACE").cloned().ok_or_else(|| {
                ActionError::MissingContext {
                    detail: "GITHUB_WORKSPACE environment variable is not set".to_string(),
                    env_var: Some("GITHUB_WORKSPACE".to_string()),
                }
            })?
        } else {
            self.repository.workspace_root().await?
        };

        // 2. Event name
        let event_name = if let Some(ref override_name) = input.event_name_override {
            override_name.clone()
        } else if let Some(overrides) = &input.env_override {
            overrides.get("GITHUB_EVENT_NAME").cloned().ok_or_else(|| {
                ActionError::MissingContext {
                    detail: "GITHUB_EVENT_NAME environment variable is not set".to_string(),
                    env_var: Some("GITHUB_EVENT_NAME".to_string()),
                }
            })?
        } else {
            self.repository.event_name().await?
        };

        // 3. Event payload
        let (event, raw_payload) = if let Some(ref override_payload) = input.event_payload_override
        {
            let parsed = self
                .parse_event_payload(&event_name, override_payload)
                .await?;
            (parsed.event, parsed.raw_payload)
        } else {
            let event_path = if let Some(ref override_path) = input.event_path_override {
                override_path.clone()
            } else if let Some(overrides) = &input.env_override {
                overrides.get("GITHUB_EVENT_PATH").cloned().ok_or_else(|| {
                    ActionError::MissingContext {
                        detail: "GITHUB_EVENT_PATH environment variable is not set".to_string(),
                        env_var: Some("GITHUB_EVENT_PATH".to_string()),
                    }
                })?
            } else {
                self.repository.event_path().await?
            };

            let content = self.repository.read_event_payload(&event_path).await?;
            let parsed = self.parse_event_payload(&event_name, &content).await?;
            (parsed.event, parsed.raw_payload)
        };

        // 4. Input mode
        let input_mode = if let Some(overrides) = &input.env_override {
            overrides.get("INPUT_MODE").cloned()
        } else {
            self.repository.read_env_var("INPUT_MODE").await?
        };

        // 5. Resolve mode
        let (mode, _mode_source) =
            self.resolve_mode(input_mode.as_deref(), &event_name, &raw_payload)?;

        if input_mode.is_some() {
            warnings.push(format!(
                "Mode resolved from INPUT_MODE: {}",
                input_mode.as_deref().unwrap()
            ));
        } else {
            warnings.push(format!("Mode resolved from event type '{}'", event_name));
        }

        // 6. GitHub token
        let github_token = if let Some(overrides) = &input.env_override {
            overrides.get("GITHUB_TOKEN").cloned()
        } else {
            self.repository.github_token().await?
        };

        // 7. Intent from INPUT_INTENT
        let intent = if let Some(overrides) = &input.env_override {
            overrides.get("INPUT_INTENT").cloned()
        } else {
            self.repository.read_env_var("INPUT_INTENT").await?
        };

        // Apply intent to mode if needed
        let mode = if let Some(ref intent_val) = intent {
            match mode {
                ActionMode::Run { .. } => ActionMode::Run {
                    intent: intent_val.clone(),
                },
                ActionMode::Plan { .. } => ActionMode::Plan {
                    intent: intent_val.clone(),
                },
                ActionMode::Validate { .. } => ActionMode::Validate {
                    intent: intent_val.clone(),
                },
                other => other,
            }
        } else {
            mode
        };

        // 8. Additional context fields
        let max_validation_iterations = if let Some(overrides) = &input.env_override {
            overrides
                .get("INPUT_MAX_VALIDATION_ITERATIONS")
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(3)
        } else {
            self.repository
                .read_env_var("INPUT_MAX_VALIDATION_ITERATIONS")
                .await?
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(3)
        };

        let max_llm_calls = if let Some(overrides) = &input.env_override {
            overrides
                .get("INPUT_MAX_LLM_CALLS")
                .and_then(|v| v.parse::<u32>().ok())
        } else {
            self.repository
                .read_env_var("INPUT_MAX_LLM_CALLS")
                .await?
                .and_then(|v| v.parse::<u32>().ok())
        };

        let max_llm_tokens = if let Some(overrides) = &input.env_override {
            overrides
                .get("INPUT_MAX_LLM_TOKENS")
                .and_then(|v| v.parse::<u64>().ok())
        } else {
            self.repository
                .read_env_var("INPUT_MAX_LLM_TOKENS")
                .await?
                .and_then(|v| v.parse::<u64>().ok())
        };

        let profile = if let Some(overrides) = &input.env_override {
            overrides.get("INPUT_PROFILE").cloned()
        } else {
            self.repository.read_env_var("INPUT_PROFILE").await?
        };

        let permission_mode = if let Some(overrides) = &input.env_override {
            overrides.get("INPUT_PERMISSION_MODE").cloned()
        } else {
            self.repository
                .read_env_var("INPUT_PERMISSION_MODE")
                .await?
        };

        let context = ActionContext {
            workspace_root,
            event,
            mode: mode.clone(),
            github_token,
            max_validation_iterations,
            max_llm_calls,
            max_llm_tokens,
            profile,
            permission_mode,
        };

        Ok(BuildContextOutput {
            context,
            event_name,
            input_mode,
            warnings,
        })
    }

    async fn get_workspace_root(&self) -> Result<String, ActionError> {
        self.repository.workspace_root().await
    }

    async fn get_github_token(&self) -> Result<Option<String>, ActionError> {
        self.repository.github_token().await
    }

    async fn parse_event(
        &self,
        event_name: &str,
        event_path: &str,
    ) -> Result<ParseEventOutput, ActionError> {
        let content = self.repository.read_event_payload(event_path).await?;
        self.parse_event_payload(event_name, &content).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // ── Mock ContextRepository ──

    struct MockContextRepository {
        env_vars: std::collections::HashMap<String, String>,
        event_payload: Option<String>,
    }

    impl MockContextRepository {
        fn new() -> Self {
            let mut env = std::collections::HashMap::new();
            env.insert("GITHUB_WORKSPACE".to_string(), "/tmp/workspace".to_string());
            env.insert(
                "GITHUB_EVENT_NAME".to_string(),
                "workflow_dispatch".to_string(),
            );
            env.insert(
                "GITHUB_EVENT_PATH".to_string(),
                "/tmp/event.json".to_string(),
            );
            env.insert("GITHUB_TOKEN".to_string(), "gh_token_123".to_string());
            Self {
                env_vars: env,
                event_payload: Some(r#"{"ref": "main"}"#.to_string()),
            }
        }

        fn with_env(mut self, key: &str, value: &str) -> Self {
            self.env_vars.insert(key.to_string(), value.to_string());
            self
        }

        fn with_event(mut self, event_name: &str, payload: &str) -> Self {
            self.env_vars
                .insert("GITHUB_EVENT_NAME".to_string(), event_name.to_string());
            self.event_payload = Some(payload.to_string());
            self
        }

        fn with_intent(self, intent: &str) -> Self {
            self.with_env("INPUT_INTENT", intent)
        }

        fn with_mode(self, mode: &str) -> Self {
            self.with_env("INPUT_MODE", mode)
        }
    }

    #[async_trait]
    impl ContextRepository for MockContextRepository {
        async fn read_env_var(&self, name: &str) -> Result<Option<String>, ActionError> {
            Ok(self.env_vars.get(name).cloned())
        }

        async fn read_env_vars(
            &self,
            _prefix: &str,
        ) -> Result<std::collections::HashMap<String, String>, ActionError> {
            Ok(self.env_vars.clone())
        }

        async fn has_env_var(&self, name: &str) -> Result<bool, ActionError> {
            Ok(self.env_vars.contains_key(name))
        }

        async fn workspace_root(&self) -> Result<String, ActionError> {
            self.env_vars
                .get("GITHUB_WORKSPACE")
                .cloned()
                .ok_or_else(|| ActionError::MissingContext {
                    detail: "Missing GITHUB_WORKSPACE".to_string(),
                    env_var: Some("GITHUB_WORKSPACE".to_string()),
                })
        }

        async fn event_name(&self) -> Result<String, ActionError> {
            self.env_vars
                .get("GITHUB_EVENT_NAME")
                .cloned()
                .ok_or_else(|| ActionError::MissingContext {
                    detail: "Missing GITHUB_EVENT_NAME".to_string(),
                    env_var: Some("GITHUB_EVENT_NAME".to_string()),
                })
        }

        async fn event_path(&self) -> Result<String, ActionError> {
            self.env_vars
                .get("GITHUB_EVENT_PATH")
                .cloned()
                .ok_or_else(|| ActionError::MissingContext {
                    detail: "Missing GITHUB_EVENT_PATH".to_string(),
                    env_var: Some("GITHUB_EVENT_PATH".to_string()),
                })
        }

        async fn read_event_payload(&self, _path: &str) -> Result<String, ActionError> {
            self.event_payload
                .clone()
                .ok_or_else(|| ActionError::ContextRepositoryError {
                    detail: "No mock event payload".to_string(),
                })
        }

        async fn github_token(&self) -> Result<Option<String>, ActionError> {
            Ok(self.env_vars.get("GITHUB_TOKEN").cloned())
        }

        async fn github_api_url(&self) -> Result<String, ActionError> {
            Ok("https://api.github.com".to_string())
        }

        async fn read_ci_env_vars(
            &self,
        ) -> Result<std::collections::HashMap<String, String>, ActionError> {
            Ok(self.env_vars.clone())
        }

        async fn resolve_path(&self, path: &str) -> Result<String, ActionError> {
            let workspace = self.workspace_root().await?;
            Ok(std::path::PathBuf::from(&workspace)
                .join(path)
                .to_string_lossy()
                .to_string())
        }
    }

    fn create_builder(mock: MockContextRepository) -> ContextBuilderImpl {
        ContextBuilderImpl::new(Arc::new(mock))
    }

    // ── Tests ──

    #[tokio::test]
    async fn test_build_context_workflow_dispatch() {
        let builder = create_builder(MockContextRepository::new());
        let input = BuildContextInput::default();
        let output = builder.build(input).await.unwrap();

        assert_eq!(output.context.workspace_root, "/tmp/workspace");
        assert_eq!(output.event_name, "workflow_dispatch");
        assert!(output.context.github_token.is_some());
        assert_eq!(output.context.github_token.unwrap(), "gh_token_123");

        match output.context.mode {
            ActionMode::Run { .. } => {} // workflow_dispatch -> Run
            other => panic!("Expected Run mode, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_build_context_pull_request() {
        let mock = MockContextRepository::new().with_event(
            "pull_request",
            r#"{
                "action": "opened",
                "number": 42,
                "pull_request": {
                    "title": "Test PR",
                    "base": { "ref": "main" },
                    "head": { "ref": "feature-branch", "sha": "abc123" }
                }
            }"#,
        );
        let builder = create_builder(mock);
        let input = BuildContextInput::default();
        let output = builder.build(input).await.unwrap();

        assert_eq!(output.event_name, "pull_request");

        match output.context.mode {
            ActionMode::Validate { .. } => {} // pull_request -> Validate
            other => panic!("Expected Validate mode, got {:?}", other),
        }

        match output.context.event {
            GitHubEvent::PullRequest {
                pr_number, title, ..
            } => {
                assert_eq!(pr_number, 42);
                assert_eq!(title, "Test PR");
            }
            other => panic!("Expected PullRequest event, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_build_context_with_input_mode() {
        let mock = MockContextRepository::new().with_mode("plan");
        let builder = create_builder(mock);
        let input = BuildContextInput::default();
        let output = builder.build(input).await.unwrap();

        match output.context.mode {
            ActionMode::Plan { .. } => {}
            other => panic!("Expected Plan mode, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_build_context_with_input_mode_and_intent() {
        let mock = MockContextRepository::new()
            .with_mode("run")
            .with_intent("implement feature X");
        let builder = create_builder(mock);
        let input = BuildContextInput::default();
        let output = builder.build(input).await.unwrap();

        match output.context.mode {
            ActionMode::Run { intent } => {
                assert_eq!(intent, "implement feature X");
            }
            other => panic!("Expected Run mode with intent, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_build_context_issue_comment() {
        let mock = MockContextRepository::new().with_event(
            "issue_comment",
            r#"{
                "issue": { "number": 100 },
                "comment": {
                    "body": "/rigorix run implement feature",
                    "user": { "login": "test-user" }
                }
            }"#,
        );
        let builder = create_builder(mock);
        let input = BuildContextInput::default();
        let output = builder.build(input).await.unwrap();

        assert_eq!(output.event_name, "issue_comment");

        match output.context.event {
            GitHubEvent::IssueComment {
                issue_number,
                commenter,
                ..
            } => {
                assert_eq!(issue_number, 100);
                assert_eq!(commenter, "test-user");
            }
            other => panic!("Expected IssueComment event, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_build_context_push() {
        let mock = MockContextRepository::new().with_event(
            "push",
            r#"{
                "ref": "refs/heads/main",
                "after": "def456",
                "pusher": { "name": "dev-user" }
            }"#,
        );
        let builder = create_builder(mock);
        let input = BuildContextInput::default();
        let output = builder.build(input).await.unwrap();

        assert_eq!(output.event_name, "push");

        match output.context.mode {
            ActionMode::Status => {} // push -> Status
            other => panic!("Expected Status mode, got {:?}", other),
        }

        match output.context.event {
            GitHubEvent::Push {
                branch,
                sha,
                pusher,
            } => {
                assert_eq!(branch, "main");
                assert_eq!(sha, "def456");
                assert_eq!(pusher, "dev-user");
            }
            other => panic!("Expected Push event, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_build_context_unknown_event() {
        let mock = MockContextRepository::new().with_event("unknown_event", "{}");
        let builder = create_builder(mock);
        let input = BuildContextInput::default();
        let output = builder.build(input).await.unwrap();

        match output.context.event {
            GitHubEvent::Unknown { .. } => {}
            other => panic!("Expected Unknown event, got {:?}", other),
        }

        match output.context.mode {
            ActionMode::Status => {} // unknown -> Status fallback
            other => panic!("Expected Status fallback, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_build_context_env_overrides() {
        let builder = create_builder(MockContextRepository::new());
        let mut env_override = std::collections::HashMap::new();
        env_override.insert(
            "GITHUB_WORKSPACE".to_string(),
            "/custom/workspace".to_string(),
        );
        env_override.insert("GITHUB_EVENT_NAME".to_string(), "status".to_string());
        env_override.insert(
            "GITHUB_EVENT_PATH".to_string(),
            "/tmp/status.json".to_string(),
        );
        env_override.insert("INPUT_MODE".to_string(), "status".to_string());

        let input = BuildContextInput {
            env_override: Some(env_override),
            event_payload_override: Some(r#"{}"#.to_string()),
            ..Default::default()
        };

        let output = builder.build(input).await.unwrap();
        assert_eq!(output.context.workspace_root, "/custom/workspace");
        assert_eq!(output.event_name, "status");
    }

    #[tokio::test]
    async fn test_build_context_missing_workspace() {
        let mut mock = MockContextRepository::new();
        mock.env_vars.remove("GITHUB_WORKSPACE");
        let builder = create_builder(mock);
        let input = BuildContextInput::default();
        let result = builder.build(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_build_context_max_validation_iterations() {
        let mock = MockContextRepository::new().with_env("INPUT_MAX_VALIDATION_ITERATIONS", "5");
        let builder = create_builder(mock);
        let input = BuildContextInput::default();
        let output = builder.build(input).await.unwrap();
        assert_eq!(output.context.max_validation_iterations, 5);
    }

    #[tokio::test]
    async fn test_parse_event_workflow_dispatch() {
        let builder = create_builder(MockContextRepository::new());
        let parsed = builder
            .parse_event_payload("workflow_dispatch", r#"{"ref": "develop"}"#)
            .await
            .unwrap();

        match parsed.event {
            GitHubEvent::WorkflowDispatch { ref_name } => {
                assert_eq!(ref_name, "develop");
            }
            other => panic!("Expected WorkflowDispatch, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_get_workspace_root() {
        let mock = MockContextRepository::new();
        let builder = create_builder(mock);
        let root = builder.get_workspace_root().await.unwrap();
        assert_eq!(root, "/tmp/workspace");
    }

    #[tokio::test]
    async fn test_get_github_token() {
        let mock = MockContextRepository::new();
        let builder = create_builder(mock);
        let token = builder.get_github_token().await.unwrap();
        assert_eq!(token, Some("gh_token_123".to_string()));
    }

    #[tokio::test]
    async fn test_build_context_with_permission_mode() {
        let mock = MockContextRepository::new().with_env("INPUT_PERMISSION_MODE", "read_only");
        let builder = create_builder(mock);
        let input = BuildContextInput::default();
        let output = builder.build(input).await.unwrap();
        assert_eq!(
            output.context.permission_mode,
            Some("read_only".to_string())
        );
    }
}
