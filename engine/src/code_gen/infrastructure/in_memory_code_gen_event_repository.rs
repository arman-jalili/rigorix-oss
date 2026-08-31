//! In-memory `CodeGenEventRepository` implementation.
//!
//! @canonical .pi/architecture/modules/code-generation.md#events
//! Implements: GAP-A-16 — CodeGenEventRepository impl
//!
//! Stores code generation events in memory for audit/observability, with
//! session/path querying and timestamp-based pruning.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Mutex;

use crate::code_gen::domain::error::CodeGenError;
use crate::code_gen::domain::event::CodeGenEvent;
use crate::code_gen::infrastructure::repository::CodeGenEventRepository;

/// In-memory event log.
pub struct InMemoryCodeGenEventRepository {
    events: Mutex<Vec<CodeGenEvent>>,
}

impl InMemoryCodeGenEventRepository {
    /// Create an empty event log.
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }
}

impl Default for InMemoryCodeGenEventRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryCodeGenEventRepository {
    fn session_id(event: &CodeGenEvent) -> &str {
        match event {
            CodeGenEvent::EditFileStarted { session_id, .. }
            | CodeGenEvent::EditFileCompleted { session_id, .. }
            | CodeGenEvent::EditFileFailed { session_id, .. }
            | CodeGenEvent::ReadFileCompleted { session_id, .. }
            | CodeGenEvent::SyntaxGateApplied { session_id, .. } => session_id,
        }
    }

    fn file_path(event: &CodeGenEvent) -> &str {
        match event {
            CodeGenEvent::EditFileStarted { file_path, .. }
            | CodeGenEvent::EditFileCompleted { file_path, .. }
            | CodeGenEvent::EditFileFailed { file_path, .. }
            | CodeGenEvent::ReadFileCompleted { file_path, .. }
            | CodeGenEvent::SyntaxGateApplied { file_path, .. } => file_path,
        }
    }

    fn timestamp(event: &CodeGenEvent) -> DateTime<Utc> {
        match event {
            CodeGenEvent::EditFileStarted { timestamp, .. }
            | CodeGenEvent::EditFileCompleted { timestamp, .. }
            | CodeGenEvent::EditFileFailed { timestamp, .. }
            | CodeGenEvent::ReadFileCompleted { timestamp, .. }
            | CodeGenEvent::SyntaxGateApplied { timestamp, .. } => *timestamp,
        }
    }
}

#[async_trait]
impl CodeGenEventRepository for InMemoryCodeGenEventRepository {
    async fn record_event(&self, event: &CodeGenEvent) -> Result<(), CodeGenError> {
        self.events
            .lock()
            .map_err(|_| CodeGenError::Internal {
                detail: "event log poisoned".to_string(),
            })?
            .push(event.clone());
        Ok(())
    }

    async fn query_by_session(&self, session_id: &str) -> Result<Vec<CodeGenEvent>, CodeGenError> {
        let events = self.events.lock().map_err(|_| CodeGenError::Internal {
            detail: "event log poisoned".to_string(),
        })?;
        Ok(events
            .iter()
            .filter(|e| Self::session_id(e) == session_id)
            .cloned()
            .collect())
    }

    async fn query_by_path(&self, file_path: &str) -> Result<Vec<CodeGenEvent>, CodeGenError> {
        let events = self.events.lock().map_err(|_| CodeGenError::Internal {
            detail: "event log poisoned".to_string(),
        })?;
        Ok(events
            .iter()
            .filter(|e| Self::file_path(e) == file_path)
            .cloned()
            .collect())
    }

    async fn event_count(&self) -> Result<u64, CodeGenError> {
        Ok(self
            .events
            .lock()
            .map_err(|_| CodeGenError::Internal {
                detail: "event log poisoned".to_string(),
            })?
            .len() as u64)
    }

    async fn prune(&self, older_than: DateTime<Utc>) -> Result<u64, CodeGenError> {
        let mut events = self.events.lock().map_err(|_| CodeGenError::Internal {
            detail: "event log poisoned".to_string(),
        })?;
        let before = events.len();
        events.retain(|e| Self::timestamp(e) >= older_than);
        Ok((before - events.len()) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_event(session: &str, path: &str) -> CodeGenEvent {
        CodeGenEvent::EditFileStarted {
            session_id: session.to_string(),
            file_path: path.to_string(),
            old_string_length: 1,
            new_string_length: 2,
            timestamp: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_event_round_trip_and_query() {
        let repo = InMemoryCodeGenEventRepository::new();
        repo.record_event(&sample_event("s1", "a.rs"))
            .await
            .unwrap();
        repo.record_event(&sample_event("s1", "b.rs"))
            .await
            .unwrap();
        repo.record_event(&sample_event("s2", "a.rs"))
            .await
            .unwrap();

        assert_eq!(repo.event_count().await.unwrap(), 3);
        assert_eq!(repo.query_by_session("s1").await.unwrap().len(), 2);
        assert_eq!(repo.query_by_path("a.rs").await.unwrap().len(), 2);

        let pruned = repo
            .prune(Utc::now() + chrono::Duration::seconds(1))
            .await
            .unwrap();
        assert_eq!(pruned, 3);
        assert_eq!(repo.event_count().await.unwrap(), 0);
    }
}
