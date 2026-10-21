//! Application layer for Effect Commitment.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#ddd-layers
//! Implements: ISSUE-CONTRACT-FREEZE — service, key derivation, policy, DTOs, seams
//! Issue: #1113
//!
//! Defines the use cases over the [`EffectCommitmentLedger`] port, the
//! intent-derived key contract, the pure reconciliation policy, the input/output
//! DTOs, the dispatch/fence ports, the optional receipt-reader port, and the
//! composition-root contract. Concrete implementations live in later issues;
//! this layer is the frozen contract they must satisfy.
//!
//! [`EffectCommitmentLedger`]: crate::effect_commitment::interfaces::EffectCommitmentLedger

pub mod config;
pub mod dto;
pub mod factory;
pub mod fence;
pub mod key;
pub mod key_impl;
pub mod policy;
pub mod receipt;
pub mod seam;
pub mod seam_impl;
pub mod service;
pub mod service_impl;
pub mod setup;
pub mod vocabulary;

pub use config::{EffectCommitmentConfig, ProviderReceiptConfig};
pub use dto::{
    BeginEffectInput, BeginEffectOutput, MarkAmbiguousInput, MarkAmbiguousOutput, QueryEffectInput,
    QueryEffectOutput, ResolveEffectInput, ResolveEffectOutput, SettleDefiniteInput,
    SettleDefiniteOutput,
};
pub use factory::{EffectCommitmentBackendConfig, EffectCommitmentFactory};
pub use fence::{DependentFence, RetryFence, may_release_dependents, may_retry};
pub use key::IntentKeyDerivation;
pub use key_impl::Sha256IntentKeyDerivation;
pub use policy::ReconciliationPolicy;
pub use receipt::ProviderReceiptReader;
pub use seam::{DispatchObservation, DispatchSeam};
pub use seam_impl::EffectCommitmentSeam;
pub use service::EffectCommitmentService;
pub use service_impl::EffectCommitmentServiceImpl;
pub use setup::{
    EffectCommitmentCompositionRoot, EffectCommitmentSetup, EffectCommitmentSetupError,
};
pub use vocabulary::NodeStateVocabulary;
