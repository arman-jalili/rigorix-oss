//! PreconditionDispatchGate — the concrete dispatch choke-point gate.
//!
//! @canonical .pi/architecture/modules/precondition.md#dispatch-integration
//! Implements: ISSUE-CONSEQUENCE-GATING-5 — gate + fail-closed arming
//! Issue: #943; contract frozen in #938
//!
//! Wraps a [`PreconditionService`] and adds the fail-closed arming state: when
//! preconditions are configured but the gate could not arm, a matching step is
//! refused with [`PreconditionError::NotArmed`] rather than silently
//! dispatching. An armed gate delegates to the service (`Dispatch` when
//! nothing matches, `Deny` on a refusal).
//!
//! The execution engine holds an `Arc<dyn DispatchGate>` and calls `assess`
//! after ADR-011 approval verification and before `spawn_concurrent_node`; a
//! `Deny` marks the node failed and the tool is never called.

use async_trait::async_trait;

use crate::precondition::domain::{PreconditionError, PreconditionFinding, PreconditionVerdict};

use super::dto::DispatchStep;
use super::gate::DispatchGate;
use super::service::PreconditionService;

/// Concrete dispatch gate over a precondition service.
pub struct PreconditionDispatchGate {
    service: Box<dyn PreconditionService>,
    /// Whether the gate armed successfully. `false` = configured-but-unarmed
    /// (fail closed).
    armed: bool,
    /// Why the gate is unarmed (operator-facing detail).
    unarmed_detail: Option<String>,
}

impl PreconditionDispatchGate {
    /// Build an armed gate over the service.
    ///
    /// "Armed" means the operator config loaded and every matching
    /// precondition can be evaluated; the composition root only constructs an
    /// armed gate when it can vouch for that. Use [`Self::unarmed`] to build
    /// the fail-closed refusal gate when arming failed.
    pub fn new(service: Box<dyn PreconditionService>) -> Self {
        Self {
            service,
            armed: true,
            unarmed_detail: None,
        }
    }

    /// Build an unarmed gate that refuses every assessed step (fail closed).
    ///
    /// Used by the composition root when preconditions are configured but
    /// cannot arm (malformed config, missing executable). Refusing is
    /// deliberate: configured-but-unarmed is never a silent downgrade.
    pub fn unarmed(service: Box<dyn PreconditionService>, detail: impl Into<String>) -> Self {
        Self {
            service,
            armed: false,
            unarmed_detail: Some(detail.into()),
        }
    }

    /// Whether this gate is armed (synchronous view).
    pub fn is_armed_sync(&self) -> bool {
        self.armed
    }
}

#[async_trait]
impl DispatchGate for PreconditionDispatchGate {
    async fn assess(
        &self,
        execution_id: uuid::Uuid,
        step: &DispatchStep,
    ) -> Result<PreconditionVerdict, PreconditionError> {
        if !self.armed {
            return Err(PreconditionError::NotArmed {
                precondition_id: "<gate>".to_string(),
                detail: self
                    .unarmed_detail
                    .clone()
                    .unwrap_or_else(|| "configured but not armed".to_string()),
            });
        }
        self.service.evaluate(execution_id, step).await
    }

    async fn is_armed(&self) -> Result<bool, PreconditionError> {
        if !self.armed {
            return Ok(false);
        }
        self.service.is_configured().await
    }

    async fn assess_with_findings(
        &self,
        execution_id: uuid::Uuid,
        step: &DispatchStep,
    ) -> Result<(PreconditionVerdict, Vec<PreconditionFinding>), PreconditionError> {
        if !self.armed {
            return Err(PreconditionError::NotArmed {
                precondition_id: "<gate>".to_string(),
                detail: self
                    .unarmed_detail
                    .clone()
                    .unwrap_or_else(|| "configured but not armed".to_string()),
            });
        }
        self.service
            .evaluate_with_findings(execution_id, step)
            .await
    }
}
