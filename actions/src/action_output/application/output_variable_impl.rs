//! Implementation of `OutputVariableService`.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md#outputvariablewriter
//! Implements: OutputVariableService — sets `$GITHUB_OUTPUT` variables for downstream steps
//! Issue: issue-outputformatter
//!
//! # Contract
//! - Implements `OutputVariableService` trait from the frozen contract
//! - Writes `name=value` pairs to `$GITHUB_OUTPUT` via `OutputRepository`
//! - Validates variable names against `[a-z_][a-z0-9_]*`

use async_trait::async_trait;
use regex::Regex;
use tracing::info;

use crate::action_output::domain::ActionOutputError;

use super::dto::{
    SetOutputVariablesInput, SetOutputVariablesOutput, SetVariableInput, SetVariableOutput,
};
use super::service::OutputVariableService;
use crate::action_output::infrastructure::repository::OutputRepository;

/// Default implementation of `OutputVariableService`.
///
/// Sets GitHub Actions output variables for downstream workflow steps
/// by writing `name=value` pairs to the file at `$GITHUB_OUTPUT`.
///
/// # Variable Name Validation
/// Names must match `[a-z_][a-z0-9_]*` (lowercase alphanumeric + underscores).
///
/// # Value Sanitization
/// - Newlines are stripped to prevent injection across steps
/// - Values are capped at 10KB
///
/// # Dependencies
/// - `OutputRepository` — for writing to `$GITHUB_OUTPUT`
pub struct OutputVariableServiceImpl {
    output_repo: Box<dyn OutputRepository>,
    name_regex: Regex,
    max_value_length: usize,
}

impl OutputVariableServiceImpl {
    /// Create a new `OutputVariableServiceImpl`.
    pub fn new(output_repo: Box<dyn OutputRepository>) -> Self {
        Self {
            output_repo,
            name_regex: Regex::new(r"^[a-z_][a-z0-9_]*$").unwrap(),
            max_value_length: 10_240, // 10KB default
        }
    }

    /// Validate a variable name against `[a-z_][a-z0-9_]*`.
    fn validate_name(&self, name: &str) -> Result<(), ActionOutputError> {
        if !self.name_regex.is_match(name) {
            return Err(ActionOutputError::InvalidVariableName {
                name: name.to_string(),
            });
        }
        Ok(())
    }

    /// Sanitize a value: strip newlines, cap length.
    fn sanitize_value(&self, value: &str) -> String {
        let sanitized: String = value.chars().filter(|&c| c != '\n' && c != '\r').collect();
        if sanitized.len() > self.max_value_length {
            sanitized[..self.max_value_length].to_string()
        } else {
            sanitized
        }
    }
}

#[async_trait]
impl OutputVariableService for OutputVariableServiceImpl {
    async fn set_variable(
        &self,
        input: SetVariableInput,
    ) -> Result<SetVariableOutput, ActionOutputError> {
        self.validate_name(&input.name)?;

        let max_len = input.max_length.unwrap_or(self.max_value_length);
        let sanitized = self.sanitize_value(&input.value);

        if sanitized.len() > max_len {
            return Err(ActionOutputError::VariableTooLong {
                name: input.name.clone(),
                actual_length: sanitized.len(),
                max_length: max_len,
            });
        }

        let bytes = self
            .output_repo
            .write_output_variable(&input.name, &sanitized)
            .await?;

        info!(
            variable_name = %input.name,
            value_length = sanitized.len(),
            bytes_written = bytes,
            "output variable set"
        );

        Ok(SetVariableOutput {
            bytes_written: bytes,
        })
    }

    async fn set_from_context(
        &self,
        input: SetOutputVariablesInput,
    ) -> Result<SetOutputVariablesOutput, ActionOutputError> {
        let context = &input.context;
        let mut names = Vec::new();

        let variables = [
            ("execution_id", context.execution_id.to_string()),
            ("status", context.status.as_str().to_string()),
            ("iterations", context.iterations.to_string()),
            ("failure_count", context.failure_count.to_string()),
            ("cumulative_tokens", context.cumulative_tokens.to_string()),
            ("duration_ms", context.duration_ms.to_string()),
        ];

        for (name, value) in &variables {
            self.set_variable(SetVariableInput {
                name: name.to_string(),
                value: value.to_string(),
                max_length: None,
            })
            .await?;
            names.push(name.to_string());
        }

        if let Some(ref template_id) = context.template_id {
            self.set_variable(SetVariableInput {
                name: "template_id".to_string(),
                value: template_id.clone(),
                max_length: None,
            })
            .await?;
            names.push("template_id".to_string());
        }

        if let Some(ref quality) = context.quality_level {
            self.set_variable(SetVariableInput {
                name: "quality_level".to_string(),
                value: quality.clone(),
                max_length: None,
            })
            .await?;
            names.push("quality_level".to_string());
        }

        info!(
            variable_count = names.len(),
            "output variables set from context"
        );

        Ok(SetOutputVariablesOutput {
            variable_count: names.len() as u32,
            variable_names: names,
        })
    }

    async fn is_available(&self) -> bool {
        self.output_repo
            .get_output_path()
            .await
            .ok()
            .flatten()
            .is_some()
    }

