//! Implementation of `AnnotationWritingService`.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md#annotationwriter
//! Implements: AnnotationWritingService — GitHub workflow annotation emission via stdout
//! Issue: issue-annotationwriter
//!
//! # Contract
//! - Implements `AnnotationWritingService` trait from the frozen contract
//! - Emits workflow commands to stdout in GitHub Actions format
//! - Supports levels: error, warning, notice

use async_trait::async_trait;

use crate::action_output::domain::{
    ActionOutputError, OutputLevel, WorkflowAnnotation,
};

use super::dto::{
    FormatAnnotationInput, FormatAnnotationOutput, WriteAnnotationInput, WriteAnnotationOutput,
};
use super::service::AnnotationWritingService;
use crate::action_output::infrastructure::repository::OutputRepository;

/// Default implementation of `AnnotationWritingService`.
///
/// Emits GitHub Actions workflow annotations by writing workflow commands
/// to stdout. The GitHub Actions runner parses these commands and renders
/// them as inline annotations in the UI.
///
/// # Workflow Command Format
///
/// ```text
/// ::<level> file=<path>,line=<line>,col=<col>,title=<title>::<message>
/// ```
///
/// # Level Mapping
///
/// | OutputLevel | Workflow Command | UI Behavior |
/// |-------------|-----------------|-------------|
/// | Error       | `::error`       | Fails the step |
/// | Warning     | `::warning`     | Warning annotation |
/// | Notice      | `::notice`      | Notice annotation |
/// | Debug       | (traced, no annotation) | Logged via tracing |
///
/// # Dependencies
/// - `OutputRepository` — for writing to stdout
///
/// # Construction
/// Use `AnnotationWriterImpl::new(output_repo)`.
pub struct AnnotationWriterImpl {
    output_repo: Box<dyn OutputRepository>,
}

impl AnnotationWriterImpl {
    /// Create a new `AnnotationWriterImpl`.
    pub fn new(output_repo: Box<dyn OutputRepository>) -> Self {
        Self { output_repo }
    }

    /// Render a `WorkflowAnnotation` into a GitHub Actions workflow command string.
    ///
    /// Format: `::<level> file=<path>,line=<line>,col=<col>,title=<title>::<message>`
    ///
    /// Fields are escaped according to GitHub Actions conventions:
    /// - `%` → `%25`
    /// - `\n` → `%0A`
    /// - `\r` → `%0D`
    fn render_workflow_command(&self, annotation: &WorkflowAnnotation) -> String {
        let level_str = match annotation.level {
            OutputLevel::Error => "error",
            OutputLevel::Warning => "warning",
            OutputLevel::Notice => "notice",
            OutputLevel::Debug => return String::new(), // Debug emits no annotation
        };

        let mut params = format!("file={},line={}", annotation.file, annotation.line);

        if let Some(col) = annotation.column {
            params.push_str(&format!(",col={}", col));
        }

        if let Some(ref title) = annotation.title {
            params.push_str(&format!(",title={}", self.escape(title)));
        }

        let message = self.escape(&annotation.message);

        format!("::{} {}::{}", level_str, params, message)
    }

    /// Escape special characters for GitHub Actions workflow commands.
    ///
    /// GitHub Actions requires these characters to be percent-encoded:
    /// - `%` → `%25`
    /// - `\n` → `%0A`
    /// - `\r` → `%0D`
    fn escape(&self, input: &str) -> String {
        input
            .replace('%', "%25")
            .replace('\n', "%0A")
            .replace('\r', "%0D")
    }
}

#[async_trait]
impl AnnotationWritingService for AnnotationWriterImpl {
    async fn write_annotation(
        &self,
        input: WriteAnnotationInput,
    ) -> Result<WriteAnnotationOutput, ActionOutputError> {
        let command = self.render_workflow_command(&input.annotation);

        if command.is_empty() {
            // Debug level or no-op annotations are silently ignored
            return Ok(WriteAnnotationOutput { bytes_written: 0 });
        }

        // Append newline for the runner to detect the command boundary
        let output = format!("{}\n", command);
        let bytes = self.output_repo.write_stdout(&output).await?;

        Ok(WriteAnnotationOutput { bytes_written: bytes })
    }

