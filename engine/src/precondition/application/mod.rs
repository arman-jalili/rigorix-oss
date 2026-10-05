//! Application layer interfaces for the Consequence Gating bounded context.
//!
//! @canonical .pi/architecture/modules/consequence-gating.md#ddd-layers
//! Implements: Contract Freeze — PreconditionService, DispatchGate,
//!   CompanionStepObligation, PreconditionSurfaces, HardeningConfig,
//!   PreconditionFactory, DispatchStep / PreconditionCheckInput DTOs
//! Issue: #938 (consequence-gating epic — contract freeze)
//!
//! This module defines:
//! - The `PreconditionService` use-case trait (match → run → Dispatch/Deny)
//! - The `DispatchGate` choke-point trait + fail-closed arming
//! - The R3 `CompanionStepObligation` contract
//! - The `PreconditionSurfaces` error taxonomy (`policy_violation`)
//! - `HardeningConfig` (`max_failures_before_abort`)
//! - The `PreconditionFactory` construction interface
//! - The boundary DTOs (`DispatchStep`, `PreconditionCheckInput`)
//!
//! # Contract (Frozen)
//! - All service methods are async and trait-object safe (`Send + Sync`,
//!   via `async-trait`)
//! - All public methods return domain types (`PreconditionVerdict`) or
//!   `PreconditionError`
//! - Evaluation is fail-closed: an error refuses the step — never a silent
//!   pass-through to dispatch
//! - No implementation logic — only contract signatures (behavior lands in
//!   ISSUE-CONSEQUENCE-GATING-4/-5/-8/-9/-10/-11)

pub mod companion;
pub mod companion_impl;
pub mod dto;
pub mod factory;
pub mod gate;
pub mod gate_impl;
pub mod hardening;
pub mod service;
pub mod service_impl;
pub mod surfaces;

pub use companion::{
    CompanionAction, CompanionFinding, CompanionStepObligation, CompanionStepObligationService,
};
pub use companion_impl::CompanionStepObligationServiceImpl;
pub use dto::{DispatchStep, PreconditionCheckInput};
pub use factory::PreconditionFactory;
pub use gate::DispatchGate;
pub use gate_impl::PreconditionDispatchGate;
pub use hardening::HardeningConfig;
pub use service::PreconditionService;
pub use service_impl::PreconditionServiceImpl;
pub use surfaces::{PreconditionPolicyViolation, PreconditionSurfaces};
