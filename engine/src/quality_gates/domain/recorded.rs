//! A recorded step outcome — the only subject a quality level may be derived from (#1585).
//!
//! @canonical .pi/architecture/modules/quality-gates.md#recorded-outcome
//! Issue: #1585
//!
//! Before this existed, `ClassifyTestScopeInput` carried six booleans and both production callers
//! invented them: `workspace_tests_run: input.iteration > 2` in `plan_validation`, and
//! `workspace_tests_run: final_status != ExecutionStatus::Failed` in the orchestrator. "Workspace-level
//! tests passed" was therefore an assertion about the loop's own bookkeeping, `lint_passed` was the
//! literal `false` at both sites (so `MergeReady` was unreachable by construction), and any
//! `source: Engine` stamped from either would have put a counter's arithmetic into a signed record.
//!
//! This type is what the classifier is allowed to read instead: the step that ran, the tool it ran,
//! what it ran, and whether it reported success. A run whose steps say nothing about tests produces
//! no level at all — see [`ClaimSource`], which is how the difference travels.

use serde::{Deserialize, Serialize};

/// What a recorded outcome is evidence about.
///
/// Derived from the step's tool and the subject it ran, never declared by the caller: a caller that
/// could declare it would be back to asserting. `Other` is the honest answer for a step that says
/// nothing about quality, and it is not a failure — it is no evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    /// A workspace-wide test run (`cargo test --workspace`, `nextest --workspace`).
    WorkspaceTest,
    /// A package/crate-scoped test run (`cargo test -p crate`).
    PackageTest,
    /// A filtered or single-target test run (`cargo test <name>`, `--test <target>`).
    TargetedTest,
    /// A lint pass (clippy).
    Lint,
    /// A formatting check (`fmt --check`).
    Format,
    /// A security audit (`cargo audit`).
    Audit,
    /// A step that reported an outcome about something else.
    Other,
}

impl CheckKind {
    /// Whether this kind is a test run, at any scope.
    #[must_use]
    pub fn is_test(&self) -> bool {
        matches!(
            self,
            CheckKind::WorkspaceTest | CheckKind::PackageTest | CheckKind::TargetedTest
        )
    }

    /// The canonical name, for explanations and for records.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            CheckKind::WorkspaceTest => "workspace_test",
            CheckKind::PackageTest => "package_test",
            CheckKind::TargetedTest => "targeted_test",
            CheckKind::Lint => "lint",
            CheckKind::Format => "format",
            CheckKind::Audit => "audit",
            CheckKind::Other => "other",
        }
    }
}

/// One outcome a step actually reported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedOutcome {
    /// The step (DAG node) that reported it.
    pub step_id: String,

    /// The step's name, for an explanation a person can act on.
    #[serde(default)]
    pub step_name: String,

    /// The tool the step ran.
    pub tool: String,

    /// What the step ran — the command, expanded invocation, or target — when the plan recorded it.
    ///
    /// `None` is not "unknown but probably fine": a `cargo test` step whose command was not recorded
    /// cannot say whether it was workspace-wide, and the classifier treats it as the narrower scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,

    /// Whether the step reported success.
    pub passed: bool,
}

impl RecordedOutcome {
    /// A recorded outcome.
    #[must_use]
    pub fn new(step_id: impl Into<String>, tool: impl Into<String>, passed: bool) -> Self {
        Self {
            step_id: step_id.into(),
            step_name: String::new(),
            tool: tool.into(),
            subject: None,
            passed,
        }
    }

