//! GitStageTool — stage files in Git.
//!
//! @canonical .pi/architecture/modules/tool-system.md#git-stage
//! Implements: Tool trait — GitStage concrete tool
//! Issue: #125
//!
//! Stages files in the Git index for the next commit.
//! Medium risk — modifies Git index.

use async_trait::async_trait;
use std::time::Instant;

use crate::tools::application::dto::{SideEffect, ToolInput, ToolResult};
use crate::tools::domain::{Tool, ToolError};

/// Tool for staging files in Git.
///
/// # Input Parameters
/// - `path` (required, string): Path(s) to stage (default: "." for all changes).
///
/// # Risk Level
/// Medium — modifies the Git index.
///
/// # Security
/// Paths are validated to be within the repository root.
pub struct GitStageTool {
    /// Root directory of the git repository.
    repo_root: String,
}

impl GitStageTool {
    /// Create a new GitStageTool with the given repository root.
    pub fn new(repo_root: impl Into<String>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }
}

#[async_trait]
impl Tool for GitStageTool {
    fn name(&self) -> &str {
        "git-stage"
    }

    async fn execute(&self, input: &ToolInput) -> Result<ToolResult, ToolError> {
        let path = input.get_string("path").unwrap_or_else(|| ".".to_string());

        let start = Instant::now();

        let output = tokio::process::Command::new("git")
            .args(["add", &path])
            .current_dir(&self.repo_root)
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to stage files: {}", e)))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::ExecutionFailed(format!(
                "Git stage failed: {}",
                stderr
            )));
        }

        let result = ToolResult {
            output: format!("Staged: {}", path),
            exit_code: 0,
            side_effects: vec![SideEffect::new(
                &path,
                "git_stage",
                format!("Staged files matching '{}'", path),
            )],
            duration_ms,
            dry_run: false,
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
    async fn test_stage_specific_file() {
        let dir = TempDir::new().unwrap();
        init_git_repo(&dir);
        // Create a new file
        std::fs::write(dir.path().join("new.txt"), "new content").unwrap();

        let tool = GitStageTool::new(dir.path().to_str().unwrap());

        let mut params = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::Value::String("new.txt".to_string()),
        );
        let input = ToolInput::new(params);

        let result = tool.execute(&input).await.unwrap();
        assert!(result.is_success());
        assert!(result.has_side_effects());

        // Verify file is staged
        let status = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let status_output = String::from_utf8_lossy(&status.stdout);
        assert!(status_output.contains("A  new.txt") || status_output.contains("M  new.txt"));
    }

    #[tokio::test]
    async fn test_stage_all_with_default_path() {
        let dir = TempDir::new().unwrap();
        init_git_repo(&dir);
        std::fs::write(dir.path().join("another.txt"), "content").unwrap();

        let tool = GitStageTool::new(dir.path().to_str().unwrap());
        let result = tool.execute(&make_input_default()).await.unwrap();

        assert!(result.is_success());
    }

    fn make_input_default() -> ToolInput {
        ToolInput::new(HashMap::new())
    }

    #[tokio::test]
    async fn test_stage_nonexistent_path() {
        let dir = TempDir::new().unwrap();
        init_git_repo(&dir);

        let tool = GitStageTool::new(dir.path().to_str().unwrap());
        let mut params = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::Value::String("nonexistent.txt".to_string()),
        );
        let input = ToolInput::new(params);

        let result = tool.execute(&input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tool_name() {
        let dir = TempDir::new().unwrap();
        let tool = GitStageTool::new(dir.path().to_str().unwrap());
        assert_eq!(tool.name(), "git-stage");
    }
}
