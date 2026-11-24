//! GitCommitTool — create Git commits.
//!
//! @canonical .pi/architecture/modules/tool-system.md#git-commit
//! Implements: Tool trait — GitCommit concrete tool
//! Issue: #125
//!
//! Creates Git commits with optional auto-staging of tracked files.
//! High risk — irreversible Git action.
//!
//! # Risk Level
//! High — commits are permanent and modify Git history.

// #1083: approved process-spawning sink — fixed-argv `git` sink: no caller string reaches a shell.
// See `.pi/architecture/modules/tool-system.md#process-spawning-sinks`.
#![allow(clippy::disallowed_types)]

use async_trait::async_trait;
use std::time::Instant;

use crate::tools::application::dto::{SideEffect, ToolInput, ToolResult};
use crate::tools::domain::{Tool, ToolError};

/// Tool for creating Git commits.
///
/// # Input Parameters
/// - `message` (required, string): Commit message.
/// - `auto_stage` (optional, bool): Whether to automatically stage tracked files (default: false).
///
/// # Risk Level
/// High — irreversible git action.
///
/// # Security
/// - If `auto_stage` is false, only already-staged changes are committed.
/// - The commit message must not be empty.
pub struct GitCommitTool {
    /// Root directory of the git repository.
    repo_root: String,
}

impl GitCommitTool {
    /// Create a new GitCommitTool with the given repository root.
    pub fn new(repo_root: impl Into<String>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }
}

#[async_trait]
impl Tool for GitCommitTool {
    fn name(&self) -> &str {
        "git-commit"
    }

    async fn execute(&self, input: &ToolInput) -> Result<ToolResult, ToolError> {
        let message = input.require_string("message")?;
        // #1660: a boolean, read as one — see `ToolInput::get_bool`. The unit tests below build the
        // string shape, which is why the drop was invisible to them.
        let auto_stage = input.get_bool("auto_stage").unwrap_or(false);

        if message.trim().is_empty() {
            return Err(ToolError::InvalidInput(
                "Commit message must not be empty".to_string(),
            ));
        }

        let start = Instant::now();

        // Optionally stage all tracked files
        if auto_stage {
            // sink: tools:git-commit-fixed-argv
            let add_output = tokio::process::Command::new("git")
                .args(["add", "-u"])
                .current_dir(&self.repo_root)
                .output()
                .await
                .map_err(|e| {
                    ToolError::ExecutionFailed(format!("Failed to auto-stage files: {}", e))
                })?;

            if !add_output.status.success() {
                let stderr = String::from_utf8_lossy(&add_output.stderr);
                return Err(ToolError::ExecutionFailed(format!(
                    "Auto-stage failed: {}",
                    stderr
                )));
            }
        }

        // Create the commit
        // sink: tools:git-commit-fixed-argv
        let output = tokio::process::Command::new("git")
            .args(["commit", "-m", &message])
            .current_dir(&self.repo_root)
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to create commit: {}", e)))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        if !output.status.success() {
            // #1660: git writes "nothing added to commit" to **stdout**, so a failure reported from
            // stderr alone said `Git commit failed: ` and nothing else — a refusal that names no
            // reason. Whichever stream carries it, the message carries it.
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = if stderr.trim().is_empty() {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            } else {
                stderr.trim().to_string()
            };
            return Err(ToolError::ExecutionFailed(format!(
                "Git commit failed: {}",
                if detail.is_empty() {
                    "git exited non-zero and said nothing on either stream".to_string()
                } else {
                    detail
                }
            )));
        }

