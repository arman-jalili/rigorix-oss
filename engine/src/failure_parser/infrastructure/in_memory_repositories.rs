//! Implementations of `ParserConfigRepository` and `FailureLogRepository`.
//!
//! @canonical .pi/architecture/modules/failure-parser.md
//! Implements: GAP-A-16 — ParserConfigRepository + FailureLogRepository impls
//!
//! In-memory, Mutex-backed. Persistence of custom parser registrations and
//! failure logs is deliberately in-memory for the current single-process
//! runtime (the trait abstracts the storage so a DB backend can replace it).

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::failure_parser::domain::error::FailureParserError;
use crate::failure_parser::domain::failure::TemplateFailure;
use crate::failure_parser::infrastructure::repository::{
    FailureLogRepository, ParserConfigRepository,
};

fn internal(detail: impl Into<String>) -> FailureParserError {
    FailureParserError::SourceContextError {
        detail: detail.into(),
    }
}

/// In-memory custom-parser registry keyed by tool name.
pub struct InMemoryParserConfigRepository {
    parsers: Mutex<HashMap<String, String>>,
}

impl InMemoryParserConfigRepository {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            parsers: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryParserConfigRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ParserConfigRepository for InMemoryParserConfigRepository {
    async fn store_custom_parser(
        &self,
        tool: &str,
        parser_type: &str,
    ) -> Result<(), FailureParserError> {
        self.parsers
            .lock()
            .map_err(|_| internal("parser registry poisoned"))?
            .insert(tool.to_string(), parser_type.to_string());
        Ok(())
    }

    async fn get_custom_parser(&self, tool: &str) -> Result<Option<String>, FailureParserError> {
        Ok(self
            .parsers
            .lock()
            .map_err(|_| internal("parser registry poisoned"))?
            .get(tool)
            .cloned())
    }

    async fn get_all_custom_parsers(&self) -> Result<HashMap<String, String>, FailureParserError> {
        Ok(self
            .parsers
            .lock()
            .map_err(|_| internal("parser registry poisoned"))?
            .clone())
    }

    async fn remove_custom_parser(&self, tool: &str) -> Result<bool, FailureParserError> {
        Ok(self
            .parsers
            .lock()
            .map_err(|_| internal("parser registry poisoned"))?
            .remove(tool)
            .is_some())
    }
}

/// In-memory failure log: tool -> list of (timestamp, failure).
pub struct InMemoryFailureLogRepository {
    logs: Mutex<HashMap<String, Vec<(String, TemplateFailure)>>>,
}

impl InMemoryFailureLogRepository {
    /// Create an empty log.
    pub fn new() -> Self {
        Self {
            logs: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryFailureLogRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FailureLogRepository for InMemoryFailureLogRepository {
    async fn record_failure(
        &self,
        tool: &str,
        failure: &TemplateFailure,
    ) -> Result<(), FailureParserError> {
        self.record_failures_batch(tool, std::slice::from_ref(failure))
            .await
    }

    async fn record_failures_batch(
        &self,
        tool: &str,
        failures: &[TemplateFailure],
    ) -> Result<(), FailureParserError> {
        let mut logs = self
            .logs
            .lock()
            .map_err(|_| internal("failure log poisoned"))?;
        let entry = logs.entry(tool.to_string()).or_default();
        let now = chrono::Utc::now().to_rfc3339();
        for failure in failures {
            entry.push((now.clone(), failure.clone()));
        }
        Ok(())
    }

    async fn get_recent_failures(
        &self,
        tool: &str,
        limit: usize,
    ) -> Result<Vec<(String, TemplateFailure)>, FailureParserError> {
        let logs = self
            .logs
            .lock()
            .map_err(|_| internal("failure log poisoned"))?;
        let entry = logs.get(tool).cloned().unwrap_or_default();
        Ok(entry.into_iter().rev().take(limit).collect())
    }

    async fn get_failure_stats(
        &self,
        tool: &str,
    ) -> Result<HashMap<String, usize>, FailureParserError> {
        let logs = self
            .logs
            .lock()
            .map_err(|_| internal("failure log poisoned"))?;
        let mut stats = HashMap::new();
        if let Some(entry) = logs.get(tool) {
            for (_, failure) in entry {
                *stats.entry(failure.variant_name().to_string()).or_insert(0) += 1;
            }
        }
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::failure_parser::domain::failure::SourceLocation;

    fn sample_failure() -> TemplateFailure {
        TemplateFailure::MissingSymbol {
            symbol: "missing_fn".to_string(),
            available: vec![],
            suggestion: None,
            location: SourceLocation::new("src/main.rs".to_string(), 10, None),
        }
    }

    #[tokio::test]
    async fn test_parser_config_round_trip() {
        let repo = InMemoryParserConfigRepository::new();
        repo.store_custom_parser("my-tool", "custom_ts")
            .await
            .unwrap();
        assert_eq!(
            repo.get_custom_parser("my-tool").await.unwrap(),
            Some("custom_ts".to_string())
        );
        assert_eq!(repo.get_custom_parser("missing").await.unwrap(), None);
        assert!(repo.remove_custom_parser("my-tool").await.unwrap());
        assert!(!repo.remove_custom_parser("my-tool").await.unwrap());
    }

    #[tokio::test]
    async fn test_failure_log_round_trip() {
        let repo = InMemoryFailureLogRepository::new();
        repo.record_failure("tsc", &sample_failure()).await.unwrap();
        repo.record_failure("tsc", &sample_failure()).await.unwrap();

        let recent = repo.get_recent_failures("tsc", 10).await.unwrap();
        assert_eq!(recent.len(), 2);

        let stats = repo.get_failure_stats("tsc").await.unwrap();
        assert_eq!(stats.get("missing_symbol"), Some(&2));
        assert_eq!(repo.get_recent_failures("other", 5).await.unwrap().len(), 0);
    }
}
