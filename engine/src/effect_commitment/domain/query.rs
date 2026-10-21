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
}
