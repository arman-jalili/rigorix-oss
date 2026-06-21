//! ModeResolver implementation — resolves execution mode from inputs and event context.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md#actionmode
//! Implements: ModeResolver trait — resolves ActionMode from INPUT_MODE, event type,
//!   and slash commands
//! Issue: issue-actionmode (#616)
//!
//! Resolution priority:
//! 1. Explicit INPUT_MODE (highest)
//! 2. Slash command in issue_comment
//! 3. Event type heuristic (pull_request → Validate, workflow_dispatch → Run, etc.)
//! 4. Fallback: Status (lowest)

use async_trait::async_trait;

use crate::action_entrypoint::domain::{ActionError, ActionMode};

use super::dto::{ResolveModeInput, ResolveModeOutput};
use super::service::ModeResolver;

/// Resolves the execution mode from workflow inputs and event context.
///
/// Stateless — can be shared across dispatches.
pub struct ModeResolverImpl;

impl ModeResolverImpl {
    /// Parse a `/rigorix <command> [args...]` slash command from comment body.
    fn parse_slash_command(comment_body: &str) -> Option<(String, Option<String>)> {
        // Look for lines starting with /rigorix
        for line in comment_body.lines() {
            let trimmed = line.trim();
            if let Some(args) = trimmed.strip_prefix("/rigorix ") {
                let mut parts = args
                    .splitn(2, char::is_whitespace)
                    .filter(|s| !s.is_empty());
                let command = parts.next()?.to_lowercase();
                let intent = parts
                    .next()
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty());
                return Some((command, intent));
            }
        }
        None
    }
}

#[async_trait]
impl ModeResolver for ModeResolverImpl {
    async fn resolve(&self, input: ResolveModeInput) -> Result<ResolveModeOutput, ActionError> {
        let mut warnings = Vec::new();

        // Priority 1: Explicit INPUT_MODE
        if let Some(ref mode_str) = input.input_mode {
            let mode_str_lower = mode_str.to_lowercase();
            match mode_str_lower.as_str() {
                "run" => {
                    let intent = input.input_intent.unwrap_or_default();
                    return Ok(ResolveModeOutput {
                        mode: ActionMode::Run { intent },
                        source: "input".to_string(),
                        unambiguous: true,
                        warnings,
                    });
                }
                "plan" => {
                    let intent = input.input_intent.unwrap_or_default();
                    return Ok(ResolveModeOutput {
                        mode: ActionMode::Plan { intent },
                        source: "input".to_string(),
                        unambiguous: true,
                        warnings,
                    });
                }
                "validate" => {
                    let intent = input.input_intent.unwrap_or_default();
                    return Ok(ResolveModeOutput {
                        mode: ActionMode::Validate { intent },
                        source: "input".to_string(),
                        unambiguous: true,
                        warnings,
                    });
                }
                "status" => {
                    return Ok(ResolveModeOutput {
                        mode: ActionMode::Status,
                        source: "input".to_string(),
                        unambiguous: true,
                        warnings,
                    });
                }
                "auto" => {
                    warnings.push(
                        "INPUT_MODE=auto: falling through to event-based resolution".to_string(),
                    );
                }
                _ => {
                    return Err(ActionError::ModeResolutionError {
                        detail: format!(
                            "Unknown INPUT_MODE value: '{mode_str}'. Valid values: run, plan, validate, status, auto"
                        ),
                        input_mode: Some(mode_str.clone()),
                        event_name: Some(input.event_name.clone()),
                    });
                }
            }
        }

        // Priority 2: Slash command in issue_comment event payload
        if input.event_name == "issue_comment" {
            if let Some(ref payload) = input.event_payload
                && let Some(comment_body) =
                    payload.pointer("/comment/body").and_then(|v| v.as_str())
                && let Some((command, intent)) = Self::parse_slash_command(comment_body)
            {
                return self
                    .resolve_from_command(&command, intent)
                    .await
                    .map(|mode| ResolveModeOutput {
                        mode,
                        source: "issue_comment_command".to_string(),
                        unambiguous: true,
                        warnings,
                    });
            }
            // No slash command found in issue_comment — fall through to event type
            warnings.push("No /rigorix command found in issue_comment".to_string());
        }

        // Priority 3: Event type heuristic
        let (mode, source) = match input.event_name.as_str() {
            "workflow_dispatch" | "repository_dispatch" => {
                let intent = input.input_intent.unwrap_or_default();
                (ActionMode::Run { intent }, "event_type".to_string())
            }
            "pull_request" | "pull_request_target" => {
                let intent = input.input_intent.unwrap_or_default();
                (ActionMode::Validate { intent }, "event_type".to_string())
            }
            "push" => (ActionMode::Status, "event_type".to_string()),
            "issue_comment" => {
                // No slash command found — default to Run
                let intent = input.input_intent.unwrap_or_default();
                (ActionMode::Run { intent }, "event_type".to_string())
            }
            _ => {
                warnings.push(format!(
                    "Unknown event type '{}': falling back to Status mode",
                    input.event_name
                ));
                (ActionMode::Status, "default".to_string())
            }
        };

        Ok(ResolveModeOutput {
            mode,
            source,
            unambiguous: true,
            warnings,
        })
    }

