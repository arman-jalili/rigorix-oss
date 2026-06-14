//! In-memory implementation of `PersistedEventRepository`.
//!
//! @canonical .pi/architecture/modules/event-system.md
//! Implements: PersistedEventRepository trait — in-memory event storage
//! Issue: #47
//!
//! Provides thread-safe in-memory storage for persisted events using
//! `Arc<Mutex<Vec<PersistedEvent>>>`. Supports append-only writes,
//! filtered queries, drain operation, and capacity limits.

use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::event_system::application::dto::{EventBusConfig, QueryEventsInput};
use crate::event_system::domain::{EventSystemError, ExecutionEvent, PersistedEvent};
use crate::event_system::infrastructure::repository::PersistedEventRepository;

/// In-memory implementation of `PersistedEventRepository`.
///
/// Stores events in a `Vec<PersistedEvent>` behind an `Arc<Mutex>` for
/// thread-safe concurrent access. Provides bounded capacity with oldest
/// eviction when the buffer is full.
///
/// # Performance
/// - Append is O(1) amortized
/// - Query is O(n) with linear scan (acceptable for event counts < 100K)
/// - Drain is O(n) with full buffer clone
pub struct InMemoryEventRepository {
    /// The in-memory event buffer.
    buffer: Arc<Mutex<Vec<PersistedEvent>>>,

    /// Monotonically increasing sequence counter.
    sequence: AtomicU64,

    /// Whether the buffer has been drained.
    drained: AtomicBool,

    /// Number of events drained.
    drained_count: AtomicU64,

    /// Maximum buffer capacity (0 = unlimited).
    max_capacity: AtomicUsize,
}

impl InMemoryEventRepository {
    /// Create a new empty in-memory repository.
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
            sequence: AtomicU64::new(0),
            drained: AtomicBool::new(false),
            drained_count: AtomicU64::new(0),
            max_capacity: AtomicUsize::new(10_000),
        }
    }

    /// Get the next monotonic sequence number.
    fn next_sequence(&self) -> u64 {
        self.sequence.fetch_add(1, Ordering::SeqCst) + 1
    }
}

impl Default for InMemoryEventRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PersistedEventRepository for InMemoryEventRepository {
    async fn save(&self, event: &PersistedEvent) -> Result<u64, EventSystemError> {
        let sequence = self.next_sequence();
        let mut persisted = event.clone();
        persisted.sequence = sequence;

        let mut buffer = self.buffer.lock().await;
        let max = self.max_capacity.load(Ordering::SeqCst);
        if max > 0 && buffer.len() >= max {
            buffer.remove(0);
        }
        buffer.push(persisted);
        Ok(sequence)
    }

    async fn find_all(&self) -> Result<Vec<PersistedEvent>, EventSystemError> {
        let buffer = self.buffer.lock().await;
        Ok(buffer.clone())
    }

