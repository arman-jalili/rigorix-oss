//! `GateSetup` — the composition root for a native runtime.
//!
//! @canonical .pi/architecture/modules/gate.md
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Issue: #1381 (Phase C.1), consumed by the CLI in #1382
//!
//! # Why the construction lives here too
//!
//! Moving the *order* into the engine while leaving the *construction* in each caller
//! would only move half the problem: the CLI would still be the only place that knows
//! that a malformed `preconditions.toml` arms a fail-closed gate instead of none, and
//! the next runtime would have to rediscover it. Both halves belong to the engine,
//! and the CLI's job reduces to "ask for a `GateSetup`, hand it to the services".
//!
//! The loading semantics are the ones `PreconditionSetup` and `SequencePolicySetup`
//! already define, unchanged:
//!
//! - absent preconditions file → no dispatch-time gate (fail-open-absent, ADR-017)
//! - malformed preconditions file → an **unarmed** gate that refuses everything
//!   (fail closed: governance that cannot be established must not vanish)
//! - absent sequence policy → no obligations
//!
//! This module is native-only (`precondition::application::setup` is outside the
//! portable slice, which takes its ports from the host instead).

use std::path::Path;
use std::sync::Arc;

use crate::execution_engine::application::factory::{
    ParallelExecutionFactoryConfig, SequencePolicySetup,
};
use crate::orchestrator::application::builder::OrchestratorBuilder;
use crate::precondition::PreconditionSetup;
use crate::sequence_policy::application::service::SequencePolicyService;

use crate::gate::application::GatePorts;

/// The gate's ports plus the services a runtime wires around them.
///
/// `Clone` so it can travel into the factory config the way `PreconditionSetup` does.
#[derive(Clone)]
pub struct GateSetup {
    /// The dispatch-time precondition gate's service, when configured.
    ///
    /// Kept separately from [`Self::precondition_setup`] because the *service* is what
    /// [`crate::gate::application::evaluate_gate`] consults, while the setup is what
    /// the execution factory consumes (it also carries the gating mode).
    service: Option<Arc<dyn crate::precondition::application::service::PreconditionService>>,
    /// The armed/unarmed precondition setup, when configured.
    pub precondition: Option<PreconditionSetup>,
    /// The operator requirements service, when configured.
    pub sequence: Option<Arc<dyn SequencePolicyService>>,
}

impl GateSetup {
    /// Build the gate from the repository's operator configuration.
    ///
    /// This is the *whole* composition root: after it returns, a caller never has to
    /// know which file arms which gate, or what a malformed file means.
    #[must_use]
    pub fn from_env(repo_root: &Path) -> Self {
        let sequence = SequencePolicySetup::from_env(repo_root);
        let precondition = match PreconditionSetup::from_env(repo_root) {
            Ok(setup) => setup,
            Err(error) => {
                tracing::error!(
                    %error,
                    "precondition config invalid — arming fail-closed refusal gate (ADR-017)"
                );
                Some(PreconditionSetup::unarmed(repo_root, error.to_string()))
            }
        };
        // `PreconditionSetup` owns the *gate*; the service behind it is what the
        // composition consults. Rebuild the service from the same repository the
        // setup uses, so both read one file (ADR-017 forbids caching the answer).
        let service = precondition.as_ref().map(PreconditionSetup::service);
        Self {
            service,
            precondition,
            sequence,
        }
    }

    /// A setup with no configured gates (status quo: everything dispatches).
    #[must_use]
    pub fn none() -> Self {
        Self {
            service: None,
            precondition: None,
            sequence: None,
        }
    }

    /// The ports [`crate::gate::application::evaluate_gate`] consults.
    #[must_use]
    pub fn ports(&self) -> GatePorts {
        GatePorts {
            precondition: self.service.clone(),
            sequence: self.sequence.clone(),
        }
    }

    /// Whether any gate is configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ports().is_empty()
    }

    /// Attach the configured gates to an execution-factory config.
    ///
    /// Both fields are set here rather than by the caller, so a caller cannot wire one
    /// gate and forget the other — the defect this module exists to prevent.
    #[must_use]
    pub fn execution_config(
        &self,
        mut config: ParallelExecutionFactoryConfig,
    ) -> ParallelExecutionFactoryConfig {
        config.sequence_policy = self.sequence.clone();
        config.precondition = self.precondition.clone();
        config
    }

    /// Attach the plan-time sequence policy to an orchestrator builder, when present.
    #[must_use]
    pub fn orchestrator_builder<B: OrchestratorBuilder>(&self, builder: B) -> B {
        match &self.sequence {
            Some(policy) => builder.with_sequence_policy(Arc::clone(policy)),
            None => builder,
        }
    }
}

impl std::fmt::Debug for GateSetup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GateSetup")
            .field("precondition", &self.precondition.is_some())
            .field("sequence", &self.sequence.is_some())
            .finish()
    }
}
