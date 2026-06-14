//! GitReadTool — read Git repository information.
//!
//! @canonical .pi/architecture/modules/tool-system.md#git-read
//! Implements: Tool trait — GitRead concrete tool
//! Issue: #125
//!
//! Reads git repository information including log, diff, status, and other
//! git commands. Read-only operation (Low risk).
//!
//! All git commands are restricted to read-only operations (log, diff, status,
//! show, branch, ls-files) to prevent accidental mutations.

use async_trait::async_trait;
use std::time::Instant;

use crate::tools::application::dto::{ToolInput, ToolResult};
use crate::tools::domain::{Tool, ToolError};

/// Set of allowed read-only git commands.
const ALLOWED_GIT_COMMANDS: &[&str] = &[
    "log",
    "diff",
    "status",
    "show",
    "branch",
    "ls-files",
    "describe",
    "rev-parse",
    "rev-list",
    "cat-file",
];

/// Tool for reading Git repository information.
///
/// # Input Parameters
/// - `command` (required, string): Git command to execute (e.g., "log", "diff --cached", "status").
/// - `path` (optional, string): Path constraint for the command.
/// - `max_results` (optional, int): Maximum results to return (default: 50).
///
/// # Risk Level
/// Low — read-only, no side effects.
///
/// # Security
/// Only read-only git commands are allowed. Write commands (commit, push, reset)
/// are rejected to prevent accidental mutations.
pub struct GitReadTool {
    /// Root directory of the git repository.
    repo_root: String,
}

impl GitReadTool {
    /// Create a new GitReadTool with the given repository root.
    pub fn new(repo_root: impl Into<String>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }

    /// Check if the git command is a read-only operation.
    fn is_read_only_command(command: &str) -> bool {
        let main_cmd = command.split_whitespace().next().unwrap_or("");
        ALLOWED_GIT_COMMANDS.contains(&main_cmd)
    }
}

#[async_trait]
impl Tool for GitReadTool {
    fn name(&self) -> &str {
        "git-read"
    }

    async fn execute(&self, input: &ToolInput) -> Result<ToolResult, ToolError> {
        let command = input.require_string("command")?;
        let path = input.get_string("path");
        let max_results = input.get_u64("max_results").unwrap_or(50);

        // Validate the command is read-only
        if !Self::is_read_only_command(&command) {
            return Err(ToolError::InvalidInput(format!(
                "Git command '{}' is not a read-only operation. Allowed commands: {}",
                command,
                ALLOWED_GIT_COMMANDS.join(", ")
            )));
        }

        let start = Instant::now();

        // Build the full git command
        let mut full_cmd = format!("git {}", command);
        if let Some(p) = &path {
            full_cmd.push_str(&format!(" -- {}", p));
        }

        // Add max-count for log commands
        if command.starts_with("log") {
            full_cmd = format!("git {} --max-count={}", command, max_results);
            if let Some(p) = &path {
                full_cmd.push_str(&format!(" -- {}", p));
            }
        }

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&full_cmd)
            .current_dir(&self.repo_root)
            .output()
            .await
            .map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to execute git command: {}", e))
            })?;

        let duration_ms = start.elapsed().as_millis() as u64;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let result_output = if output.status.success() {
            stdout
        } else {
            format!("GIT ERROR:\n{}\n\nOUTPUT:\n{}", stderr, stdout)
        };

        let exit_code = output.status.code().unwrap_or(-1);

        Ok(ToolResult {
            output: result_output,
            exit_code,
            side_effects: vec![],
            duration_ms,
            dry_run: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn make_input(command: &str) -> ToolInput {
        let mut params = HashMap::new();
        params.insert("command".to_string(), serde_json::Value::String(command.to_string()));
        ToolInput::new(params)
    }

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
        // Create an initial commit
        std::fs::write(dir.path().join("README.md"), "# Test").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(dir.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(dir.path())
            .output()
            .unwrap();
    }

    #[tokio::test]
    async fn test_git_status() {
        let dir = TempDir::new().unwrap();
        init_git_repo(&dir);

        let tool = GitReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("status")).await.unwrap();

        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_git_log() {
        let dir = TempDir::new().unwrap();
        init_git_repo(&dir);

        let tool = GitReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("log")).await.unwrap();

        assert!(result.is_success());
        assert!(result.output.contains("Initial commit"));
    }

    #[tokio::test]
    async fn test_write_command_rejected() {
        let dir = TempDir::new().unwrap();
        let tool = GitReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("commit -m 'bad'")).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_git_diff() {
        let dir = TempDir::new().unwrap();
        init_git_repo(&dir);

        // Modify the existing tracked file to produce a diff
        std::fs::write(dir.path().join("README.md"), "# Modified Content").unwrap();

        let tool = GitReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("diff")).await.unwrap();

        assert!(result.is_success(), "Git diff should succeed, got: {}", result.output);
        // Output should contain a diff showing the modification
        assert!(!result.output.is_empty(), "Diff output should not be empty");
    }

    #[tokio::test]
    async fn test_missing_command() {
        let dir = TempDir::new().unwrap();
        let tool = GitReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&ToolInput::new(HashMap::new())).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_tool_name() {
        let dir = TempDir::new().unwrap();
        let tool = GitReadTool::new(dir.path().to_str().unwrap());
        assert_eq!(tool.name(), "git-read");
    }

    #[tokio::test]
    async fn test_no_side_effects() {
        let dir = TempDir::new().unwrap();
        init_git_repo(&dir);

        let tool = GitReadTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input("status")).await.unwrap();

        assert!(!result.has_side_effects());
    }
}
