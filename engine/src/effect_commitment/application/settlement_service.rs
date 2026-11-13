//! Open a settlement before the first effect, close it on every exit path, and never resume over an
//! ambiguity (#1528).
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#the-aggregate
//! Implements: ADR-019's write-ahead rule, ADR-024 §7
//!
//! # The drop guard, and why it is not belt-and-braces
//!
//! "Closed on every exit path" includes the path where the dispatch future panics: a `Drop` impl
//! cannot await, so the guard cannot write the store — but it *can* make the absence of a terminal
//! the honest answer, by leaving the record without one. A reopened store then reads `Ambiguous`,
//! which is exactly the statement available to a process that did not survive its own dispatch.
//! The guard's job is therefore not to close the run; it is to make sure a run that did not close
//! never looks closed.
//!
//! That distinction is why the test asserts on **state after reopening**, not on timing, and why a
//! real process kill is a host-level test (`cli`/`mcp`) rather than one here: `rigorix-engine` is a
//! library, so there is no process in this crate to kill.

use std::sync::Arc;

use uuid::Uuid;

use crate::effect_commitment::domain::settlement::{
    AmbiguousSettlement, ExecutionSettlement, SettlementState, SettlementTerminal,
    refuse_resume_on_ambiguity,
};
use crate::state_persistence::domain::StateError;

/// The port this service needs: three writes and two reads, over the state repository that already
/// holds durable records (`ADR-024 §7` — not the effect ledger's port, whose only local adapter is a
/// `HashMap`, which a crash erases).
#[async_trait::async_trait]
pub trait SettlementStore: Send + Sync {
    /// Write the settlement, atomically (tmp + rename).
    async fn put(&self, settlement: &ExecutionSettlement) -> Result<(), StateError>;
    /// Read one settlement.
    async fn get(&self, execution_id: Uuid) -> Result<ExecutionSettlement, StateError>;
    /// Every settlement in the store.
    async fn list(&self) -> Result<Vec<ExecutionSettlement>, StateError>;
}

/// The use case: one settlement per execution, opened first and closed on every exit path.
pub struct SettlementService {
    store: Arc<dyn SettlementStore>,
}

impl SettlementService {
    /// Build over a store.
    #[must_use]
    pub fn new(store: Arc<dyn SettlementStore>) -> Self {
        Self { store }
    }

    /// Open a settlement. Call this **before** the first `begin`: a crash must have something to
    /// reopen into, or the effect records it left are attributed to nothing.
    ///
    /// # Errors
    /// The store's write error.
    pub async fn open(
        &self,
        execution_id: Uuid,
        receipt_digest: Option<String>,
        started_at: String,
    ) -> Result<SettlementGuard, StateError> {
        let settlement = ExecutionSettlement::open(execution_id, receipt_digest, started_at);
        self.store.put(&settlement).await?;
        Ok(SettlementGuard {
            service: self.clone_ref(),
            settlement,
            closed: false,
        })
    }

    /// Refuse a resume over an ambiguous settlement of this receipt.
    ///
    /// # Errors
    /// `AmbiguousSettlement`, naming the record ids — and nothing is dispatched, because this check
    /// runs before the graph is built.
    pub async fn refuse_resume(&self, receipt_digest: &str) -> Result<(), AmbiguousSettlement> {
        let settlements = match self.store.list().await {
            Ok(settlements) => settlements,
            // A store that cannot be read is not evidence of safety: refusing to relaunch on an
            // unreadable history is the conservative direction, and the ids are unknown so the
            // refusal names the receipt instead of inventing records.
            Err(_) => {
                return Err(AmbiguousSettlement {
                    execution_id: Uuid::nil(),
                    receipt_digest: Some(receipt_digest.to_string()),
                    record_ids: vec!["<store unreadable>".to_string()],
                });
            }
        };
        refuse_resume_on_ambiguity(&settlements, receipt_digest)
    }

    /// The settlements whose ambiguity concerns this receipt — the query surface.
    ///
    /// # Errors
    /// The store's read error.
    pub async fn ambiguities_for(
        &self,
        receipt_digest: &str,
    ) -> Result<Vec<ExecutionSettlement>, StateError> {
        let settlements = self.store.list().await?;
        Ok(settlements
            .into_iter()
            .filter(|s| s.receipt_digest.as_deref() == Some(receipt_digest))
            .filter(|s| matches!(s.terminal_as_read(), SettlementTerminal::Ambiguous))
            .collect())
    }

    /// The settlement for one execution, as a store reopens it.
    ///
    /// # Errors
    /// The store's read error.
    pub async fn settlement_for(
        &self,
        execution_id: Uuid,
    ) -> Result<ExecutionSettlement, StateError> {
        self.store.get(execution_id).await
    }

    fn clone_ref(&self) -> Arc<dyn SettlementStore> {
        Arc::clone(&self.store)
    }
}

/// The write-ahead handle. Closed explicitly on the success path; on any other path — including a
/// panic inside the dispatch future — it is dropped unclosed, which is what makes the reopened
/// settlement `Ambiguous`.
pub struct SettlementGuard {
    service: Arc<dyn SettlementStore>,
    settlement: ExecutionSettlement,
    closed: bool,
}

impl SettlementGuard {
    /// Record what this run knows about one effect, and **write it**.
    ///
    /// Writing here rather than only at close is the whole of the write-ahead rule: a crash must find
    /// the effects the run had written, or the reopened settlement says "nothing was in flight" about
    /// a run that dispatched. An in-memory-only `record_effect` would make every crash look clean.
    ///
    /// # Errors
    /// The store's write error. A guard that cannot persist what it just did must fail loudly: the
    /// alternative is a run whose evidence is a memory that will not survive it.
    pub async fn record_effect(
        &mut self,
        record_id: impl Into<String>,
        state: SettlementState,
    ) -> Result<(), StateError> {
        self.settlement.record_effect(record_id, state);
        self.service.put(&self.settlement).await
    }

    /// The settlement as it stands.
    #[must_use]
    pub fn settlement(&self) -> &ExecutionSettlement {
        &self.settlement
    }

    /// Close as completed and write it.
    ///
    /// # Errors
    /// The store's write error.
    pub async fn close_completed(&mut self, finished_at: String) -> Result<(), StateError> {
        self.settlement.close_completed(finished_at);
        self.closed = true;
        self.service.put(&self.settlement).await
    }

    /// Close as aborted and write it.
    ///
    /// # Errors
    /// The store's write error.
    pub async fn close_aborted(
        &mut self,
        reason: impl Into<String>,
        finished_at: String,
    ) -> Result<(), StateError> {
        self.settlement.close_aborted(reason, finished_at);
        self.closed = true;
        self.service.put(&self.settlement).await
    }
}

impl Drop for SettlementGuard {
    fn drop(&mut self) {
        // Deliberately does nothing. A `Drop` cannot await, so it cannot write; and writing a
        // terminal here would be worse than not writing one — it would state an outcome for a run
        // that may have panicked mid-dispatch. Leaving the record without a terminal is the honest
        // signal: a reopened store reads it as `Ambiguous`, and `refuse_resume` refuses.
        let _ = self.closed;
    }
}

/// The persisted record's location is the state repository's, so this trait can be implemented over
/// any backend that already keeps durable records.
pub use crate::state_persistence::infrastructure::filesystem_settlement_store::FileSystemSettlementStore;