    async fn get_output_path(&self) -> Result<String, ActionOutputError> {
        self.output_repo
            .get_output_path()
            .await?
            .ok_or_else(|| ActionOutputError::MissingEnv("GITHUB_OUTPUT".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct MockOutputRepo {
        written: Arc<Mutex<Vec<(String, String)>>>,
        output_path: Option<String>,
    }

    impl MockOutputRepo {
        fn new() -> Self {
            Self {
                written: Arc::new(Mutex::new(Vec::new())),
                output_path: Some("/tmp/test-output".to_string()),
            }
        }

        fn no_path() -> Self {
            Self {
                written: Arc::new(Mutex::new(Vec::new())),
                output_path: None,
            }
        }

        fn get_written(&self) -> Vec<(String, String)> {
            self.written.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl OutputRepository for MockOutputRepo {
        async fn write_stdout(&self, _content: &str) -> Result<u64, ActionOutputError> {
            Ok(0)
        }
        async fn write_output_variable(
            &self,
            name: &str,
            value: &str,
        ) -> Result<u64, ActionOutputError> {
            self.written
                .lock()
                .unwrap()
                .push((name.to_string(), value.to_string()));
            Ok((name.len() + 1 + value.len() + 1) as u64) // "name=value\n"
        }
        async fn append_summary(&self, _markdown: &str) -> Result<u64, ActionOutputError> {
            Ok(0)
        }
        async fn overwrite_summary(&self, _markdown: &str) -> Result<u64, ActionOutputError> {
            Ok(0)
        }
        async fn get_output_path(&self) -> Result<Option<String>, ActionOutputError> {
            Ok(self.output_path.clone())
        }
        async fn get_summary_path(&self) -> Result<Option<String>, ActionOutputError> {
            Ok(None)
        }
        async fn is_github_actions(&self) -> bool {
            true
        }
    }

    fn make_service(repo: MockOutputRepo) -> (OutputVariableServiceImpl, MockOutputRepo) {
        let service = OutputVariableServiceImpl::new(Box::new(repo.clone()));
        (service, repo)
    }

    #[tokio::test]
    async fn test_set_variable() {
        let (svc, repo) = make_service(MockOutputRepo::new());
        let result = svc
            .set_variable(SetVariableInput {
                name: "execution_id".to_string(),
                value: "abc-123".to_string(),
                max_length: None,
            })
            .await;
        assert!(result.is_ok());

        let written = repo.get_written();
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].0, "execution_id");
        assert_eq!(written[0].1, "abc-123");
    }

    #[tokio::test]
    async fn test_set_variable_invalid_name() {
        let (svc, _repo) = make_service(MockOutputRepo::new());
        let result = svc
            .set_variable(SetVariableInput {
                name: "INVALID-NAME".to_string(),
                value: "test".to_string(),
                max_length: None,
            })
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ActionOutputError::InvalidVariableName { .. }
        ));
    }

    #[tokio::test]
    async fn test_set_variable_too_long() {
        let (svc, _repo) = make_service(MockOutputRepo::new());
        let result = svc
            .set_variable(SetVariableInput {
                name: "x".to_string(),
                value: "a".repeat(20),
                max_length: Some(10),
            })
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ActionOutputError::VariableTooLong { .. }
        ));
    }

    #[tokio::test]
    async fn test_sanitize_strips_newlines() {
        let (svc, repo) = make_service(MockOutputRepo::new());
        svc.set_variable(SetVariableInput {
            name: "key".to_string(),
            value: "line1\nline2\rline3".to_string(),
            max_length: None,
        })
        .await
        .unwrap();

        let written = repo.get_written();
        assert_eq!(written[0].1, "line1line2line3");
    }

    #[tokio::test]
    async fn test_set_from_context() {
        use crate::action_output::domain::{ExecutionContext, ExecutionStatus};
        use std::collections::HashMap;
        use uuid::Uuid;

        let (svc, _repo) = make_service(MockOutputRepo::new());
        let context = ExecutionContext {
            execution_id: Uuid::parse_str("e1852176-e586-4377-a8e8-d1cb4be89144").unwrap(),
            status: ExecutionStatus::Completed,
            iterations: 2,
            max_iterations: 3,
            cumulative_tokens: 3240,
            duration_ms: 12400,
            quality_level: Some("workspace".to_string()),
            template_id: Some("tpl-1".to_string()),
            failure_count: 0,
            file_changes: vec![],
            execution_steps: vec![],
            metadata: HashMap::new(),
        };

        let result = svc
            .set_from_context(SetOutputVariablesInput { context })
            .await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.variable_count, 8); // 6 base + template_id + quality_level
        assert!(output.variable_names.contains(&"execution_id".to_string()));
        assert!(output.variable_names.contains(&"template_id".to_string()));
    }

    #[tokio::test]
    async fn test_is_available() {
        let (svc, _) = make_service(MockOutputRepo::new());
        assert!(svc.is_available().await);

        let no_path = OutputVariableServiceImpl::new(Box::new(MockOutputRepo::no_path()));
        assert!(!no_path.is_available().await);
    }

    #[tokio::test]
    async fn test_get_output_path() {
        let (svc, _) = make_service(MockOutputRepo::new());
        let path = svc.get_output_path().await.unwrap();
        assert_eq!(path, "/tmp/test-output");
    }
}
