//! Implementation of `RiskClassificationService`.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md#risk
//! Implements: RiskClassificationService trait — classifies files by risk level
//! Issue: #556
//!
//! The RiskClassifier assigns a risk level to each changed file based on
//! path patterns:
//!
//! - **Critical**: auth/, security/, credentials, access control
//! - **High**: migrations/, sql, database schemas, infrastructure config
//! - **Medium**: Source code (.rs, .ts, .py, .js, .go, .kt, .java, .swift)
//! - **Low**: Documentation (.md, .txt), config (.json, .yaml, .toml, .xml)
//!
//! Custom patterns can override the defaults.

use async_trait::async_trait;
use std::collections::HashMap;

use crate::diff_analyzer::application::dto::{
    ClassifyRiskInput, ClassifyRiskOutput, FileClassificationResult,
};
use crate::diff_analyzer::application::service::RiskClassificationService;
use crate::diff_analyzer::domain::{DiffAnalyzerError, FileRisk};

/// Implementation of `RiskClassificationService`.
///
/// Uses path-based heuristics to classify files by risk level.
/// Default patterns are applied unless overridden by custom patterns.
pub struct RiskClassifierImpl;

impl RiskClassifierImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RiskClassifierImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RiskClassificationService for RiskClassifierImpl {
    async fn classify(
        &self,
        input: ClassifyRiskInput,
    ) -> Result<ClassifyRiskOutput, DiffAnalyzerError> {
        let custom_patterns = input.custom_patterns.unwrap_or_default();

        let mut classifications = Vec::new();
        let mut critical_files = Vec::new();
        let mut high_risk_files = Vec::new();

        for file in &input.diff.files {
            let result = self.classify_path(&file.path, &custom_patterns).await?;
            classifications.push(result.clone());

            match result.risk {
                FileRisk::Critical => critical_files.push(file.path.clone()),
                FileRisk::High => high_risk_files.push(file.path.clone()),
                _ => {}
            }
        }

        // Update risk on the diff's files
        let mut diff = input.diff;
        for file in &mut diff.files {
            if let Some(class) = classifications.iter().find(|c| c.path == file.path) {
                file.risk = class.risk;
            }
        }

        Ok(ClassifyRiskOutput {
            diff,
            classifications,
            critical_files,
            high_risk_files,
        })
    }

