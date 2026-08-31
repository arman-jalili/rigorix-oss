//! Implementations of `PatternRepository` and `ClassificationLogRepository`.
//!
//! @canonical .pi/architecture/modules/failure-classification.md
//! Implements: GAP-A-16 — pattern store + classification log impls
//!
//! In-memory, Mutex-backed. Patterns are keyed by pattern string -> FailureType;
//! the classification log keeps per-pattern history with timestamps.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::failure_classification::domain::error::FailureClassificationError;
use crate::failure_classification::domain::failure_type::FailureType;
use crate::failure_classification::infrastructure::repository::{
    ClassificationLogRepository, PatternRepository,
};

/// In-memory pattern -> FailureType registry.
pub struct InMemoryPatternRepository {
    patterns: Mutex<HashMap<String, FailureType>>,
}

impl InMemoryPatternRepository {
    /// Create an empty pattern registry.
    pub fn new() -> Self {
        Self {
            patterns: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryPatternRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PatternRepository for InMemoryPatternRepository {
    async fn store_pattern(
        &self,
        pattern: &str,
        target: FailureType,
    ) -> Result<u32, FailureClassificationError> {
        let mut patterns =
            self.patterns
                .lock()
                .map_err(|_| FailureClassificationError::PatternRepository {
                    detail: "pattern registry poisoned".to_string(),
                })?;
        patterns.insert(pattern.to_string(), target);
        Ok(patterns.len() as u32)
    }

    async fn get_pattern(
        &self,
        pattern: &str,
    ) -> Result<Option<FailureType>, FailureClassificationError> {
        Ok(self
            .patterns
            .lock()
            .map_err(|_| FailureClassificationError::PatternRepository {
                detail: "pattern registry poisoned".to_string(),
            })?
            .get(pattern)
            .cloned())
    }

    async fn get_all_patterns(
        &self,
    ) -> Result<HashMap<String, FailureType>, FailureClassificationError> {
        Ok(self
            .patterns
            .lock()
            .map_err(|_| FailureClassificationError::PatternRepository {
                detail: "pattern registry poisoned".to_string(),
            })?
            .clone())
    }

    async fn remove_pattern(&self, pattern: &str) -> Result<bool, FailureClassificationError> {
        Ok(self
            .patterns
            .lock()
            .map_err(|_| FailureClassificationError::PatternRepository {
                detail: "pattern registry poisoned".to_string(),
            })?
            .remove(pattern)
            .is_some())
    }

    async fn clear_patterns(&self) -> Result<(), FailureClassificationError> {
        self.patterns
            .lock()
            .map_err(|_| FailureClassificationError::PatternRepository {
                detail: "pattern registry poisoned".to_string(),
            })?
            .clear();
        Ok(())
    }
}

/// In-memory classification history: pattern -> [(timestamp, FailureType)].
pub struct InMemoryClassificationLogRepository {
    log: Mutex<HashMap<String, Vec<(String, FailureType)>>>,
}

impl InMemoryClassificationLogRepository {
    /// Create an empty classification log.
    pub fn new() -> Self {
        Self {
            log: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryClassificationLogRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ClassificationLogRepository for InMemoryClassificationLogRepository {
    async fn record_classification(
        &self,
        error_message: &str,
        failure_type: &FailureType,
    ) -> Result<(), FailureClassificationError> {
        let mut log =
            self.log
                .lock()
                .map_err(|_| FailureClassificationError::PatternRepository {
                    detail: "classification log poisoned".to_string(),
                })?;
        log.entry(error_message.to_string())
            .or_default()
            .push((chrono::Utc::now().to_rfc3339(), failure_type.clone()));
        Ok(())
    }

    async fn get_classification_history(
        &self,
        error_pattern: &str,
        limit: usize,
    ) -> Result<Vec<(String, FailureType)>, FailureClassificationError> {
        let log = self
            .log
            .lock()
            .map_err(|_| FailureClassificationError::PatternRepository {
                detail: "classification log poisoned".to_string(),
            })?;
        let entry = log.get(error_pattern).cloned().unwrap_or_default();
        Ok(entry.into_iter().rev().take(limit).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pattern_round_trip() {
        let repo = InMemoryPatternRepository::new();
        repo.store_pattern("network timeout", FailureType::Transient)
            .await
            .unwrap();
        assert_eq!(
            repo.get_pattern("network timeout").await.unwrap(),
            Some(FailureType::Transient)
        );
        assert!(repo.remove_pattern("network timeout").await.unwrap());
        assert!(repo.get_all_patterns().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_classification_log_round_trip() {
        let repo = InMemoryClassificationLogRepository::new();
        repo.record_classification("error: build failed", &FailureType::BuildFailure)
            .await
            .unwrap();
        repo.record_classification("error: build failed", &FailureType::BuildFailure)
            .await
            .unwrap();
        let history = repo
            .get_classification_history("error: build failed", 5)
            .await
            .unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].1, FailureType::BuildFailure);
    }
}
