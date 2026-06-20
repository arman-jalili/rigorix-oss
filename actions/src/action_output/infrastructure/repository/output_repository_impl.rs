//! Implementation of `OutputRepository`.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md#outputrepository
//! Implements: OutputRepository — filesystem I/O for stdout, GITHUB_OUTPUT, GITHUB_STEP_SUMMARY
//! Issue: issue-outputformatter
//!
//! # Contract
//! - Implements `OutputRepository` trait from the frozen contract
//! - Writes annotations to stdout (workflow commands)
//! - Writes output variables to `$GITHUB_OUTPUT`
//! - Writes step summaries to `$GITHUB_STEP_SUMMARY`
//! - File paths validated against directory traversal

use async_trait::async_trait;
use std::io::Write;
use std::path::Path;
use tracing::info;

use super::OutputRepository;
use crate::action_output::domain::ActionOutputError;

/// Filesystem-based implementation of `OutputRepository`.
///
/// Uses real I/O to:
/// - `println!` for stdout (workflow commands)
/// - `std::fs::OpenOptions` for `$GITHUB_OUTPUT` and `$GITHUB_STEP_SUMMARY`
///
/// # Security
/// - File paths are validated against directory traversal
/// - Written content is not logged (only metadata)
pub struct OutputRepositoryImpl;

impl OutputRepositoryImpl {
    pub fn new() -> Self {
        Self
    }

    /// Resolve a file path, checking for directory traversal.
    fn resolve_path(path: &str) -> Result<String, ActionOutputError> {
        let p = Path::new(path);
        // Check for directory traversal components
        for component in p.components() {
            if let std::path::Component::ParentDir = component {
                return Err(ActionOutputError::Internal {
                    detail: format!("Directory traversal detected in path: {}", path),
                });
            }
        }
        Ok(path.to_string())
    }

    /// Get the path to `$GITHUB_OUTPUT`, returning `/dev/null` if not set.
    fn get_output_path_from_env() -> String {
        std::env::var("GITHUB_OUTPUT").unwrap_or_else(|_| "/dev/null".to_string())
    }

    /// Get the path to `$GITHUB_STEP_SUMMARY`, returning an error if not set.
    fn get_summary_path_from_env() -> Result<String, ActionOutputError> {
        std::env::var("GITHUB_STEP_SUMMARY")
            .map_err(|_| ActionOutputError::MissingEnv("GITHUB_STEP_SUMMARY".to_string()))
    }
}

#[async_trait]
impl OutputRepository for OutputRepositoryImpl {
    async fn write_stdout(&self, content: &str) -> Result<u64, ActionOutputError> {
        let bytes = content.len() as u64;
        // Use print! to stdout — GitHub Actions runner parses workflow commands
        // from stdout (not stderr)
        print!("{}", content);
        // Flush to ensure the runner sees the command immediately
        std::io::stdout().flush()?;
        Ok(bytes)
    }

    async fn write_output_variable(
        &self,
        name: &str,
        value: &str,
    ) -> Result<u64, ActionOutputError> {
        let path = Self::resolve_path(&Self::get_output_path_from_env())?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| ActionOutputError::WriteError {
                destination: path.clone(),
                detail: e.to_string(),
            })?;

        let line = format!("{}={}\n", name, value);
        let bytes = line.len() as u64;
        file.write_all(line.as_bytes())
            .map_err(|e| ActionOutputError::WriteError {
                destination: path,
                detail: e.to_string(),
            })?;