    async fn query(
        &self,
        input: &QueryEventsInput,
    ) -> Result<Vec<PersistedEvent>, EventSystemError> {
        let buffer = self.buffer.lock().await;

        let filtered: Vec<PersistedEvent> = buffer
            .iter()
            .filter(|pe| {
                if let Some(eid) = &input.execution_id {
                    match &pe.event {
                        ExecutionEvent::PlanningStarted { execution_id, .. }
                        | ExecutionEvent::PlanningCompleted { execution_id, .. }
                        | ExecutionEvent::NodeStarted { execution_id, .. }
                        | ExecutionEvent::NodeCompleted { execution_id, .. }
                        | ExecutionEvent::NodeFailed { execution_id, .. }
                        | ExecutionEvent::NodeRetrying { execution_id, .. }
                        | ExecutionEvent::ToolExecuted { execution_id, .. }
                        | ExecutionEvent::ExecutionCompleted { execution_id, .. }
                        | ExecutionEvent::ExecutionFailed { execution_id, .. }
                        | ExecutionEvent::ExecutionCancelled { execution_id, .. }
                        | ExecutionEvent::BudgetWarning { execution_id, .. } => {
                            if execution_id != eid {
                                return false;
                            }
                        }
                    }
                }
                true
            })
            .filter(|pe| {
                if let Some(after) = input.after_sequence {
                    if pe.sequence <= after {
                        return false;
                    }
                }
                true
            })
            .filter(|pe| {
                if let Some(ref event_type) = input.event_type {
                    let variant_name = match &pe.event {
                        ExecutionEvent::PlanningStarted { .. } => "planning_started",
                        ExecutionEvent::PlanningCompleted { .. } => "planning_completed",
                        ExecutionEvent::NodeStarted { .. } => "node_started",
                        ExecutionEvent::NodeCompleted { .. } => "node_completed",
                        ExecutionEvent::NodeFailed { .. } => "node_failed",
                        ExecutionEvent::NodeRetrying { .. } => "node_retrying",
                        ExecutionEvent::ToolExecuted { .. } => "tool_executed",
                        ExecutionEvent::ExecutionCompleted { .. } => "execution_completed",
                        ExecutionEvent::ExecutionFailed { .. } => "execution_failed",
                        ExecutionEvent::ExecutionCancelled { .. } => "execution_cancelled",
                        ExecutionEvent::BudgetWarning { .. } => "budget_warning",
                    };
                    if variant_name != event_type {
                        return false;
                    }
                }
                true
            })
            .filter(|pe| {
                let ts = match &pe.event {
                    ExecutionEvent::PlanningStarted { timestamp, .. } => timestamp,
                    ExecutionEvent::PlanningCompleted { timestamp, .. } => timestamp,
                    ExecutionEvent::NodeStarted { timestamp, .. } => timestamp,
                    ExecutionEvent::NodeCompleted { timestamp, .. } => timestamp,
                    ExecutionEvent::NodeFailed { timestamp, .. } => timestamp,
                    ExecutionEvent::NodeRetrying { timestamp, .. } => timestamp,
                    ExecutionEvent::ToolExecuted { timestamp, .. } => timestamp,
                    ExecutionEvent::ExecutionCompleted { timestamp, .. } => timestamp,
                    ExecutionEvent::ExecutionFailed { timestamp, .. } => timestamp,
                    ExecutionEvent::ExecutionCancelled { timestamp, .. } => timestamp,
                    ExecutionEvent::BudgetWarning { timestamp, .. } => timestamp,
                };
                if let Some(after) = &input.after_timestamp {
                    if ts < after {
                        return false;
                    }
                }
                if let Some(before) = &input.before_timestamp {
                    if ts > before {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        let limit = input.limit.unwrap_or(100) as usize;
        let events: Vec<PersistedEvent> = filtered.into_iter().take(limit).collect();
        Ok(events)
    }

    async fn drain(&self) -> Result<Vec<PersistedEvent>, EventSystemError> {
        if self.drained.load(Ordering::SeqCst) {
            return Err(EventSystemError::AlreadyDrained {
                count: self.drained_count.load(Ordering::SeqCst),
            });
        }

        let mut buffer = self.buffer.lock().await;
        let events = buffer.clone();
        buffer.clear();
        let count = events.len() as u64;
        self.drained.store(true, Ordering::SeqCst);
        self.drained_count.store(count, Ordering::SeqCst);

        Ok(events)
    }

    async fn count(&self) -> Result<u64, EventSystemError> {
        let buffer = self.buffer.lock().await;
        Ok(buffer.len() as u64)
    }

    async fn current_sequence(&self) -> Result<u64, EventSystemError> {
        Ok(self.sequence.load(Ordering::SeqCst))
    }

    async fn prune(
        &self,
        older_than: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, EventSystemError> {
        let mut buffer = self.buffer.lock().await;
        let before = buffer.len();
        buffer.retain(|pe| {
            let ts = match &pe.event {
                ExecutionEvent::PlanningStarted { timestamp, .. } => timestamp,
                ExecutionEvent::PlanningCompleted { timestamp, .. } => timestamp,
                ExecutionEvent::NodeStarted { timestamp, .. } => timestamp,
                ExecutionEvent::NodeCompleted { timestamp, .. } => timestamp,
                ExecutionEvent::NodeFailed { timestamp, .. } => timestamp,
                ExecutionEvent::NodeRetrying { timestamp, .. } => timestamp,
                ExecutionEvent::ToolExecuted { timestamp, .. } => timestamp,
                ExecutionEvent::ExecutionCompleted { timestamp, .. } => timestamp,
                ExecutionEvent::ExecutionFailed { timestamp, .. } => timestamp,
                ExecutionEvent::ExecutionCancelled { timestamp, .. } => timestamp,
                ExecutionEvent::BudgetWarning { timestamp, .. } => timestamp,
            };
            ts >= &older_than
        });
        let removed = (before - buffer.len()) as u64;
        Ok(removed)
    }

    async fn clear(&self) -> Result<u64, EventSystemError> {
        let mut buffer = self.buffer.lock().await;
        let count = buffer.len() as u64;
        buffer.clear();
        self.drained.store(false, Ordering::SeqCst);
        self.drained_count.store(0, Ordering::SeqCst);
        Ok(count)
    }

    async fn is_drained(&self) -> Result<bool, EventSystemError> {
        Ok(self.drained.load(Ordering::SeqCst))
    }

    async fn configure(&self, config: &EventBusConfig) -> Result<(), EventSystemError> {
        self.max_capacity
            .store(config.buffer_capacity, Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_system::domain::ExecutionEvent;
    use chrono::Utc;

    fn sample_event() -> ExecutionEvent {
        ExecutionEvent::NodeStarted {
            execution_id: uuid::Uuid::new_v4(),
            node_id: "node-1".to_string(),
            node_name: "Test Node".to_string(),
            timestamp: Utc::now(),
        }
    }

    fn sample_persisted(sequence: u64) -> PersistedEvent {
        PersistedEvent {
            sequence,
            event: sample_event(),
        }
    }

    #[tokio::test]
    async fn test_save_assigns_sequence() {
        let repo = InMemoryEventRepository::new();
        let event = sample_persisted(0);
        let seq = repo.save(&event).await.unwrap();
        assert_eq!(seq, 1); // First sequence should be 1
    }

    #[tokio::test]
    async fn test_find_all_returns_all() {
        let repo = InMemoryEventRepository::new();
        repo.save(&sample_persisted(0)).await.unwrap();
        repo.save(&sample_persisted(0)).await.unwrap();

        let all = repo.find_all().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_drain_returns_and_clears() {
        let repo = InMemoryEventRepository::new();
        repo.save(&sample_persisted(0)).await.unwrap();
        repo.save(&sample_persisted(0)).await.unwrap();

        let drained = repo.drain().await.unwrap();
        assert_eq!(drained.len(), 2);

        let count = repo.count().await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_double_drain_fails() {
        let repo = InMemoryEventRepository::new();
        repo.save(&sample_persisted(0)).await.unwrap();
        repo.drain().await.unwrap();

        let result = repo.drain().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_clear_resets_state() {
        let repo = InMemoryEventRepository::new();
        repo.save(&sample_persisted(0)).await.unwrap();
        repo.save(&sample_persisted(0)).await.unwrap();

        let cleared = repo.clear().await.unwrap();
        assert_eq!(cleared, 2);

        // After clear, drain should work again
        assert!(!repo.is_drained().await.unwrap());
    }

    #[tokio::test]
    async fn test_prune_removes_old_events() {
        let repo = InMemoryEventRepository::new();

        // Save an event with a past timestamp
        let old_event = PersistedEvent {
            sequence: 0,
            event: ExecutionEvent::NodeStarted {
                execution_id: uuid::Uuid::new_v4(),
                node_id: "old-node".to_string(),
                node_name: "Old Node".to_string(),
                timestamp: Utc::now() - chrono::Duration::hours(2),
            },
        };
        repo.save(&old_event).await.unwrap();

        // Save a recent event
        let new_event = PersistedEvent {
            sequence: 0,
            event: ExecutionEvent::NodeStarted {
                execution_id: uuid::Uuid::new_v4(),
                node_id: "new-node".to_string(),
                node_name: "New Node".to_string(),
                timestamp: Utc::now(),
            },
        };
        repo.save(&new_event).await.unwrap();

        // Prune events older than 1 hour
        let pruned = repo
            .prune(Utc::now() - chrono::Duration::hours(1))
            .await
            .unwrap();
        assert_eq!(pruned, 1);

        let remaining = repo.count().await.unwrap();
        assert_eq!(remaining, 1);
    }

    #[tokio::test]
    async fn test_configure_sets_capacity() {
        let repo = InMemoryEventRepository::new();
        let config = EventBusConfig {
            channel_capacity: 100,
            buffer_capacity: 5, // Small capacity
        };
        repo.configure(&config).await.unwrap();

        // Save 6 events — the 6th should evict the oldest
        for _ in 0..6 {
            repo.save(&sample_persisted(0)).await.unwrap();
        }

        let count = repo.count().await.unwrap();
        assert_eq!(count, 5); // Only 5 fit within capacity
    }

    #[tokio::test]
    async fn test_query_by_execution_id() {
        let repo = InMemoryEventRepository::new();
        let eid1 = uuid::Uuid::new_v4();
        let eid2 = uuid::Uuid::new_v4();

        repo.save(&PersistedEvent {
            sequence: 0,
            event: ExecutionEvent::NodeStarted {
                execution_id: eid1,
                node_id: "n1".to_string(),
                node_name: "Node 1".to_string(),
                timestamp: Utc::now(),
            },
        })
        .await
        .unwrap();

        repo.save(&PersistedEvent {
            sequence: 0,
            event: ExecutionEvent::NodeStarted {
                execution_id: eid2,
                node_id: "n2".to_string(),
                node_name: "Node 2".to_string(),
                timestamp: Utc::now(),
            },
        })
        .await
        .unwrap();

        let query = QueryEventsInput {
            execution_id: Some(eid1),
            event_type: None,
            after_sequence: None,
            limit: None,
            after_timestamp: None,
            before_timestamp: None,
        };

        let results = repo.query(&query).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_current_sequence() {
        let repo = InMemoryEventRepository::new();
        assert_eq!(repo.current_sequence().await.unwrap(), 0);

        repo.save(&sample_persisted(0)).await.unwrap();
        assert_eq!(repo.current_sequence().await.unwrap(), 1);

        repo.save(&sample_persisted(0)).await.unwrap();
        assert_eq!(repo.current_sequence().await.unwrap(), 2);
    }
}
