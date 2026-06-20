//! Implementation of `CommentParsingService`.
//!
//! @canonical actions/.pi/architecture/modules/action-input.md#comment-parser
//! Implements: CommentParsingService trait — parses /rigorix slash commands
//! Issue: #524
//!
//! The CommentParser scans issue/PR comment bodies for `/rigorix <command> [args]`
//! patterns and returns structured `CommentCommand` values.
//!
//! # Supported Commands
//!
//! | Command | Format | Description |
//! |---------|--------|-------------|
//! | run | `/rigorix run <intent>` | Full Mode B execution |
//! | validate | `/rigorix validate <intent>` | Self-correcting validation loop |
//! | plan | `/rigorix plan <intent>` | Planning phase only |
//! | status | `/rigorix status` | Show execution status |
//! | retry | `/rigorix retry <execution_id>` | Retry a failed execution |
//! | help | `/rigorix <unknown>` | Show usage information |
//!
//! # Matching Rules
//!
//! - Case-sensitive prefix matching (`/rigorix`)
//! - Only checks the first line of the comment
//! - Leading/trailing whitespace is trimmed before matching
//! - Command word must be space-delimited after the prefix

use async_trait::async_trait;
use uuid::Uuid;

use crate::action_input::application::dto::{ParseCommentInput, ParseCommentOutput};
use crate::action_input::application::service::CommentParsingService;
use crate::action_input::domain::{ActionInputError, CommentCommand};

/// Implementation of `CommentParsingService`.
///
/// Parses `/rigorix` commands from comment text. Supports configurable
/// command prefix (default: `/rigorix`).
pub struct CommentParserImpl {
    /// The command prefix to look for (e.g., `/rigorix`).
    command_prefix: String,
}

impl CommentParserImpl {
    pub fn new(command_prefix: Option<String>) -> Self {
        Self {
            command_prefix: command_prefix.unwrap_or_else(|| "/rigorix".to_string()),
        }
    }

    /// Parse a comment string into a CommentCommand.
    ///
    /// Internal synchronous implementation shared by `parse` and `parse_and_validate`.
    fn parse_internal(&self, comment: &str) -> Option<CommentCommand> {
        let trimmed = comment.trim();

        // Check the first line only
        let first_line = trimmed.lines().next().unwrap_or("").trim();

        if !first_line.starts_with(&self.command_prefix) {
            return None;
        }

        // Extract the part after the prefix
        let after_prefix = first_line[self.command_prefix.len()..].trim();

        // Split into command word and args
        let parts: Vec<&str> = after_prefix.splitn(2, ' ').collect();
        let command_word = parts.first().copied().unwrap_or("");
        let args = parts.get(1).copied().unwrap_or("");

        match command_word {
            "run" => Some(CommentCommand::Run {
                intent: args.to_string(),
            }),
            "validate" => Some(CommentCommand::Validate {
                intent: args.to_string(),
            }),
            "plan" => Some(CommentCommand::Plan {
                intent: args.to_string(),
            }),
            "status" => Some(CommentCommand::Status),
            "retry" => Some(CommentCommand::Retry {
                execution_id: args.to_string(),
            }),
            "" => None, // Just "/rigorix" with no command
            _ => Some(CommentCommand::Help),
        }
    }
}

impl Default for CommentParserImpl {
    fn default() -> Self {
        Self::new(None)
    }
}

#[async_trait]
impl CommentParsingService for CommentParserImpl {
    async fn parse(
        &self,
        input: ParseCommentInput,
    ) -> Result<ParseCommentOutput, ActionInputError> {
        let effective_prefix = input
            .command_prefix
            .clone()
            .unwrap_or_else(|| self.command_prefix.clone());

        // Use our prefix for this parse, creating a temporary parser if needed
        let parser = if effective_prefix != self.command_prefix {
            CommentParserImpl::new(Some(effective_prefix))
        } else {
            CommentParserImpl {
                command_prefix: self.command_prefix.clone(),
            }
        };

        let command = parser.parse_internal(&input.comment_body);

        let command_type = command.as_ref().map(|c| match c {
            CommentCommand::Run { .. } => "run",
            CommentCommand::Validate { .. } => "validate",
            CommentCommand::Plan { .. } => "plan",
            CommentCommand::Status => "status",
            CommentCommand::Retry { .. } => "retry",
            CommentCommand::Help => "help",
        });

        Ok(ParseCommentOutput {
            found: command.is_some(),
            command,
            command_type: command_type.map(String::from),
        })
    }