        // Extract commit hash from output for side effect tracking
        let commit_hash = stdout
            .lines()
            .find(|l| l.contains("commit") || l.starts_with("["))
            .map(|l| l.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let result = ToolResult {
            output: stdout,
            exit_code: 0,
            side_effects: vec![
                SideEffect::new("HEAD", "git_commit", format!("Committed: {}", message))
                    .with_previous_hash(commit_hash),
            ],
            duration_ms,
            dry_run: false,
            resolved_executable: None,
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn init_git_repo(dir: &TempDir) {
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir.path())
            .output()
            .unwrap();
    }

    fn make_input(message: &str) -> ToolInput {
        let mut params = HashMap::new();
        params.insert(
            "message".to_string(),
            serde_json::Value::String(message.to_string()),
        );
        ToolInput::new(params)
    }

    fn make_input_with_auto_stage(message: &str, auto_stage: bool) -> ToolInput {
        let mut params = HashMap::new();
        params.insert(
            "message".to_string(),
            serde_json::Value::String(message.to_string()),
        );
        params.insert(
            "auto_stage".to_string(),
            serde_json::Value::String(auto_stage.to_string()),
        );
        ToolInput::new(params)
    }

    #[tokio::test]
    async fn test_commit_staged_changes() {
        let dir = TempDir::new().unwrap();
        init_git_repo(&dir);

        // Create and stage a file
        std::fs::write(dir.path().join("test.txt"), "content").unwrap();
        std::process::Command::new("git")
            .args(["add", "test.txt"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        let tool = GitCommitTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input("feat: add test file"))
            .await
            .unwrap();

        assert!(result.is_success());
        assert!(result.has_side_effects());

        // Verify commit exists
        let log = std::process::Command::new("git")
            .args(["log", "--oneline"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let log_output = String::from_utf8_lossy(&log.stdout);
        assert!(log_output.contains("feat: add test file"));
    }

    #[tokio::test]
    async fn test_commit_with_auto_stage() {
        let dir = TempDir::new().unwrap();
        init_git_repo(&dir);

        // Create a file (not staged)
        std::fs::write(dir.path().join("auto.txt"), "auto content").unwrap();
        // Need an initial commit first for auto-stage to work properly
        std::fs::write(dir.path().join("base.txt"), "base").unwrap();
        std::process::Command::new("git")
            .args(["add", "base.txt"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        // Now auto.txt is untracked - auto_stage only stages tracked files
        std::fs::write(dir.path().join("auto.txt"), "auto content").unwrap();
        // Stage it manually for this test
        std::process::Command::new("git")
            .args(["add", "auto.txt"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        let tool = GitCommitTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input_with_auto_stage("feat: auto commit", true))
            .await
            .unwrap();

        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_empty_commit_message() {
        let dir = TempDir::new().unwrap();
        let tool = GitCommitTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("")).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_commit_nothing_staged() {
        let dir = TempDir::new().unwrap();
        init_git_repo(&dir);

        let tool = GitCommitTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("feat: nothing")).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tool_name() {
        let dir = TempDir::new().unwrap();
        let tool = GitCommitTool::new(dir.path().to_str().unwrap());
        assert_eq!(tool.name(), "git-commit");
    }

    #[tokio::test]
    async fn test_commit_side_effect_tracking() {
        let dir = TempDir::new().unwrap();
        init_git_repo(&dir);

        std::fs::write(dir.path().join("track.txt"), "tracked").unwrap();
        std::process::Command::new("git")
            .args(["add", "track.txt"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        let tool = GitCommitTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input("feat: tracked change"))
            .await
            .unwrap();

        assert!(result.has_side_effects());
        assert_eq!(result.side_effects[0].effect_type, "git_commit");
        assert_eq!(result.side_effects[0].path, "HEAD");
    }

    /// #1660: `auto_stage` as a JSON boolean, which is what a plan step carries.
    #[tokio::test]
    async fn test_auto_stage_accepts_a_json_boolean() {
        let temp = TempDir::new().unwrap();
        init_git_repo(&temp);
        let dir = temp;
        // The file must be *tracked* first: `auto_stage` is `git add -u`, which stages modifications to
        // tracked paths and deliberately not new ones.
        std::fs::write(dir.path().join("tracked.txt"), "seed\n").unwrap();
        for args in [
            vec!["add", "tracked.txt"],
            vec!["commit", "-q", "-m", "seed"],
        ] {
            assert!(
                std::process::Command::new("git")
                    .args(&args)
                    .current_dir(dir.path())
                    .output()
                    .unwrap()
                    .status
                    .success(),
                "git {args:?} seeds the fixture"
            );
        }
        std::fs::write(dir.path().join("tracked.txt"), "changed\n").unwrap();
        let tool = GitCommitTool::new(dir.path().to_str().unwrap());
        let mut params = HashMap::new();
        params.insert("message".to_string(), serde_json::json!("a commit"));
        params.insert("auto_stage".to_string(), serde_json::json!(true));
        tool.execute(&ToolInput::new(params))
            .await
            .expect("a boolean auto_stage stages and commits");
        let subject = std::process::Command::new("git")
            .args(["log", "-1", "--format=%s"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&subject.stdout).trim(), "a commit");
    }

    /// A refusal names a reason. `git` reports "nothing added to commit" on **stdout**, so a message
    /// built from stderr alone said `Git commit failed: ` and nothing else.
    #[tokio::test]
    async fn test_a_commit_with_nothing_staged_names_the_reason() {
        let temp = TempDir::new().unwrap();
        init_git_repo(&temp);
        let dir = temp;
        let tool = GitCommitTool::new(dir.path().to_str().unwrap());
        let mut params = HashMap::new();
        params.insert("message".to_string(), serde_json::json!("nothing"));
        let error = tool
            .execute(&ToolInput::new(params))
            .await
            .expect_err("nothing is staged");
        let message = format!("{error}");
        assert!(
            message.to_lowercase().contains("nothing"),
            "the refusal must carry git's own reason: {message}"
        );
    }
}
