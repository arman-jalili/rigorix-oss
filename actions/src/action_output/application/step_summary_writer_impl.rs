//! Implementation of `StepSummaryWritingService`.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md#stepsummarywriter
//! Implements: StepSummaryWritingService — markdown step summary writing to GITHUB_STEP_SUMMARY
//! Issue: issue-stepsummarywriter
//!
//! # Contract
//! - Implements `StepSummaryWritingService` trait from the frozen contract
//! - Renders `StepSummary` domain type into GitHub-flavored markdown
//! - Writes to `$GITHUB_STEP_SUMMARY` via `OutputRepository`
//! - Supports append and overwrite modes
//! - Large content wrapped in `<details>` HTML tags

use async_trait::async_trait;
use tracing::info;

use crate::action_output::domain::{ActionOutputError, StepSummary, SummarySection};

use super::dto::{WriteSummaryInput, WriteSummaryOutput};
use super::service::StepSummaryWritingService;
use crate::action_output::infrastructure::repository::OutputRepository;

/// Default implementation of `StepSummaryWritingService`.
///
/// Writes GitHub Actions step summaries as markdown to the file specified
/// by `$GITHUB_STEP_SUMMARY`. The rendered markdown appears in the
/// GitHub Actions UI in the "Summary" section of the workflow run.
///
/// # Summary Format
///
/// ```markdown
/// ## Title
///
/// ### Section Heading
/// Body content (markdown)
///
/// <details>
/// <summary>Label</summary>
///
/// Collapsible content
/// </details>
///
/// ---
/// Footer text
/// ```
///
/// # Dependencies
/// - `OutputRepository` — for writing to the step summary file
///
/// # Construction
/// Use `StepSummaryWriterImpl::new(output_repo)`.
pub struct StepSummaryWriterImpl {
    output_repo: Box<dyn OutputRepository>,
}

impl StepSummaryWriterImpl {
    /// Create a new `StepSummaryWriterImpl`.
    pub fn new(output_repo: Box<dyn OutputRepository>) -> Self {
        Self { output_repo }
    }

    /// Render a `StepSummary` into a markdown string.
    pub fn render_markdown(&self, summary: &StepSummary) -> String {
        let mut parts: Vec<String> = Vec::new();

        // Title (H2)
        parts.push(format!("## {}\n", summary.title));

        // Sections
        for section in &summary.sections {
            parts.push(self.render_section(section));
        }

        // Footer
        if let Some(ref footer) = summary.footer {
            parts.push(format!("---\n{}\n", footer));
        }

        parts.join("\n")
    }

    /// Render a single summary section to markdown.
    fn render_section(&self, section: &SummarySection) -> String {
        let mut parts = Vec::new();

        if section.collapsible {
            // Collapsible section with <details> HTML tags
            let label = section.collapsible_label.as_deref().unwrap_or("Details");
            parts.push(format!("<details>\n<summary>{}</summary>\n\n", label));
            parts.push(format!("### {}\n", section.heading));
            parts.push(section.body.clone());
            parts.push("\n</details>\n".to_string());
        } else {
            // Visible section
            parts.push(format!("### {}\n", section.heading));
            parts.push(section.body.clone());
            parts.push("\n".to_string());
        }

        parts.join("")
    }
}

#[async_trait]
impl StepSummaryWritingService for StepSummaryWriterImpl {
    async fn write_summary(
        &self,
        input: WriteSummaryInput,
    ) -> Result<WriteSummaryOutput, ActionOutputError> {
        let markdown = self.render_markdown(&input.summary);

        let mode = if input.append { "append" } else { "overwrite" };
        let bytes = if input.append {
            self.output_repo.append_summary(&markdown).await?
        } else {
            self.output_repo.overwrite_summary(&markdown).await?
        };

        info!(
            section_count = input.summary.sections.len(),
            bytes_written = bytes,
            mode = mode,
            title = %input.summary.title,
            "step summary written"
        );

        Ok(WriteSummaryOutput {
            bytes_written: bytes,
            section_count: input.summary.sections.len() as u32,
        })
    }