        Ok(bytes)
    }

    async fn append_summary(&self, markdown: &str) -> Result<u64, ActionOutputError> {
        let path = Self::resolve_path(&Self::get_summary_path_from_env()?)?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| ActionOutputError::WriteError {
                destination: path.clone(),
                detail: e.to_string(),
            })?;

        let bytes = markdown.len() as u64;
        file.write_all(markdown.as_bytes())
            .map_err(|e| ActionOutputError::WriteError {
                destination: path,
                detail: e.to_string(),
            })?;

        Ok(bytes)
    }

    async fn overwrite_summary(&self, markdown: &str) -> Result<u64, ActionOutputError> {
        let path = Self::resolve_path(&Self::get_summary_path_from_env()?)?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .map_err(|e| ActionOutputError::WriteError {
                destination: path.clone(),
                detail: e.to_string(),
            })?;

        let bytes = markdown.len() as u64;
        file.write_all(markdown.as_bytes())
            .map_err(|e| ActionOutputError::WriteError {
                destination: path,
                detail: e.to_string(),
            })?;

        Ok(bytes)
    }

    async fn get_output_path(&self) -> Result<Option<String>, ActionOutputError> {
        let path = Self::get_output_path_from_env();
        if path == "/dev/null" {
            // Only return None if truly not set (not when /dev/null is the fallback)
            if std::env::var("GITHUB_OUTPUT").is_ok() {
                return Ok(Some(path));
            }
            return Ok(None);
        }
        Ok(Some(path))
    }

    async fn get_summary_path(&self) -> Result<Option<String>, ActionOutputError> {
        match Self::get_summary_path_from_env() {
            Ok(path) => Ok(Some(path)),
            Err(_) => Ok(None),
        }
    }

    async fn is_github_actions(&self) -> bool {
        std::env::var("GITHUB_ACTIONS").is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Ensure tests don't run in parallel when modifying env vars
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[tokio::test]
    async fn test_resolve_path_rejects_traversal() {
        let result = OutputRepositoryImpl::resolve_path("../../etc/passwd");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resolve_path_accepts_normal() {
        let result = OutputRepositoryImpl::resolve_path("/tmp/test/file.txt");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_is_github_actions_false_locally() {
        let _lock = ENV_LOCK.lock().unwrap();
        // SAFETY: test-only env manipulation
        unsafe { std::env::remove_var("GITHUB_ACTIONS") };
        let repo = OutputRepositoryImpl::new();
        assert!(!repo.is_github_actions().await);
    }

    #[tokio::test]
    async fn test_write_output_variable_to_tempfile() {
        let _lock = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let output_path = dir.path().join("GITHUB_OUTPUT");
        // SAFETY: test-only env manipulation
        unsafe { std::env::set_var("GITHUB_OUTPUT", output_path.to_str().unwrap()) };

        let repo = OutputRepositoryImpl::new();
        let result = repo.write_output_variable("test_key", "test_value").await;
        assert!(result.is_ok());

        let content = std::fs::read_to_string(&output_path).unwrap();
        assert_eq!(content, "test_key=test_value\n");

        unsafe { std::env::remove_var("GITHUB_OUTPUT") };
    }

    #[tokio::test]
    async fn test_append_summary_to_tempfile() {
        let _lock = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let summary_path = dir.path().join("GITHUB_STEP_SUMMARY");
        // SAFETY: test-only env manipulation
        unsafe { std::env::set_var("GITHUB_STEP_SUMMARY", summary_path.to_str().unwrap()) };

        let repo = OutputRepositoryImpl::new();
        repo.append_summary("# First\n").await.unwrap();
        repo.append_summary("# Second\n").await.unwrap();

        let content = std::fs::read_to_string(&summary_path).unwrap();
        assert_eq!(content, "# First\n# Second\n");

        unsafe { std::env::remove_var("GITHUB_STEP_SUMMARY") };
    }

    #[tokio::test]
    async fn test_overwrite_summary() {
        let _lock = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let summary_path = dir.path().join("GITHUB_STEP_SUMMARY");
        // SAFETY: test-only env manipulation
        unsafe { std::env::set_var("GITHUB_STEP_SUMMARY", summary_path.to_str().unwrap()) };

        // Write initial content
        std::fs::write(&summary_path, "# Old\n").unwrap();

        let repo = OutputRepositoryImpl::new();
        repo.overwrite_summary("# New\n").await.unwrap();

        let content = std::fs::read_to_string(&summary_path).unwrap();
        assert_eq!(content, "# New\n");

        unsafe { std::env::remove_var("GITHUB_STEP_SUMMARY") };
    }

    #[tokio::test]
    async fn test_get_output_path() {
        let _lock = ENV_LOCK.lock().unwrap();
        unsafe { std::env::remove_var("GITHUB_OUTPUT") };
        let repo = OutputRepositoryImpl::new();
        assert!(repo.get_output_path().await.unwrap().is_none());

        unsafe { std::env::set_var("GITHUB_OUTPUT", "/tmp/test") };
        let repo = OutputRepositoryImpl::new();
        assert_eq!(repo.get_output_path().await.unwrap(), Some("/tmp/test".to_string()));
        unsafe { std::env::remove_var("GITHUB_OUTPUT") };
    }
}