    async fn format_annotation(
        &self,
        input: FormatAnnotationInput,
    ) -> Result<FormatAnnotationOutput, ActionOutputError> {
        let level = match input.failure_type.to_lowercase().as_str() {
            "error" | "compile_error" | "assertion_failure" | "test_failure" => OutputLevel::Error,
            "warning" | "deprecation" => OutputLevel::Warning,
            _ => OutputLevel::Notice,
        };

        let file = input.file.unwrap_or_else(|| "<unknown>".to_string());
        let line = input.line.unwrap_or(1);

        let mut title = None;
        let mut message = input.context.clone();

        // If there's structured details, include them in the message
        if let Some(ref details) = input.details {
            if let Some(detail_msg) = details.get("message").and_then(|v| v.as_str()) {
                message = detail_msg.to_string();
            }
            if let Some(detail_title) = details.get("title").and_then(|v| v.as_str()) {
                title = Some(detail_title.to_string());
            }
        }

        let annotation = WorkflowAnnotation {
            level,
            file,
            line,
            column: None,
            title,
            message,
        };

        let workflow_command = self.render_workflow_command(&annotation);

        Ok(FormatAnnotationOutput {
            annotation,
            workflow_command,
        })
    }

    async fn write_annotations(
        &self,
        annotations: &[WorkflowAnnotation],
    ) -> Result<u32, ActionOutputError> {
        let mut count = 0u32;
        for annotation in annotations {
            let input = WriteAnnotationInput {
                annotation: annotation.clone(),
            };
            self.write_annotation(input).await?;
            count += 1;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action_output::domain::WorkflowAnnotation;
    use std::sync::{Arc, Mutex};

    // ── Mock OutputRepository ──

    #[derive(Clone)]
    struct MockOutputRepository {
        written: Arc<Mutex<Vec<String>>>,
    }

    impl MockOutputRepository {
        fn new() -> Self {
            Self {
                written: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_written(&self) -> Vec<String> {
            self.written.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl OutputRepository for MockOutputRepository {
        async fn write_stdout(&self, content: &str) -> Result<u64, ActionOutputError> {
            self.written.lock().unwrap().push(content.to_string());
            Ok(content.len() as u64)
        }

        async fn write_output_variable(&self, _name: &str, _value: &str) -> Result<u64, ActionOutputError> {
            Ok(0)
        }

        async fn append_summary(&self, _markdown: &str) -> Result<u64, ActionOutputError> {
            Ok(0)
        }

        async fn overwrite_summary(&self, _markdown: &str) -> Result<u64, ActionOutputError> {
            Ok(0)
        }

        async fn get_output_path(&self) -> Result<Option<String>, ActionOutputError> {
            Ok(None)
        }

        async fn get_summary_path(&self) -> Result<Option<String>, ActionOutputError> {
            Ok(None)
        }

        async fn is_github_actions(&self) -> bool {
            true
        }
    }

    // ── Helpers ──

    fn make_writer() -> AnnotationWriterImpl {
        AnnotationWriterImpl::new(Box::new(MockOutputRepository::new()))
    }

    fn make_writer_with_repo(repo: MockOutputRepository) -> (AnnotationWriterImpl, MockOutputRepository) {
        let writer = AnnotationWriterImpl::new(Box::new(repo.clone()));
        (writer, repo)
    }

    // ── Tests ──

    #[tokio::test]
    async fn test_write_error_annotation() {
        let repo = MockOutputRepository::new();
        let (writer, repo) = make_writer_with_repo(repo);

        let annotation = WorkflowAnnotation::error("src/main.rs", 42, "Symbol 'foo' not found");

        let input = WriteAnnotationInput { annotation };
        let result = writer.write_annotation(input).await;
        assert!(result.is_ok());

        let written = repo.get_written();
        assert_eq!(written.len(), 1);
        assert!(written[0].contains("::error file=src/main.rs,line=42::"));
        assert!(written[0].contains("Symbol 'foo' not found"));
    }

    #[tokio::test]
    async fn test_write_warning_annotation() {
        let repo = MockOutputRepository::new();
        let (writer, repo) = make_writer_with_repo(repo);

        let annotation = WorkflowAnnotation::warning("src/lib.rs", 10, "Deprecated function used");

        let input = WriteAnnotationInput { annotation };
        let result = writer.write_annotation(input).await;
        assert!(result.is_ok());

        let written = repo.get_written();
        assert_eq!(written.len(), 1);
        assert!(written[0].contains("::warning file=src/lib.rs,line=10::"));
    }

    #[tokio::test]
    async fn test_write_notice_annotation() {
        let repo = MockOutputRepository::new();
        let (writer, repo) = make_writer_with_repo(repo);

        let annotation = WorkflowAnnotation::notice("README.md", 1, "Consider adding documentation");

        let input = WriteAnnotationInput { annotation };
        let result = writer.write_annotation(input).await;
        assert!(result.is_ok());

        let written = repo.get_written();
        assert_eq!(written.len(), 1);
        assert!(written[0].contains("::notice file=README.md,line=1::"));
    }

    #[tokio::test]
    async fn test_write_annotation_with_column() {
        let repo = MockOutputRepository::new();
        let (writer, repo) = make_writer_with_repo(repo);

        let annotation = WorkflowAnnotation {
            level: OutputLevel::Error,
            file: "src/main.rs".to_string(),
            line: 42,
            column: Some(10),
            title: None,
            message: "Unexpected token".to_string(),
        };

        let input = WriteAnnotationInput { annotation };
        let result = writer.write_annotation(input).await;
        assert!(result.is_ok());

        let written = repo.get_written();
        assert!(written[0].contains("col=10::"));
    }

    #[tokio::test]
    async fn test_write_annotation_with_title() {
        let repo = MockOutputRepository::new();
        let (writer, repo) = make_writer_with_repo(repo);

        let annotation = WorkflowAnnotation {
            level: OutputLevel::Error,
            file: "src/main.rs".to_string(),
            line: 42,
            column: None,
            title: Some("Compile Error".to_string()),
            message: "Type mismatch".to_string(),
        };

        let input = WriteAnnotationInput { annotation };
        writer.write_annotation(input).await.unwrap();

        let written = repo.get_written();
        assert!(written[0].contains("title=Compile Error::"));
    }

    #[tokio::test]
    async fn test_debug_level_produces_no_output() {
        let repo = MockOutputRepository::new();
        let (writer, repo) = make_writer_with_repo(repo);

        let annotation = WorkflowAnnotation {
            level: OutputLevel::Debug,
            file: "src/main.rs".to_string(),
            line: 1,
            column: None,
            title: None,
            message: "Debug info".to_string(),
        };

        let input = WriteAnnotationInput { annotation };
        let result = writer.write_annotation(input).await.unwrap();
        assert_eq!(result.bytes_written, 0);

        let written = repo.get_written();
        assert_eq!(written.len(), 0);
    }

    #[tokio::test]
    async fn test_special_characters_are_escaped() {
        let repo = MockOutputRepository::new();
        let (writer, repo) = make_writer_with_repo(repo);

        let annotation = WorkflowAnnotation::error(
            "src/main.rs",
            1,
            "100% done\nnew line\r",
        );

        let input = WriteAnnotationInput { annotation };
        writer.write_annotation(input).await.unwrap();

        let written = repo.get_written();
        assert!(written[0].contains("100%25 done"));
        assert!(written[0].contains("%0Anew line"));
        assert!(written[0].contains("%0D"));
    }

    #[tokio::test]
    async fn test_write_annotations_batch() {
        let repo = MockOutputRepository::new();
        let (writer, repo) = make_writer_with_repo(repo);

        let annotations = vec![
            WorkflowAnnotation::error("src/main.rs", 42, "Symbol not found"),
            WorkflowAnnotation::warning("src/lib.rs", 10, "Deprecated"),
            WorkflowAnnotation::notice("README.md", 1, "Add docs"),
        ];

        let count = writer.write_annotations(&annotations).await.unwrap();
        assert_eq!(count, 3);

        let written = repo.get_written();
        assert_eq!(written.len(), 3);
    }

    #[tokio::test]
    async fn test_format_annotation_from_input() {
        let writer = make_writer();

        let input = FormatAnnotationInput {
            context: "Symbol 'foo' not found in scope".to_string(),
            failure_type: "error".to_string(),
            file: Some("src/main.rs".to_string()),
            line: Some(42),
            details: None,
        };

        let result = writer.format_annotation(input).await.unwrap();
        assert_eq!(result.annotation.level, OutputLevel::Error);
        assert_eq!(result.annotation.file, "src/main.rs");
        assert_eq!(result.annotation.line, 42);
        assert!(result.workflow_command.contains("::error file=src/main.rs,line=42::"));
    }

    #[tokio::test]
    async fn test_format_annotation_with_details() {
        let writer = make_writer();

        let input = FormatAnnotationInput {
            context: "test".to_string(),
            failure_type: "compile_error".to_string(),
            file: Some("src/lib.rs".to_string()),
            line: Some(15),
            details: Some(serde_json::json!({
                "message": "Expected ';' found '.'",
                "title": "Syntax Error",
            })),
        };

        let result = writer.format_annotation(input).await.unwrap();
        assert_eq!(result.annotation.message, "Expected ';' found '.'");
        assert_eq!(result.annotation.title, Some("Syntax Error".to_string()));
    }

    #[tokio::test]
    async fn test_format_annotation_unknown_type_defaults_to_notice() {
        let writer = make_writer();

        let input = FormatAnnotationInput {
            context: "Some informational message".to_string(),
            failure_type: "info".to_string(),
            file: None,
            line: None,
            details: None,
        };

        let result = writer.format_annotation(input).await.unwrap();
        assert_eq!(result.annotation.level, OutputLevel::Notice);
        assert_eq!(result.annotation.file, "<unknown>");
        assert_eq!(result.annotation.line, 1);
    }
}