    async fn classify_path(
        &self,
        path: &str,
        custom_patterns: &HashMap<String, FileRisk>,
    ) -> Result<FileClassificationResult, DiffAnalyzerError> {
        // Check custom patterns first (highest priority)
        for (pattern, risk) in custom_patterns {
            if self.matches_pattern(path, pattern).await {
                return Ok(FileClassificationResult {
                    path: path.to_string(),
                    risk: *risk,
                    matched_pattern: Some(pattern.clone()),
                });
            }
        }

        // Check against default patterns
        let path_lower = path.to_lowercase();

        // Critical: Auth, security, credentials
        if path_lower.contains("auth/")
            || path_lower.contains("security/")
            || path_lower.contains("credentials")
            || path_lower.contains("secrets")
            || path_lower.contains(".env")
            || path_lower.contains("access-control")
            || path_lower.contains("oauth")
            || path_lower.contains("jwt")
        {
            return Ok(FileClassificationResult {
                path: path.to_string(),
                risk: FileRisk::Critical,
                matched_pattern: Some("auth/security".to_string()),
            });
        }

        // High: Migrations, SQL, database, infrastructure
        if path_lower.contains("migrations/")
            || path_lower.ends_with(".sql")
            || path_lower.contains("database/")
            || path_lower.contains("infrastructure/")
            || path_lower.contains("deployment/")
            || path_lower.contains("kubernetes")
            || path_lower.contains("docker")
            || path_lower.contains("terraform")
            || path_lower.contains("cloudformation")
        {
            return Ok(FileClassificationResult {
                path: path.to_string(),
                risk: FileRisk::High,
                matched_pattern: Some("migrations/infrastructure".to_string()),
            });
        }

        // Low: Documentation, config, text
        if path_lower.ends_with(".md")
            || path_lower.ends_with(".txt")
            || path_lower.ends_with(".json")
            || path_lower.ends_with(".yaml")
            || path_lower.ends_with(".yml")
            || path_lower.ends_with(".toml")
            || path_lower.ends_with(".xml")
            || path_lower.ends_with(".html")
            || path_lower.ends_with(".css")
            || path_lower.ends_with(".svg")
            || path_lower.ends_with(".png")
            || path_lower.ends_with(".jpg")
            || path_lower.ends_with(".jpeg")
            || path_lower.ends_with(".gif")
            || path_lower.ends_with(".ico")
        {
            return Ok(FileClassificationResult {
                path: path.to_string(),
                risk: FileRisk::Low,
                matched_pattern: Some("documentation/config".to_string()),
            });
        }

        // Medium: Source code (default for most source files)
        // Check by extension
        let source_extensions = [
            "rs", "ts", "tsx", "js", "jsx", "py", "go", "kt", "java", "swift", "c", "h", "cpp",
            "hpp", "cs", "rb", "php", "scala", "clj", "ex", "exs", "erl", "hs", "elm", "vue",
            "svelte", "dart",
        ];

        if source_extensions
            .iter()
            .any(|ext| path_lower.ends_with(&format!(".{}", ext)))
        {
            return Ok(FileClassificationResult {
                path: path.to_string(),
                risk: FileRisk::Medium,
                matched_pattern: Some("source_code".to_string()),
            });
        }

        // Default to Medium for unknown file types
        Ok(FileClassificationResult {
            path: path.to_string(),
            risk: FileRisk::Medium,
            matched_pattern: Some("default".to_string()),
        })
    }

    async fn default_risk(&self) -> FileRisk {
        FileRisk::Medium
    }

