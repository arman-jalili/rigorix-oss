# Action Output Architecture

<!--
Canonical Reference: .pi/architecture/modules/action-output.md
Blueprint Source: Rigorix design session (2026-06-20)
Rationale: Format engine results as GitHub Actions-native outputs — annotations, step summaries, output variables
-->

## Overview

The Action Output module formats `rigorix-engine` execution results as GitHub Actions-native outputs. It converts structured engine types (ExecutionContext, WorkflowAnnotation) into GitHub workflow commands, step summaries, annotations, and output variables. No business logic — pure formatting. It converts structured engine types (ExecutionRecord, ValidationReport, TemplateFailure) into GitHub workflow commands, step summaries, annotations, and output variables. No business logic — pure formatting.

## Responsibilities

- Format engine `RunOutput` as GitHub step summary (markdown)
- Convert `TemplateFailure` list into workflow annotations (`::error file=...::message`)
- Set GitHub output variables for downstream workflow steps
- Post PR comments with execution summaries (when GitHub token is available)
- Format validation loop reports as collapsible step summary sections
- Map engine log levels to GitHub workflow command levels (warning, error, notice)

## Components

| Component | Interface | Implementation | Canonical Section |
|-----------|-----------|----------------|-------------------|
| OutputFormatter | `OutputFormattingService` (in `application/service.rs`) | `output_formatter_impl.rs` | #formatter |
| AnnotationWriter | `AnnotationWritingService` (in `application/service.rs`) | `annotation_writer_impl.rs` | #annotations |
| StepSummaryWriter | `StepSummaryWritingService` (in `application/service.rs`) | `step_summary_writer_impl.rs` | #summary |
| OutputVariableWriter | `OutputVariableService` (in `application/service.rs`) | *(pending)* | #variables |
| PrCommentWriter | `PrCommentService` (in `application/service.rs`) | *(pending)* | #pr-comment |
| Infrastructure | `OutputRepository` / `EnvRepository` / `GitHubApiClient` | `infrastructure/repository/mod.rs` (interfaces) | #repository |

---

## Component Details

### OutputFormatter

**Purpose:** Top-level formatter that orchestrates all output channels

```rust
/// Formats engine results into GitHub Actions-native outputs.
///
/// This is the main public interface. It delegates to specialized
/// writers for each output channel (summary, annotations, variables, PR comments).
pub struct OutputFormatter {
    github_token: Option<String>,
    repo: Option<String>,
    pr_number: Option<u64>,
}

impl OutputFormatter {
    /// Write all outputs for a successful engine run.
    pub async fn write_run_output(&self, output: &RunOutput) -> Result<(), ActionOutputError> {
        self.write_step_summary(&self.format_run_summary(output))?;
        self.write_output_variables(&self.format_run_variables(output))?;
        Ok(())
    }

    /// Write all outputs for a failed validation with failure details.
    pub async fn write_validation_failure(
        &self,
        report: &ValidationReport,
        execution_id: Uuid,
    ) -> Result<(), ActionOutputError> {
        // Write annotations for each failure
        for iteration in &report.failure_history {
            for failure in &iteration.failures {
                self.write_annotation(failure)?;
            }
        }

        // Write detailed step summary
        self.write_step_summary(&self.format_validation_report(report))?;

        // Post PR comment if we have token + PR context
        if let Some(pr) = self.pr_number {
            self.write_pr_comment(pr, &self.format_pr_failure_summary(report, execution_id)).await?;
        }

        Ok(())
    }
}
```

### AnnotationWriter

**Purpose:** Emits GitHub workflow annotations for template failures

```rust
/// Writes workflow annotations using GitHub Actions workflow commands.
///
/// Format: ::error file=path,line=10,col=5::message
/// Supported levels: error, warning, notice
pub struct AnnotationWriter;

impl AnnotationWriter {
    /// Convert a TemplateFailure into a workflow annotation.
    pub fn write_annotation(failure: &TemplateFailure) -> Result<(), ActionOutputError> {
        match failure {
            TemplateFailure::MissingSymbol { symbol, location, suggestion, .. } => {
                let msg = if let Some(fix) = suggestion {
                    format!("'{}' not found. {}", symbol, fix)
                } else {
                    format!("'{}' not found", symbol)
                };
                Self::emit("error", &location.file, location.line, location.column, &msg)
            }
            TemplateFailure::WrongArgCount { location, .. } => {
                Self::emit("error", &location.file, location.line, location.column,
                    &format!("Wrong argument count for '{}'", failure))
            }
            TemplateFailure::TypeMismatch { location, .. } => {
                Self::emit("error", &location.file, location.line, location.column,
                    &format!("Type mismatch: {}", failure))
            }
            TemplateFailure::CompileError { code, message, location } => {
                Self::emit("error", &location.file, location.line, location.column,
                    &format!("{}: {}", code, message))
            }
            TemplateFailure::AssertionFailure { test_name, expected, received, location } => {
                Self::emit("error", &location.file, location.line, location.column,
                    &format!("Test '{}' failed: expected {}, received {}", test_name, expected, received))
            }
            TemplateFailure::TestFailure { test_name, message, .. } => {
                let loc = format!("tests/{}", test_name);
                Self::emit("error", &loc, 1, None, message)
            }
        }
    }

    /// Emit a workflow command. GitHub Actions reads workflow commands from stdout
    /// (not GITHUB_ENV or GITHUB_OUTPUT — those are for environment/output variables only).
    /// Workflow commands must be written to stdout to be parsed by the Actions runner.
    fn emit(level: &str, file: &str, line: usize, col: Option<usize>, message: &str) -> Result<(), ActionOutputError> {
        let col_str = col.map(|c| format!(",col={}", c)).unwrap_or_default();
        println!("::{} file={},line={}{}::{}", level, file, line, col_str, message);
        Ok(())
    }
}
```