    /// Name the step, so an explanation can point at it.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.step_name = name.into();
        self
    }

    /// Record what the step ran.
    #[must_use]
    pub fn with_subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// What this outcome is evidence about.
    ///
    /// The tool wins when it is specific (`rigorix_lint`); otherwise the subject decides, because a
    /// plan may run `cargo clippy` through a generic `run_command` step. A test invocation with no
    /// recognisable scope is `TargetedTest` — the narrower claim, so an unrecognised command cannot
    /// raise the level.
    #[must_use]
    pub fn kind(&self) -> CheckKind {
        let tool = self.tool.to_ascii_lowercase();
        let subject = self
            .subject
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let haystack = if subject.is_empty() {
            tool.clone()
        } else {
            format!("{tool} {subject}")
        };

        if haystack.contains("clippy") || tool.contains("lint") {
            return CheckKind::Lint;
        }
        if haystack.contains("fmt") || haystack.contains("rustfmt") || haystack.contains("format") {
            return CheckKind::Format;
        }
        if haystack.contains("audit") {
            return CheckKind::Audit;
        }
        let is_test = haystack.contains("test") || haystack.contains("nextest");
        if !is_test {
            return CheckKind::Other;
        }
        if haystack.contains("--workspace") || haystack.contains(" --all") {
            return CheckKind::WorkspaceTest;
        }
        if haystack.contains(" -p ")
            || haystack.contains("--package")
            || haystack.contains("--package=")
        {
            return CheckKind::PackageTest;
        }
        CheckKind::TargetedTest
    }

    /// How to name this outcome in an explanation: the step, and what it was evidence about.
    #[must_use]
    pub fn label(&self) -> String {
        let name = if self.step_name.is_empty() {
            self.step_id.clone()
        } else {
            format!("{} ({})", self.step_name, self.step_id)
        };
        format!("{} — {}", name, self.kind().as_str())
    }
}

/// Who is making a quality claim (#1585, ADR-024 §2 D1).
///
/// The distinction is the whole point of the type: a level the engine derived from recorded outcomes
/// is `Engine`, and a level a caller asserted — or that the engine had to default because its steps
/// recorded nothing about quality — is `Caller`. A record that stamps `source: Engine` on a claim
/// nobody measured is a signed assertion with no subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimSource {
    /// Derived from outcomes this engine recorded from executed steps.
    Engine,
    /// Asserted by the caller, or defaulted in the absence of recorded outcomes.
    Caller,
}

impl ClaimSource {
    /// The canonical name.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            ClaimSource::Engine => "engine",
            ClaimSource::Caller => "caller",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_workspace_test_is_recognised_by_its_command() {
        let outcome = RecordedOutcome::new("n1", "run_command", true)
            .with_subject(r#"{"command":"cargo test --workspace"}"#);
        assert_eq!(outcome.kind(), CheckKind::WorkspaceTest);
    }

    #[test]
    fn a_package_test_is_not_a_workspace_test() {
        let outcome = RecordedOutcome::new("n2", "run_command", true)
            .with_subject("cargo test -p crate --lib");
        assert_eq!(outcome.kind(), CheckKind::PackageTest);
    }

    #[test]
    fn a_filtered_test_is_targeted() {
        let outcome = RecordedOutcome::new("n3", "run_command", true)
            .with_subject("cargo test --test unit receipt");
        assert_eq!(outcome.kind(), CheckKind::TargetedTest);
    }

    #[test]
    fn the_three_integration_gates_are_recognised() {
        assert_eq!(
            RecordedOutcome::new("n4", "run_command", true)
                .with_subject("cargo clippy -- -D warnings")
                .kind(),
            CheckKind::Lint
        );
        assert_eq!(
            RecordedOutcome::new("n5", "run_command", true)
                .with_subject("cargo fmt --check")
                .kind(),
            CheckKind::Format
        );
        assert_eq!(
            RecordedOutcome::new("n6", "run_command", true)
                .with_subject("cargo audit --deny warnings")
                .kind(),
            CheckKind::Audit
        );
    }

    #[test]
    fn a_step_that_says_nothing_about_quality_is_other() {
        let outcome = RecordedOutcome::new("n7", "file_read", true).with_subject("README.md");
        assert_eq!(outcome.kind(), CheckKind::Other);
        assert!(!outcome.kind().is_test());
    }

    #[test]
    fn a_tool_name_decides_when_the_subject_does_not() {
        let outcome = RecordedOutcome::new("n8", "rigorix_lint", true);
        assert_eq!(outcome.kind(), CheckKind::Lint);
    }

    #[test]
    fn an_unrecognised_test_command_claims_the_narrower_scope() {
        // No `--workspace`, no `-p`: the honest reading is the smallest claim, so an argv the
        // classifier does not recognise cannot raise the level.
        let outcome = RecordedOutcome::new("n9", "run_command", true).with_subject("xtask test");
        assert_eq!(outcome.kind(), CheckKind::TargetedTest);
    }
}
