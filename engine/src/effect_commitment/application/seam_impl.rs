//! EffectCommitmentSeam — the concrete dispatch seam.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#dispatchseam
//! Implements: ISSUE-EFFECT-COMMITMENT-16 — write-ahead + settle
//! Issue: #1129
//! ADR-019: a failed begin refuses the dispatch
//!
//! Wraps `EffectCommitmentService` in the `DispatchSeam` port used by the
//! execution engine. `before_dispatch` records the intent durably; a failure is
//! returned to the dispatch loop, which must refuse to send the effect.
//! `after_dispatch` settles an observed result or marks a lost/timeout outcome
//! ambiguous.

use std::sync::Arc;

use async_trait::async_trait;

use crate::effect_commitment::application::dto::{
    BeginEffectInput, MarkAmbiguousInput, SettleDefiniteInput,
};
use crate::effect_commitment::application::service::EffectCommitmentService;
use crate::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectRecord, EffectRecordId,
};

use super::seam::{DispatchObservation, DispatchSeam};

/// The default seam over an application service.
pub struct EffectCommitmentSeam {
    service: Arc<dyn EffectCommitmentService>,
}

impl EffectCommitmentSeam {
    /// Construct the seam over `service`.
    pub fn new(service: Arc<dyn EffectCommitmentService>) -> Self {
        Self { service }
    }

    /// The wrapped service.
    pub fn service(&self) -> &Arc<dyn EffectCommitmentService> {
        &self.service
    }
}

#[async_trait]
impl DispatchSeam for EffectCommitmentSeam {
    async fn before_dispatch(&self, intent: EffectIntent) -> Result<EffectRecord, CommitmentError> {
        let output = self.service.begin(BeginEffectInput { intent }).await?;
        Ok(output.record)
    }

    async fn after_dispatch(
        &self,
        record_id: EffectRecordId,
        observation: DispatchObservation,
    ) -> Result<EffectRecord, CommitmentError> {
        match observation {
            DispatchObservation::Observed(outcome) => {
                let output = self
                    .service
                    .settle_definite(SettleDefiniteInput { record_id, outcome })
                    .await?;
                Ok(output.record)
            }
            DispatchObservation::Lost { reason } => {
                let output = self
                    .service
                    .mark_ambiguous(MarkAmbiguousInput { record_id, reason })
                    .await?;
                Ok(output.record)
            }
        }
    }
}