    async fn parse_and_validate(
        &self,
        input: ParseCommentInput,
    ) -> Result<ParseCommentOutput, ActionInputError> {
        let output = self.parse(input).await?;

        // Validate: run/validate/plan must have non-empty intent
        if let Some(ref cmd) = output.command {
            match cmd {
                CommentCommand::Run { intent }
                | CommentCommand::Validate { intent }
                | CommentCommand::Plan { intent } => {
                    if intent.trim().is_empty() {
                        return Ok(ParseCommentOutput {
                            found: true,
                            command: Some(CommentCommand::Help),
                            command_type: Some("help".to_string()),
                        });
                    }
                }
                CommentCommand::Retry { execution_id } => {
                    if Uuid::parse_str(execution_id).is_err() {
                        return Ok(ParseCommentOutput {
                            found: true,
                            command: Some(CommentCommand::Help),
                            command_type: Some("help".to_string()),
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(output)
    }

    async fn has_command_prefix(&self, comment: &str) -> bool {
        comment.trim().starts_with(&self.command_prefix)
    }

    async fn extract_args(&self, comment: &str) -> Option<String> {
        let trimmed = comment.trim();
        let first_line = trimmed.lines().next().unwrap_or("").trim();

        if !first_line.starts_with(&self.command_prefix) {
            return None;
        }

        let after_prefix = first_line[self.command_prefix.len()..].trim();
        let parts: Vec<&str> = after_prefix.splitn(2, ' ').collect();

        if parts.len() >= 2 {
            Some(parts[1].to_string())
        } else {
            Some(String::new())
        }
    }

    async fn validate_permission(
        &self,
        commenter: &str,
        command: &CommentCommand,
    ) -> Result<bool, ActionInputError> {
        // By default, only allow `status` and `help` for non-authenticated users.
        // `run`, `validate`, `plan`, `retry` require appropriate permissions.
        //
        // Full permission checking requires GitHub API access and is delegated
        // to the action-entrypoint module. This basic check prevents accidental
        // command invocation.
        Ok(match command {
            CommentCommand::Status | CommentCommand::Help => true,
            CommentCommand::Run { .. }
            | CommentCommand::Validate { .. }
            | CommentCommand::Plan { .. }
            | CommentCommand::Retry { .. } => !commenter.is_empty(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_parser() -> CommentParserImpl {
        CommentParserImpl::default()
    }

    #[tokio::test]
    async fn test_parse_run_command() {
        let parser = make_parser();
        let input = ParseCommentInput {
            comment_body: "/rigorix run implement user auth".to_string(),
            command_prefix: None,
            issue_number: 42,
            commenter: "test-user".to_string(),
        };
        let result = parser.parse(input).await.unwrap();
        assert!(result.found);
        assert_eq!(
            result.command,
            Some(CommentCommand::Run {
                intent: "implement user auth".to_string()
            })
        );
        assert_eq!(result.command_type, Some("run".to_string()));
    }

    #[tokio::test]
    async fn test_parse_validate_command() {
        let parser = make_parser();
        let input = ParseCommentInput {
            comment_body: "/rigorix validate check formatting".to_string(),
            command_prefix: None,
            issue_number: 42,
            commenter: "test-user".to_string(),
        };
        let result = parser.parse(input).await.unwrap();
        assert!(result.found);
        assert_eq!(
            result.command,
            Some(CommentCommand::Validate {
                intent: "check formatting".to_string()
            })
        );
    }

    #[tokio::test]
    async fn test_parse_plan_command() {
        let parser = make_parser();
        let input = ParseCommentInput {
            comment_body: "/rigorix plan design api".to_string(),
            command_prefix: None,
            issue_number: 42,
            commenter: "test-user".to_string(),
        };
        let result = parser.parse(input).await.unwrap();
        assert!(result.found);
        assert_eq!(
            result.command,
            Some(CommentCommand::Plan {
                intent: "design api".to_string()
            })
        );
    }

    #[tokio::test]
    async fn test_parse_status_command() {
        let parser = make_parser();
        let input = ParseCommentInput {
            comment_body: "/rigorix status".to_string(),
            command_prefix: None,
            issue_number: 42,
            commenter: "test-user".to_string(),
        };
        let result = parser.parse(input).await.unwrap();
        assert!(result.found);
        assert_eq!(result.command, Some(CommentCommand::Status));
    }

    #[tokio::test]
    async fn test_parse_retry_command() {
        let parser = make_parser();
        let id = "123e4567-e89b-12d3-a456-426614174000";
        let input = ParseCommentInput {
            comment_body: format!("/rigorix retry {}", id),
            command_prefix: None,
            issue_number: 42,
            commenter: "test-user".to_string(),
        };
        let result = parser.parse(input).await.unwrap();
        assert!(result.found);
        assert_eq!(
            result.command,
            Some(CommentCommand::Retry {
                execution_id: id.to_string()
            })
        );
    }

    #[tokio::test]
    async fn test_parse_help_for_unknown_command() {
        let parser = make_parser();
        let input = ParseCommentInput {
            comment_body: "/rigorix something".to_string(),
            command_prefix: None,
            issue_number: 42,
            commenter: "test-user".to_string(),
        };
        let result = parser.parse(input).await.unwrap();
        assert!(result.found);
        assert_eq!(result.command, Some(CommentCommand::Help));
    }

    #[tokio::test]
    async fn test_parse_no_command() {
        let parser = make_parser();
        let input = ParseCommentInput {
            comment_body: "This is just a regular comment".to_string(),
            command_prefix: None,
            issue_number: 42,
            commenter: "test-user".to_string(),
        };
        let result = parser.parse(input).await.unwrap();
        assert!(!result.found);
        assert_eq!(result.command, None);
    }

    #[tokio::test]
    async fn test_parse_just_prefix_no_command() {
        let parser = make_parser();
        let input = ParseCommentInput {
            comment_body: "/rigorix".to_string(),
            command_prefix: None,
            issue_number: 42,
            commenter: "test-user".to_string(),
        };
        let result = parser.parse(input).await.unwrap();
        assert!(!result.found);
        assert_eq!(result.command, None);
    }

    #[tokio::test]
    async fn test_parse_first_line_only() {
        let parser = make_parser();
        let input = ParseCommentInput {
            comment_body: "/rigorix status\n/rigorix run second line".to_string(),
            command_prefix: None,
            issue_number: 42,
            commenter: "test-user".to_string(),
        };
        let result = parser.parse(input).await.unwrap();
        assert!(result.found);
        assert_eq!(result.command, Some(CommentCommand::Status));
    }

    #[tokio::test]
    async fn test_parse_custom_prefix() {
        let parser = CommentParserImpl::new(Some("!bot".to_string()));
        let input = ParseCommentInput {
            comment_body: "!bot run do something".to_string(),
            command_prefix: None,
            issue_number: 42,
            commenter: "test-user".to_string(),
        };
        let result = parser.parse(input).await.unwrap();
        assert!(result.found);
        assert_eq!(
            result.command,
            Some(CommentCommand::Run {
                intent: "do something".to_string()
            })
        );
    }

    #[tokio::test]
    async fn test_parse_with_override_prefix() {
        let parser = make_parser(); // Default prefix is /rigorix
        let input = ParseCommentInput {
            comment_body: "!custom run hello".to_string(),
            command_prefix: Some("!custom".to_string()),
            issue_number: 42,
            commenter: "test-user".to_string(),
        };
        let result = parser.parse(input).await.unwrap();
        assert!(result.found);
        assert_eq!(
            result.command,
            Some(CommentCommand::Run {
                intent: "hello".to_string()
            })
        );
    }

    #[tokio::test]
    async fn test_parse_whitespace_handling() {
        let parser = make_parser();
        let input = ParseCommentInput {
            comment_body: "  /rigorix run   clean up   spaces  ".to_string(),
            command_prefix: None,
            issue_number: 42,
            commenter: "test-user".to_string(),
        };
        let result = parser.parse(input).await.unwrap();
        assert!(result.found);
        assert_eq!(
            result.command,
            Some(CommentCommand::Run {
                intent: "  clean up   spaces".to_string()
            })
        );
    }

    // ── parse_and_validate tests ──

    #[tokio::test]
    async fn test_parse_and_validate_run_with_empty_intent() {
        let parser = make_parser();
        let input = ParseCommentInput {
            comment_body: "/rigorix run".to_string(),
            command_prefix: None,
            issue_number: 42,
            commenter: "test-user".to_string(),
        };
        let result = parser.parse_and_validate(input).await.unwrap();
        assert!(result.found);
        assert_eq!(result.command, Some(CommentCommand::Help));
    }

    #[tokio::test]
    async fn test_parse_and_validate_retry_with_bad_uuid() {
        let parser = make_parser();
        let input = ParseCommentInput {
            comment_body: "/rigorix retry not-a-uuid".to_string(),
            command_prefix: None,
            issue_number: 42,
            commenter: "test-user".to_string(),
        };
        let result = parser.parse_and_validate(input).await.unwrap();
        assert!(result.found);
        assert_eq!(result.command, Some(CommentCommand::Help));
    }

    #[tokio::test]
    async fn test_parse_and_validate_retry_with_valid_uuid() {
        let parser = make_parser();
        let id = "123e4567-e89b-12d3-a456-426614174000";
        let input = ParseCommentInput {
            comment_body: format!("/rigorix retry {}", id),
            command_prefix: None,
            issue_number: 42,
            commenter: "test-user".to_string(),
        };
        let result = parser.parse_and_validate(input).await.unwrap();
        assert!(result.found);
        assert_eq!(
            result.command,
            Some(CommentCommand::Retry {
                execution_id: id.to_string()
            })
        );
    }

    #[tokio::test]
    async fn test_parse_and_validate_status_always_valid() {
        let parser = make_parser();
        let input = ParseCommentInput {
            comment_body: "/rigorix status".to_string(),
            command_prefix: None,
            issue_number: 42,
            commenter: "test-user".to_string(),
        };
        let result = parser.parse_and_validate(input).await.unwrap();
        assert!(result.found);
        assert_eq!(result.command, Some(CommentCommand::Status));
    }

    // ── has_command_prefix tests ──

    #[tokio::test]
    async fn test_has_command_prefix_true() {
        let parser = make_parser();
        assert!(parser.has_command_prefix("/rigorix run test").await);
    }

    #[tokio::test]
    async fn test_has_command_prefix_false() {
        let parser = make_parser();
        assert!(!parser
            .has_command_prefix("just a regular comment")
            .await);
    }

    #[tokio::test]
    async fn test_has_command_prefix_whitespace() {
        let parser = make_parser();
        assert!(parser.has_command_prefix("  /rigorix status").await);
    }

    // ── extract_args tests ──

    #[tokio::test]
    async fn test_extract_args_with_args() {
        let parser = make_parser();
        let result = parser.extract_args("/rigorix run implement feature").await;
        assert_eq!(result, Some("implement feature".to_string()));
    }

    #[tokio::test]
    async fn test_extract_args_no_args() {
        let parser = make_parser();
        let result = parser.extract_args("/rigorix status").await;
        assert_eq!(result, Some(String::new()));
    }

    #[tokio::test]
    async fn test_extract_args_no_prefix() {
        let parser = make_parser();
        let result = parser.extract_args("regular comment").await;
        assert_eq!(result, None);
    }

    // ── validate_permission tests ──

    #[tokio::test]
    async fn test_validate_permission_status_always_allowed() {
        let parser = make_parser();
        assert!(parser
            .validate_permission("", &CommentCommand::Status)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_validate_permission_run_requires_commenter() {
        let parser = make_parser();
        assert!(!parser
            .validate_permission("", &CommentCommand::Run {
                intent: "test".to_string()
            })
            .await
            .unwrap());
        assert!(parser
            .validate_permission("authorized-user", &CommentCommand::Run {
                intent: "test".to_string()
            })
            .await
            .unwrap());
    }
}