### StepSummaryWriter

**Purpose:** Writes GitHub Actions step summaries (markdown rendered in the Actions UI)

```rust
/// Writes GitHub Actions step summaries using `$GITHUB_STEP_SUMMARY`.
pub struct StepSummaryWriter;

impl StepSummaryWriter {
    /// Write a markdown-formatted summary to the step summary file.
    pub fn write(&self, markdown: &str) -> Result<(), ActionOutputError> {
        let summary_path = std::env::var("GITHUB_STEP_SUMMARY")
            .map_err(|_| ActionOutputError::MissingEnv("GITHUB_STEP_SUMMARY"))?;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&summary_path)?;
        file.write_all(markdown.as_bytes())?;
        Ok(())
    }
}
```

**Step summary format example:**

```markdown
## Rigorix Execution #e1852176

**Status:** ✅ Completed | **Duration:** 12.4s | **Quality:** workspace

### Execution Plan
1. ✅ `read-task-file` — Read source (0.3s)
2. ✅ `add-get-active-tasks-method` — Patched class with AST anchor (1.2s)
3. ✅ `write-test-file` — Generated test (2.1s)
4. ✅ `compile-check` — `tsc --noEmit` passed (4.5s)
5. ✅ `run-test` — Jest: 1 test passed (4.3s)

### Validation
- Iterations: 1/3
- Cumulative tokens: 3,240

<details>
<summary>Template</summary>

\`\`\`toml
[[nodes]]
id = "read-task-file"
type = "file_read"
...
\`\`\`
</details>
```

### OutputVariableWriter

**Purpose:** Sets `$GITHUB_OUTPUT` variables for downstream workflow steps

```rust
/// Writes output variables for downstream workflow steps.
///
/// Available outputs:
/// - `execution_id` — UUID of the execution
/// - `status` — "completed" | "failed" | "partial"
/// - `iterations` — number of validation iterations
/// - `template_id` — ID of the generated template
/// - `quality_level` — achieved quality level
/// - `failure_count` — number of failures (0 on success)
pub struct OutputVariableWriter;

impl OutputVariableWriter {
    pub fn write(name: &str, value: &str) -> Result<(), ActionOutputError> {
        let output_path = std::env::var("GITHUB_OUTPUT")
            .unwrap_or_else(|_| "/dev/null".to_string());
        let mut file = std::fs::OpenOptions::new().append(true).open(&output_path)?;
        writeln!(file, "{}={}", name, value)?;
        Ok(())
    }
}
```

---

## Output Variables Reference

| Variable | Type | Description | Example |
|----------|------|-------------|---------|
| `execution_id` | string | UUID of the execution | `e1852176-e586-4377-a8e8-d1cb4be89144` |
| `status` | string | Final execution status | `completed`, `failed`, `partial_failure` |
| `iterations` | number | Validation loop iterations | `2` |
| `template_id` | string | ID of the generated/used template | `add-get-active-tasks` |
| `quality_level` | string | Achieved quality level | `workspace`, `package`, `targeted_tests` |
| `failure_count` | number | Number of failures (0 on success) | `3` |
| `cumulative_tokens` | number | Total LLM tokens used | `8450` |
| `duration_ms` | number | Total execution duration | `12400` |

---

## Dependencies

### Depends On
- **rigorix-engine::orchestrator**: `ExecutionRecord`, `RunOutput` types
- **rigorix-engine::failure_parser**: `TemplateFailure` for annotations
- **rigorix-engine::plan_validation**: `ValidationReport` for failure summaries

### Used By
- **action-entrypoint**: Calls formatter after engine dispatch completes

---

## Observability

### Logging
- Module uses `tracing` for structured logging with correlation IDs
- Key events logged:
  - `OutputWritten` — summary bytes, annotation count, variable count
  - `StepSummaryWritten` — title, section count, bytes written
  - `AnnotationEmitted` — level, file, line
  - `OutputVariableSet` — variable name, value length
  - `PrCommentPosted` — PR number, body length
- Log levels: `info` (normal operations), `warn` (recoverable issues), `error` (failures)

### Tracing Spans
- Root span: `action_output` — wraps all output operations
- Child spans: `write_run_output`, `write_validation_failure`, `format_summary`
- Execution UUID propagated through all spans for correlation

### Metrics (Planned)
The module is stateless and short-lived. Future observability:
- Annotation count per execution (counter)
- Summary bytes written (histogram)
- Output variable count (gauge)
- Execution status distribution (success/failure counter)

## Security Considerations

| Concern | Mitigation |
|---------|------------|
| GitHub token in PR comments | Token passed via `secrets.GITHUB_TOKEN`, never logged |
| Sensitive output in step summaries | Full template content placed in collapsed `<details>` sections |
| Output variable injection | Values are sanitized (newlines stripped, length capped) |

---

## Related ADRs

- **Engine ADR-001** (`engine/.pi/architecture/decisions/ADR-001-architecture-pattern.md`): Pure presentation adapter
- **Engine ADR-006** (`engine/.pi/architecture/decisions/ADR-006-atomic-write-rename.md`): Append-only output patterns
- **Actions ADR-101** (`actions/.pi/architecture/decisions/ADR-101-actions-as-thin-adapter.md`): Output formatting is a thin layer

---

*Last updated: 2026-06-20*
*Module version: 1.0.0 (Implemented)*

---

**Status:** Implemented (3/5 components)
**Engine modules reused:** orchestrator, failure_parser, plan_validation
**Proofing scripts:** `check_action-output_contracts.sh`, `check_action-output_coverage.sh`
**Runbook:** `docs/runbook-action-output.md`
**DR Plan:** `docs/dr-plan-action-output.md`