    async fn render_markdown(&self, summary: &StepSummary) -> Result<String, ActionOutputError> {
        Ok(self.render_markdown(summary))
    }

    async fn is_available(&self) -> bool {
        self.output_repo.is_github_actions().await
    }

    async fn get_summary_path(&self) -> Result<String, ActionOutputError> {
        self.output_repo
            .get_summary_path()
            .await?
            .ok_or_else(|| ActionOutputError::MissingEnv("GITHUB_STEP_SUMMARY".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action_output::domain::StepSummary;
    use std::sync::{Arc, Mutex};

    // ── Mock OutputRepository ──

    #[derive(Clone)]
    struct MockOutputRepo {
        written: Arc<Mutex<Vec<String>>>,
        available: bool,
        summary_path: Option<String>,
    }

    impl MockOutputRepo {
        fn new(available: bool) -> Self {
            Self {
                written: Arc::new(Mutex::new(Vec::new())),
                available,
                summary_path: if available {
                    Some("/tmp/test-summary.md".to_string())
                } else {
                    None
                },
            }
        }

        fn get_written(&self) -> Vec<String> {
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
            _name: &str,
            _value: &str,
        ) -> Result<u64, ActionOutputError> {
            Ok(0)
        }

        async fn append_summary(&self, markdown: &str) -> Result<u64, ActionOutputError> {
            let len = markdown.len() as u64;
            self.written
                .lock()
                .unwrap()
                .push(format!("APPEND:{}", markdown));
            Ok(len)
        }

        async fn overwrite_summary(&self, markdown: &str) -> Result<u64, ActionOutputError> {
            let len = markdown.len() as u64;
            self.written
                .lock()
                .unwrap()
                .push(format!("OVERWRITE:{}", markdown));
            Ok(len)
        }

        async fn get_output_path(&self) -> Result<Option<String>, ActionOutputError> {
            Ok(None)
        }

        async fn get_summary_path(&self) -> Result<Option<String>, ActionOutputError> {
            Ok(self.summary_path.clone())
        }

        async fn is_github_actions(&self) -> bool {
            self.available
        }
    }

    // ── Helpers ──

    fn make_writer() -> StepSummaryWriterImpl {
        let repo = MockOutputRepo::new(true);
        StepSummaryWriterImpl::new(Box::new(repo))
    }

    fn make_writer_with_repo(repo: MockOutputRepo) -> (StepSummaryWriterImpl, MockOutputRepo) {
        let writer = StepSummaryWriterImpl::new(Box::new(repo.clone()));
        (writer, repo)
    }

    fn make_sample_summary() -> StepSummary {
        let mut summary = StepSummary::new("Rigorix Execution #abc123");
        summary.add_section(SummarySection::new(
            "Execution Plan",
            "1. ✅ `task-1` — Read file (0.3s)\n2. ✅ `task-2` — Patch class (1.2s)",
        ));
        summary.add_section(SummarySection::collapsible(
            "Files Changed (2)",
            "- 📝 `src/main.rs`\n- 🆕 `src/lib.rs`",
            "Show files",
        ));
        summary.set_footer("**Status:** ✅ Completed | **Duration:** 5.2s");
        summary
    }

    // ── Tests ──

    #[tokio::test]
    async fn test_write_summary_append() {
        let repo = MockOutputRepo::new(true);
        let (writer, repo) = make_writer_with_repo(repo);

        let summary = make_sample_summary();
        let input = WriteSummaryInput {
            summary,
            append: true,
        };

        let result = writer.write_summary(input).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.bytes_written > 0);
        assert_eq!(output.section_count, 2);

        let written = repo.get_written();
        assert_eq!(written.len(), 1);
        assert!(written[0].starts_with("APPEND:"));
    }

    #[tokio::test]
    async fn test_write_summary_overwrite() {
        let repo = MockOutputRepo::new(true);
        let (writer, repo) = make_writer_with_repo(repo);

        let summary = make_sample_summary();
        let input = WriteSummaryInput {
            summary,
            append: false,
        };

        let result = writer.write_summary(input).await;
        assert!(result.is_ok());

        let written = repo.get_written();
        assert!(written[0].starts_with("OVERWRITE:"));
    }

    #[tokio::test]
    async fn test_render_markdown_title() {
        let writer = make_writer();
        let summary = make_sample_summary();

        let markdown = writer.render_markdown(&summary);
        assert!(markdown.contains("## Rigorix Execution #abc123"));
    }

    #[tokio::test]
    async fn test_render_markdown_sections() {
        let writer = make_writer();
        let summary = make_sample_summary();

        let markdown = writer.render_markdown(&summary);
        assert!(markdown.contains("### Execution Plan"));
        assert!(markdown.contains("1. ✅ `task-1`"));
        assert!(markdown.contains("### Files Changed (2)"));
    }

    #[tokio::test]
    async fn test_render_markdown_collapsible() {
        let writer = make_writer();
        let summary = make_sample_summary();

        let markdown = writer.render_markdown(&summary);
        assert!(markdown.contains("<details>"));
        assert!(markdown.contains("<summary>Show files</summary>"));
        assert!(markdown.contains("</details>"));
    }

    #[tokio::test]
    async fn test_render_markdown_footer() {
        let writer = make_writer();
        let summary = make_sample_summary();

        let markdown = writer.render_markdown(&summary);
        assert!(markdown.contains("---"));
        assert!(markdown.contains("**Status:** ✅ Completed | **Duration:** 5.2s"));
    }

    #[tokio::test]
    async fn test_render_markdown_visible_section_no_details() {
        let writer = make_writer();
        let mut summary = StepSummary::new("Test");
        summary.add_section(SummarySection::new("Heading", "Body text"));

        let markdown = writer.render_markdown(&summary);
        assert!(!markdown.contains("<details>"));
        assert!(markdown.contains("### Heading"));
        assert!(markdown.contains("Body text"));
    }

    #[tokio::test]
    async fn test_is_available_when_ci() {
        let repo = MockOutputRepo::new(true);
        let writer = StepSummaryWriterImpl::new(Box::new(repo));
        assert!(writer.is_available().await);
    }

    #[tokio::test]
    async fn test_is_available_when_not_ci() {
        let repo = MockOutputRepo::new(false);
        let writer = StepSummaryWriterImpl::new(Box::new(repo));
        assert!(!writer.is_available().await);
    }

    #[tokio::test]
    async fn test_get_summary_path_available() {
        let repo = MockOutputRepo::new(true);
        let writer = StepSummaryWriterImpl::new(Box::new(repo));
        let path = writer.get_summary_path().await.unwrap();
        assert_eq!(path, "/tmp/test-summary.md");
    }

    #[tokio::test]
    async fn test_get_summary_path_missing() {
        let repo = MockOutputRepo::new(false);
        let writer = StepSummaryWriterImpl::new(Box::new(repo));
        let result = writer.get_summary_path().await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ActionOutputError::MissingEnv(_)
        ));
    }

    #[tokio::test]
    async fn test_render_markdown_empty_summary() {
        let writer = make_writer();
        let summary = StepSummary::new("Empty");

        let markdown = writer.render_markdown(&summary);
        assert_eq!(markdown, "## Empty\n");
    }

    #[tokio::test]
    async fn test_render_markdown_multiple_collapsible_sections() {
        let writer = make_writer();
        let mut summary = StepSummary::new("Multi");
        summary.add_section(SummarySection::collapsible("Section 1", "Body 1", "Show 1"));
        summary.add_section(SummarySection::collapsible("Section 2", "Body 2", "Show 2"));

        let markdown = writer.render_markdown(&summary);
        assert_eq!(markdown.matches("<details>").count(), 2);
        assert_eq!(markdown.matches("</details>").count(), 2);
        assert!(markdown.contains("<summary>Show 1</summary>"));
        assert!(markdown.contains("<summary>Show 2</summary>"));
    }

    #[tokio::test]
    async fn test_render_markdown_no_footer() {
        let writer = make_writer();
        let mut summary = StepSummary::new("No Footer");
        summary.add_section(SummarySection::new("Section", "Body"));

        let markdown = writer.render_markdown(&summary);
        assert!(!markdown.contains("---"));
    }
}
