//! EffectQuery — the read-side contract of the ledger.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#effectcommitmentledger
//! Implements: ISSUE-CONTRACT-FREEZE — query(record_id | dag_id)
//! Issue: #1113
//! ADR-019: operability requires an ambiguous-effects query
//!
//! # Contract (Frozen)
//! - `ById` reads one record
//! - `ByDag` reads every record for one execution
//! - `Ambiguous` lists unsettled effects, optionally narrowed to one DAG — this
//!   is the operator surface that makes an indefinitely blocked run legible
//! - `Dispatched` lists in-flight effects; the crash sweep uses it to find
//!   records whose owner is gone

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::EffectRecordId;

/// A read request against the commitment ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EffectQuery {
    /// Read one record by identity.
    ById {
        /// The record to read.
        record_id: EffectRecordId,
    },

    /// Read every record for one execution.
    ByDag {
        /// The execution whose records to read.
        dag_id: Uuid,
    },

    /// List unsettled (`Ambiguous`) effects, optionally narrowed to one run.
    Ambiguous {
        /// Optional execution filter.
        dag_id: Option<Uuid>,
    },

    /// List in-flight (`Dispatched`) effects, optionally narrowed to one run.
    ///
    /// This is the candidate set for the crash sweep: a record that is still
    /// `Dispatched` past its lease has no live owner and must become
    /// `Ambiguous`, never `NotCommitted` (#1164).
    Dispatched {
        /// Optional execution filter.
        dag_id: Option<Uuid>,
    },
}

impl EffectQuery {
    /// Read one record.
    pub fn by_id(record_id: EffectRecordId) -> Self {
        Self::ById { record_id }
    }

    /// Read every record for a run.
    pub fn by_dag(dag_id: Uuid) -> Self {
        Self::ByDag { dag_id }
    }

    /// List unsettled effects, optionally for one run.
    pub fn ambiguous(dag_id: Option<Uuid>) -> Self {
        Self::Ambiguous { dag_id }
    }

    /// List in-flight effects, optionally for one run.
    pub fn dispatched(dag_id: Option<Uuid>) -> Self {
        Self::Dispatched { dag_id }
    }
}
