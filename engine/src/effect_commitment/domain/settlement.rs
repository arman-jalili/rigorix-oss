//! The execution settlement: one record per run, and the ambiguity that survives a restart.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#the-aggregate
//! Implements: ADR-019 (write-ahead, three-valued outcomes), ADR-024 §7
//! Issue: #1528
//!
//! `effect-commitment` answers "was this effect committed?" per dispatch. It did not answer "how did
//! this execution end?", so a crashed run left a set of settled records and no statement about the
//! run they belong to — and a second run for the same receipt could start over an effect whose fate
//! is unknown.
//!
//! # Why an aggregate and not a field
//!
//! `EffectRecord` is keyed by an intent-derived key; adding a "this run" field to it would make one
//! record speak for many runs, and the second run's write would overwrite the first's evidence. The
//! settlement is the aggregate that *references* record ids, so the records stay the per-effect
//! truth and the settlement is the per-run statement.
//!
//! # Write-ahead
//!
//! The settlement is opened **before** the first `begin`, so a crash has something to reopen into.
//! That is ADR-019's rule, and it is why the state that a reopened store finds is `Ambiguous`
//! rather than "missing": a record with no terminal state is not an error to retry, it is a
//! statement that this run stopped somewhere the engine cannot see. Resolving it needs evidence
//! from outside — the engine records the ambiguity and refuses to paper over it.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The state of one effect within a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementState {
    /// The dispatch observed success and the effect is committed.
    Committed,
    /// The dispatch observed a definite failure.
    NotCommitted,
    /// The dispatch was sent and its outcome is not known to this run.
    Dispatched,
    /// The run ended while this effect's outcome was unknown.
    Ambiguous,
}

/// How the run ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SettlementTerminal {
    /// Every effect reached a definite state.
    Completed,
    /// The run stopped for a stated reason, with the effects still definite.
    Aborted {
        /// Why.
        reason: String,
    },
    /// The run stopped with at least one effect whose outcome this run cannot state.
    Ambiguous,
}

/// One effect, as the settlement remembers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementEffect {
    /// The effect record this refers to. The record stays the per-effect truth.
    pub record_id: String,
    /// What this run knows about it.
    pub state: SettlementState,
}

/// One execution, from open to close — including the runs that never closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSettlement {
    /// The execution.
    pub execution_id: Uuid,
    /// The receipt it answers, when the run was receipt-driven (#1527).
    pub receipt_digest: Option<String>,
    /// When it opened.
    pub started_at: String,
    /// When it closed. `None` means it has not: either it is running, or it stopped where nothing
    /// could write this field — which is the state a reopened store reads as `Ambiguous`.
    pub finished_at: Option<String>,
    /// The effects it wrote.
    pub effects: Vec<SettlementEffect>,
    /// How it ended. `None` while open.
    pub terminal: Option<SettlementTerminal>,
}

/// Why a resume was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmbiguousSettlement {
    /// The execution whose settlement is ambiguous.
    pub execution_id: Uuid,
    /// The receipt digest, when there is one.
    pub receipt_digest: Option<String>,
    /// The effect record ids this run cannot state the fate of. The refusal names them, because a
    /// refusal that says "some effect is ambiguous" leaves the operator to guess which.
    pub record_ids: Vec<String>,
}

impl std::fmt::Display for AmbiguousSettlement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "execution {} cannot resume: its settlement is ambiguous for effect record(s) {}",
            self.execution_id,
            self.record_ids.join(", ")
        )
    }
}

impl std::error::Error for AmbiguousSettlement {}

impl ExecutionSettlement {
    /// Open a settlement. Called before the first `begin` (ADR-019's write-ahead rule).
    #[must_use]
    pub fn open(execution_id: Uuid, receipt_digest: Option<String>, started_at: String) -> Self {
        Self {
            execution_id,
            receipt_digest,
            started_at,
            finished_at: None,
            effects: Vec::new(),
            terminal: None,
        }
    }

    /// Record an effect with what this run knows about it.
    pub fn record_effect(&mut self, record_id: impl Into<String>, state: SettlementState) {
        let record_id = record_id.into();
        match self.effects.iter_mut().find(|e| e.record_id == record_id) {
            Some(existing) => existing.state = state,
            None => self.effects.push(SettlementEffect { record_id, state }),
        }
    }

    /// Close the run as completed, when every effect is definite.
    ///
    /// A run whose effects are not all definite cannot be closed as `Completed`: the settlement
    /// computes its own terminal when this one would be a lie.
    pub fn close_completed(&mut self, finished_at: String) {
        self.finished_at = Some(finished_at);
        self.terminal = Some(if self.has_undefinite_fate() {
            SettlementTerminal::Ambiguous
        } else {
            SettlementTerminal::Completed
        });
    }

    /// Close the run with a stated reason.
    pub fn close_aborted(&mut self, reason: impl Into<String>, finished_at: String) {
        self.finished_at = Some(finished_at);
        self.terminal = Some(if self.has_undefinite_fate() {
            SettlementTerminal::Ambiguous
        } else {
            SettlementTerminal::Aborted {
                reason: reason.into(),
            }
        });
    }

    /// Whether any effect's fate this run cannot state.
    #[must_use]
    pub fn has_undefinite_fate(&self) -> bool {
        self.effects.iter().any(|e| {
            matches!(
                e.state,
                SettlementState::Dispatched | SettlementState::Ambiguous
            )
        })
    }

    /// The effects this run cannot state the fate of — the orphans a reopened store reports.
    #[must_use]
    pub fn orphaned(&self) -> Vec<String> {
        self.effects
            .iter()
            .filter(|e| {
                matches!(
                    e.state,
                    SettlementState::Dispatched | SettlementState::Ambiguous
                )
            })
            .map(|e| e.record_id.clone())
            .collect()
    }

    /// The terminal state as a reopened store reads it.
    ///
    /// A settlement with no terminal is **not** "still running": a running execution holds its
    /// process, and the store is what a *second* process reads. No terminal there means the writer
    /// never came back, so the terminal is `Ambiguous` — including when nothing was ever written to
    /// a record, because a run that died before its first `begin` also cannot be said to have done
    /// nothing.
    #[must_use]
    pub fn terminal_as_read(&self) -> SettlementTerminal {
        match &self.terminal {
            Some(terminal) => terminal.clone(),
            None => SettlementTerminal::Ambiguous,
        }
    }

    /// The refusal to resume, when this settlement says the fate of an effect is unknown.
    #[must_use]
    pub fn refuse_resume(&self) -> Option<AmbiguousSettlement> {
        if !matches!(self.terminal_as_read(), SettlementTerminal::Ambiguous) {
            return None;
        }
        let record_ids = self.orphaned();
        if record_ids.is_empty() {
            return None;
        }
        Some(AmbiguousSettlement {
            execution_id: self.execution_id,
            receipt_digest: self.receipt_digest.clone(),
            record_ids,
        })
    }
}

/// Refuse a resume over any ambiguous settlement of this receipt (ADR-019's claim ceiling: the
/// resolution needs evidence from outside, so the engine refuses rather than retries).
///
/// # Errors
/// `AmbiguousSettlement`, naming the ambiguous record ids.
pub fn refuse_resume_on_ambiguity(
    settlements: &[ExecutionSettlement],
    receipt_digest: &str,
) -> Result<(), AmbiguousSettlement> {
    for settlement in settlements {
        if settlement.receipt_digest.as_deref() != Some(receipt_digest) {
            continue;
        }
        if let Some(refusal) = settlement.refuse_resume() {
            return Err(refusal);
        }
    }
    Ok(())
}
