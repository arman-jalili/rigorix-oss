//! Application layer interfaces for the Sequence Policy bounded context.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#ddd-layers
//! Implements: Contract Freeze — SequencePolicyService trait,
//!   SequencePolicyServiceImpl stub, SequencePolicyFactory trait,
//!   PlannedStep / DispatchedStep DTOs
//! Issue: #838 (sequence-policy epic — contract freeze)
//!
//! This module defines:
//! - The `SequencePolicyService` use-case trait (R2 plan-time evaluation,
//!   R3 run-time prefix gate)
//! - The `SequencePolicyServiceImpl` stub bound to a
//!   `SequencePolicyRepository`
//! - The `SequencePolicyFactory` interface for service construction
//! - The boundary DTOs (`PlannedStep`, `DispatchedStep`)
//!
//! # Contract (Frozen)
//! - All service methods are async and trait-object safe (`Send + Sync`,
//!   via `async-trait`)
//! - All public methods return domain types (`SequenceMatch`) or
//!   `SequencePolicyError`
//! - Evaluation is fail-closed: an error refuses the plan / halts the node —
//!   never a silent pass-through to dispatch
//! - No implementation logic — only contract signatures (behavior lands in
//!   ISSUE-SEQUENCE-POLICY-2 and ISSUE-SEQUENCE-POLICY-5)

pub mod dto;
pub mod factory;
mod matcher;
pub mod service;
pub mod service_impl;

pub use dto::{DispatchedStep, PlannedStep};
pub use factory::SequencePolicyFactory;
pub use service::SequencePolicyService;
pub use service_impl::SequencePolicyServiceImpl;