    async fn resolve_from_string(&self, mode_str: &str) -> Option<ActionMode> {
        match mode_str.to_lowercase().as_str() {
            "run" => Some(ActionMode::Run {
                intent: String::new(),
            }),
            "plan" => Some(ActionMode::Plan {
                intent: String::new(),
            }),
            "validate" => Some(ActionMode::Validate {
                intent: String::new(),
            }),
            "status" => Some(ActionMode::Status),
            "auto" => None,
            _ => None,
        }
    }

    async fn resolve_from_command(
        &self,
        command_type: &str,
        intent: Option<String>,
    ) -> Result<ActionMode, ActionError> {
        match command_type.to_lowercase().as_str() {
            "run" => Ok(ActionMode::Run {
                intent: intent.unwrap_or_default(),
            }),
            "validate" => Ok(ActionMode::Validate {
                intent: intent.unwrap_or_default(),
            }),
            "plan" => Ok(ActionMode::Plan {
                intent: intent.unwrap_or_default(),
            }),
            "status" => Ok(ActionMode::Status),
            "retry" => {
                // Retry maps to Run with the execution_id as intent context
                Ok(ActionMode::Run {
                    intent: intent.unwrap_or_default(),
                })
            }
            _ => {
                // Unknown command — default to Status with a warning
                Ok(ActionMode::Status)
            }
        }
    }