    async fn matches_pattern(&self, path: &str, pattern: &str) -> bool {
        if pattern.contains('*') || pattern.contains('?') {
            let regex_pattern = pattern
                .replace('.', "\\.")
                .replace('*', ".*")
                .replace('?', ".");
            if let Ok(re) = regex::Regex::new(&format!("^{}$", regex_pattern)) {
                return re.is_match(path);
            }
        }
        // Also check if pattern is a prefix match (directory-based)
        if pattern.ends_with('/') {
            return path.starts_with(pattern);
        }
        path == pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff_analyzer::domain::PrDiff;
    use crate::diff_analyzer::domain::{ChangedFile, FileStatus};

    fn make_classifier() -> RiskClassifierImpl {
        RiskClassifierImpl::new()
    }

    fn make_diff_with_paths(paths: Vec<&str>) -> PrDiff {
        PrDiff {
            files: paths
                .into_iter()
                .map(|p| ChangedFile {
                    path: p.to_string(),
                    status: FileStatus::Modified,
                    additions: 1,
                    deletions: 0,
                    is_binary: false,
                    hunks: Vec::new(),
                    risk: FileRisk::Medium,
                    raw_diff: None,
                })
                .collect(),
            total_size_bytes: 0,
            excluded_files: Vec::new(),
            limits_exceeded: false,
            policy_modified: false,
            ai_signals: None,
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_classify_low_risk() {
        let classifier = make_classifier();
        let paths = vec![
            "README.md",
            "docs/guide.txt",
            "config.yaml",
            "package.json",
            "Cargo.toml",
        ];
        let diff = make_diff_with_paths(paths);
        let input = ClassifyRiskInput {
            diff,
            custom_patterns: None,
        };
        let result = classifier.classify(input).await.unwrap();
        assert_eq!(
            result
                .classifications
                .iter()
                .filter(|c| c.risk == FileRisk::Low)
                .count(),
            5
        );
    }

    #[tokio::test]
    async fn test_classify_medium_risk() {
        let classifier = make_classifier();
        let paths = vec!["src/main.rs", "lib/types.ts", "app.py", "cmd/main.go"];
        let diff = make_diff_with_paths(paths);
        let input = ClassifyRiskInput {
            diff,
            custom_patterns: None,
        };
        let result = classifier.classify(input).await.unwrap();
        assert_eq!(
            result
                .classifications
                .iter()
                .filter(|c| c.risk == FileRisk::Medium)
                .count(),
            4
        );
    }

    #[tokio::test]
    async fn test_classify_high_risk() {
        let classifier = make_classifier();
        let paths = vec![
            "db/migrations/001_init.sql",
            "infrastructure/main.tf",
            "docker/Dockerfile",
        ];
        let diff = make_diff_with_paths(paths);
        let input = ClassifyRiskInput {
            diff,
            custom_patterns: None,
        };
        let result = classifier.classify(input).await.unwrap();
        assert_eq!(result.high_risk_files.len(), 3);
    }

    #[tokio::test]
    async fn test_classify_critical_risk() {
        let classifier = make_classifier();
        let paths = vec![
            "src/auth/login.rs",
            "security/policy.toml",
            "credentials/prod.env",
        ];
        let diff = make_diff_with_paths(paths);
        let input = ClassifyRiskInput {
            diff,
            custom_patterns: None,
        };
        let result = classifier.classify(input).await.unwrap();
        assert_eq!(result.critical_files.len(), 3);
    }

    #[tokio::test]
    async fn test_classify_single_path_low() {
        let classifier = make_classifier();
        let result = classifier
            .classify_path("README.md", &HashMap::new())
            .await
            .unwrap();
        assert_eq!(result.risk, FileRisk::Low);
    }

    #[tokio::test]
    async fn test_classify_single_path_medium() {
        let classifier = make_classifier();
        let result = classifier
            .classify_path("src/main.rs", &HashMap::new())
            .await
            .unwrap();
        assert_eq!(result.risk, FileRisk::Medium);
    }

    #[tokio::test]
    async fn test_classify_single_path_high() {
        let classifier = make_classifier();
        let result = classifier
            .classify_path("db/migrations/001.sql", &HashMap::new())
            .await
            .unwrap();
        assert_eq!(result.risk, FileRisk::High);
    }

    #[tokio::test]
    async fn test_classify_single_path_critical() {
        let classifier = make_classifier();
        let result = classifier
            .classify_path("src/auth/login.rs", &HashMap::new())
            .await
            .unwrap();
        assert_eq!(result.risk, FileRisk::Critical);
    }

    #[tokio::test]
    async fn test_custom_patterns_override() {
        let classifier = make_classifier();
        let mut custom = HashMap::new();
        custom.insert("test.custom".to_string(), FileRisk::Critical);
        let result = classifier
            .classify_path("test.custom", &custom)
            .await
            .unwrap();
        assert_eq!(result.risk, FileRisk::Critical);
    }

    #[tokio::test]
    async fn test_default_risk() {
        let classifier = make_classifier();
        let risk = classifier.default_risk().await;
        assert_eq!(risk, FileRisk::Medium);
    }

    #[tokio::test]
    async fn test_classify_updates_diff() {
        let classifier = make_classifier();
        let diff = make_diff_with_paths(vec!["src/main.rs", "src/auth/login.rs"]);
        let input = ClassifyRiskInput {
            diff,
            custom_patterns: None,
        };
        let result = classifier.classify(input).await.unwrap();
        assert_eq!(result.diff.files[0].risk, FileRisk::Medium);
        assert_eq!(result.diff.files[1].risk, FileRisk::Critical);
    }

    #[tokio::test]
    async fn test_unknown_extension_defaults_medium() {
        let classifier = make_classifier();
        let result = classifier
            .classify_path("assets/weird.xyz", &HashMap::new())
            .await
            .unwrap();
        assert_eq!(result.risk, FileRisk::Medium);
        assert_eq!(result.matched_pattern.as_deref(), Some("default"));
    }
}
