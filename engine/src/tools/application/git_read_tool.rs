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
//!
//! # No shell (#1082, class C2)
//!
//! The command is executed as **argv**, never through `sh -c`. It used to be
//! built by string concatenation (`git {command} -- {path}`) and handed to a
//! shell, so the first token allowlist validated a string the executor did not
//! run: `path = "x; touch pwned"` passed the check and then executed. argv
//! removes the interpreter from the path, so a metacharacter is just a byte in
//! an argument.

// #1083: approved process-spawning sink — fixed argv (`Command::new("git")`), never `sh -c`; the caller-supplied path is a separate argv element, not interpolated (#1082).
// See `.pi/architecture/modules/tool-system.md#process-spawning-sinks`.
#![allow(clippy::disallowed_types)]

use async_trait::async_trait;
use std::time::Instant;

use crate::tools::application::dto::{ToolInput, ToolResult};
use crate::tools::application::shell_policy::apply_child_env;
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

/// Flags that make a nominally read-only git command write a file or run
/// something else. Defence in depth: argv is what removes the shell (class C2),
/// this keeps a read-only command read-only even when the caller appends flags.
const WRITE_OR_EXEC_FLAGS: &[&str] = &[
    "--output",
    "--ext-diff",
    "--textconv",
    "--upload-pack",
    "--exec",
    "--open-files-in-pager",
    "--no-index",
    "-c",
    "--config-env",
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

        // argv, never a shell: the allowlist below validates the same tokens
        // the executor runs (#1082, class C2).
        let tokens: Vec<&str> = command.split_whitespace().collect();
        for token in &tokens {
            let flag = token.split('=').next().unwrap_or(token);
            if WRITE_OR_EXEC_FLAGS.contains(&flag) {
                return Err(ToolError::InvalidInput(format!(
                    "Git flag '{}' is not a read-only operation",
                    flag
                )));
            }
        }

        let mut args: Vec<String> = tokens.iter().map(|t| t.to_string()).collect();
        // Bound the result set for log commands.
        if tokens.first() == Some(&"log") {
            args.push(format!("--max-count={max_results}"));
        }
        if let Some(p) = &path {
            args.push("--".to_string());
            args.push(p.clone());
        }

        // The child inherits only the explicit allowlist, so a git command
        // cannot read `RIGORIX_HMAC_KEY` or anchor credentials (#1068).
        let mut cmd = tokio::process::Command::new("git");
        cmd.args(&args).current_dir(&self.repo_root);
        apply_child_env(&mut cmd, &[]);
        let output = cmd.output().await.map_err(|e| {
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
        params.insert(
            "command".to_string(),
            serde_json::Value::String(command.to_string()),
        );
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

    /// #1082 class C2: the allowlist used to validate a string that was then
    /// handed to `sh -c`, so a caller path could append a command. argv removes
    /// the interpreter: the metacharacters stay inside one argument.
    #[tokio::test]
    async fn caller_path_cannot_inject_a_shell_command() {
        let dir = TempDir::new().unwrap();
        init_git_repo(&dir);
        let marker = dir.path().join("pwned");
        assert!(!marker.exists());

        let tool = GitReadTool::new(dir.path().to_str().unwrap());
        let mut params = std::collections::HashMap::new();
        params.insert(
            "command".to_string(),
            serde_json::Value::String("status".to_string()),
        );
        params.insert(
            "path".to_string(),
            serde_json::Value::String(format!("; touch {}", marker.display())),
        );

        // The call may fail (git rejects the path); what matters is that nothing
        // executed the injected text.
        let _ = tool.execute(&ToolInput::new(params)).await;
        assert!(
            !marker.exists(),
            "the caller path must never reach a shell (class C2)"
        );
    }

    /// A nominally read-only command must stay read-only when the caller appends
    /// a flag that writes a file (#1082, class C2 defence in depth).
    #[tokio::test]
    async fn write_flags_are_refused() {
        let dir = TempDir::new().unwrap();
        init_git_repo(&dir);
        let tool = GitReadTool::new(dir.path().to_str().unwrap());
        let result = tool
            .execute(&make_input("diff --output=/tmp/rigorix-should-not-exist"))
            .await;
        assert!(result.is_err(), "a write flag must be refused");
        assert!(!std::path::Path::new("/tmp/rigorix-should-not-exist").exists());
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

        assert!(
            result.is_success(),
            "Git diff should succeed, got: {}",
            result.output
        );
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