    async fn resolve_from_event(
        &self,
        event_type: &str,
        event_data: &serde_json::Value,
    ) -> Result<ActionMode, ActionError> {
        match event_type {
            "workflow_dispatch" | "repository_dispatch" => {
                // Extract intent from workflow_dispatch inputs if available
                let intent = event_data
                    .get("inputs")
                    .and_then(|i| i.get("intent"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok(ActionMode::Run { intent })
            }
            "pull_request" | "pull_request_target" => {
                // PR events default to Validate mode
                Ok(ActionMode::Validate {
                    intent: String::new(),
                })
            }
            "issue_comment" => {
                // Check for slash command in comment body
                if let Some(comment_body) =
                    event_data.pointer("/comment/body").and_then(|v| v.as_str())
                    && let Some((command, intent)) = Self::parse_slash_command(comment_body)
                {
                    return self.resolve_from_command(&command, intent).await;
                }
                // No slash command: dispatch as Run
                Ok(ActionMode::Run {
                    intent: String::new(),
                })
            }
            "push" => Ok(ActionMode::Status),
            _ => Err(ActionError::UnsupportedEvent {
                event_name: event_type.to_string(),
                detail: format!("No mode mapping for event type '{event_type}'"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolver() -> ModeResolverImpl {
        ModeResolverImpl
    }

    // ── resolve_from_string ──

    #[tokio::test]
    async fn test_resolve_from_string_run() {
        let mode = resolver().resolve_from_string("run").await;
        assert!(matches!(mode, Some(ActionMode::Run { .. })));
    }

    #[tokio::test]
    async fn test_resolve_from_string_plan() {
        let mode = resolver().resolve_from_string("plan").await;
        assert!(matches!(mode, Some(ActionMode::Plan { .. })));
    }

    #[tokio::test]
    async fn test_resolve_from_string_validate() {
        let mode = resolver().resolve_from_string("validate").await;
        assert!(matches!(mode, Some(ActionMode::Validate { .. })));
    }

    #[tokio::test]
    async fn test_resolve_from_string_status() {
        let mode = resolver().resolve_from_string("status").await;
        assert_eq!(mode, Some(ActionMode::Status));
    }

    #[tokio::test]
    async fn test_resolve_from_string_auto_returns_none() {
        let mode = resolver().resolve_from_string("auto").await;
        assert_eq!(mode, None);
    }

    #[tokio::test]
    async fn test_resolve_from_string_unknown_returns_none() {
        let mode = resolver().resolve_from_string("invalid_mode").await;
        assert_eq!(mode, None);
    }

    #[tokio::test]
    async fn test_resolve_from_string_case_insensitive() {
        let mode = resolver().resolve_from_string("RUN").await;
        assert!(matches!(mode, Some(ActionMode::Run { .. })));
    }

    // ── resolve_from_command ──

    #[tokio::test]
    async fn test_resolve_command_run() {
        let mode = resolver()
            .resolve_from_command("run", Some("implement X".to_string()))
            .await
            .unwrap();
        assert!(matches!(mode, ActionMode::Run { intent } if intent == "implement X"));
    }

    #[tokio::test]
    async fn test_resolve_command_validate() {
        let mode = resolver()
            .resolve_from_command("validate", None)
            .await
            .unwrap();
        assert!(matches!(mode, ActionMode::Validate { .. }));
    }

    #[tokio::test]
    async fn test_resolve_command_plan() {
        let mode = resolver()
            .resolve_from_command("plan", Some("plan Y".to_string()))
            .await
            .unwrap();
        assert!(matches!(mode, ActionMode::Plan { intent } if intent == "plan Y"));
    }

    #[tokio::test]
    async fn test_resolve_command_status() {
        let mode = resolver()
            .resolve_from_command("status", None)
            .await
            .unwrap();
        assert_eq!(mode, ActionMode::Status);
    }

    #[tokio::test]
    async fn test_resolve_command_retry() {
        let mode = resolver()
            .resolve_from_command("retry", Some("exec-123".to_string()))
            .await
            .unwrap();
        assert!(matches!(mode, ActionMode::Run { intent } if intent == "exec-123"));
    }

    #[tokio::test]
    async fn test_resolve_command_help_defaults_to_status() {
        let mode = resolver().resolve_from_command("help", None).await.unwrap();
        assert_eq!(mode, ActionMode::Status);
    }

    #[tokio::test]
    async fn test_resolve_command_unknown_defaults_to_status() {
        let mode = resolver()
            .resolve_from_command("unknown_cmd", None)
            .await
            .unwrap();
        assert_eq!(mode, ActionMode::Status);
    }

    // ── resolve_from_event ──

    #[tokio::test]
    async fn test_resolve_event_workflow_dispatch() {
        let event_data = serde_json::json!({
            "inputs": { "intent": "deploy to prod" }
        });
        let mode = resolver()
            .resolve_from_event("workflow_dispatch", &event_data)
            .await
            .unwrap();
        assert!(matches!(mode, ActionMode::Run { intent } if intent == "deploy to prod"));
    }

    #[tokio::test]
    async fn test_resolve_event_pull_request() {
        let event_data = serde_json::json!({});
        let mode = resolver()
            .resolve_from_event("pull_request", &event_data)
            .await
            .unwrap();
        assert!(matches!(mode, ActionMode::Validate { .. }));
    }

    #[tokio::test]
    async fn test_resolve_event_push() {
        let event_data = serde_json::json!({});
        let mode = resolver()
            .resolve_from_event("push", &event_data)
            .await
            .unwrap();
        assert_eq!(mode, ActionMode::Status);
    }

    #[tokio::test]
    async fn test_resolve_event_issue_comment_with_slash_command() {
        let event_data = serde_json::json!({
            "comment": {
                "body": "/rigorix run implement feature X",
                "user": { "login": "test-user" }
            }
        });
        let mode = resolver()
            .resolve_from_event("issue_comment", &event_data)
            .await
            .unwrap();
        assert!(matches!(mode, ActionMode::Run { .. }));
    }

    #[tokio::test]
    async fn test_resolve_event_issue_comment_no_slash_command() {
        let event_data = serde_json::json!({
            "comment": {
                "body": "This is a regular comment without a command",
                "user": { "login": "test-user" }
            }
        });
        let mode = resolver()
            .resolve_from_event("issue_comment", &event_data)
            .await
            .unwrap();
        assert!(matches!(mode, ActionMode::Run { .. }));
    }

    #[tokio::test]
    async fn test_resolve_event_unknown() {
        let event_data = serde_json::json!({});
        let result = resolver()
            .resolve_from_event("unknown_event", &event_data)
            .await;
        assert!(result.is_err());
    }

    // ── resolve (combined) ──

    #[tokio::test]
    async fn test_resolve_input_mode_run() {
        let input = ResolveModeInput {
            input_mode: Some("run".to_string()),
            event_name: "pull_request".to_string(),
            event_payload: None,
            input_intent: Some("fix bug".to_string()),
        };
        let output = resolver().resolve(input).await.unwrap();
        assert!(matches!(output.mode, ActionMode::Run { intent } if intent == "fix bug"));
        assert_eq!(output.source, "input");
    }

    #[tokio::test]
    async fn test_resolve_input_mode_overrides_event() {
        // Even though event is push -> Status, INPUT_MODE takes priority
        let input = ResolveModeInput {
            input_mode: Some("run".to_string()),
            event_name: "push".to_string(),
            event_payload: None,
            input_intent: None,
        };
        let output = resolver().resolve(input).await.unwrap();
        assert!(matches!(output.mode, ActionMode::Run { .. }));
        assert_eq!(output.source, "input");
    }

    #[tokio::test]
    async fn test_resolve_auto_falls_through() {
        let input = ResolveModeInput {
            input_mode: Some("auto".to_string()),
            event_name: "workflow_dispatch".to_string(),
            event_payload: None,
            input_intent: None,
        };
        let output = resolver().resolve(input).await.unwrap();
        assert!(matches!(output.mode, ActionMode::Run { .. }));
        assert_eq!(output.source, "event_type");
    }

    #[tokio::test]
    async fn test_resolve_pull_request_default_validate() {
        let input = ResolveModeInput {
            input_mode: None,
            event_name: "pull_request".to_string(),
            event_payload: None,
            input_intent: None,
        };
        let output = resolver().resolve(input).await.unwrap();
        assert!(matches!(output.mode, ActionMode::Validate { .. }));
    }

    #[tokio::test]
    async fn test_resolve_push_default_status() {
        let input = ResolveModeInput {
            input_mode: None,
            event_name: "push".to_string(),
            event_payload: None,
            input_intent: None,
        };
        let output = resolver().resolve(input).await.unwrap();
        assert_eq!(output.mode, ActionMode::Status);
    }

    #[tokio::test]
    async fn test_resolve_unknown_event_fallback_status() {
        let input = ResolveModeInput {
            input_mode: None,
            event_name: "unknown_event".to_string(),
            event_payload: None,
            input_intent: None,
        };
        let output = resolver().resolve(input).await.unwrap();
        assert_eq!(output.mode, ActionMode::Status);
    }

    #[tokio::test]
    async fn test_resolve_invalid_input_mode_errors() {
        let input = ResolveModeInput {
            input_mode: Some("invalid".to_string()),
            event_name: "workflow_dispatch".to_string(),
            event_payload: None,
            input_intent: None,
        };
        let result = resolver().resolve(input).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ActionError::ModeResolutionError { .. } => {}
            _ => panic!("Expected ModeResolutionError"),
        }
    }

    #[tokio::test]
    async fn test_resolve_issue_comment_with_slash_command() {
        let input = ResolveModeInput {
            input_mode: None,
            event_name: "issue_comment".to_string(),
            event_payload: Some(serde_json::json!({
                "comment": {
                    "body": "/rigorix validate check coverage",
                    "user": { "login": "bot" }
                }
            })),
            input_intent: None,
        };
        let output = resolver().resolve(input).await.unwrap();
        assert!(matches!(output.mode, ActionMode::Validate { .. }));
        assert_eq!(output.source, "issue_comment_command");
    }

    #[tokio::test]
    async fn test_resolve_issue_comment_no_command() {
        let input = ResolveModeInput {
            input_mode: None,
            event_name: "issue_comment".to_string(),
            event_payload: Some(serde_json::json!({
                "comment": {
                    "body": "Just a regular comment",
                    "user": { "login": "user" }
                }
            })),
            input_intent: None,
        };
        let output = resolver().resolve(input).await.unwrap();
        assert!(matches!(output.mode, ActionMode::Run { .. }));
        assert_eq!(output.source, "event_type");
    }

    // ── parse_slash_command ──

    #[test]
    fn test_parse_slash_command_run() {
        let result = ModeResolverImpl::parse_slash_command("/rigorix run implement feature");
        assert_eq!(
            result,
            Some(("run".to_string(), Some("implement feature".to_string())))
        );
    }

    #[test]
    fn test_parse_slash_command_validate() {
        let result = ModeResolverImpl::parse_slash_command("/rigorix validate test coverage");
        assert_eq!(
            result,
            Some(("validate".to_string(), Some("test coverage".to_string())))
        );
    }

    #[test]
    fn test_parse_slash_command_status() {
        let result = ModeResolverImpl::parse_slash_command("/rigorix status");
        assert_eq!(result, Some(("status".to_string(), None)));
    }

    #[test]
    fn test_parse_slash_command_no_command() {
        let result = ModeResolverImpl::parse_slash_command("Just a regular comment");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_slash_command_wrong_prefix() {
        let result = ModeResolverImpl::parse_slash_command("/other command");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_slash_command_multiline() {
        let comment = "Some discussion\n/rigorix run fix the bug\nMore text";
        let result = ModeResolverImpl::parse_slash_command(comment);
        assert_eq!(
            result,
            Some(("run".to_string(), Some("fix the bug".to_string())))
        );
    }

    #[test]
    fn test_parse_slash_command_empty_intent() {
        let result = ModeResolverImpl::parse_slash_command("/rigorix run ");
        assert_eq!(result, Some(("run".to_string(), None)));
    }
}
