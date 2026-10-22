//! Effect Commitment bounded context.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md
//! Implements: ISSUE-CONTRACT-FREEZE — effect-commitment module root
//! Issue: #1113
//! ADR-019: effect commitment — owning the unsettled interval
//!
//! Owns the **unsettled interval** between dispatching an effect and learning
//! whether it committed: the write-ahead intent record, the four-state machine
//! with a non-terminal `Ambiguous`, the resolver authority, and the fences that
//! stop an unsettled effect from being retried or closed.
//!
//! It explicitly does **not** own dispatch (`execution_engine`), reservations
//! (`capacity_ledger`), or evidence storage (`audit`). It answers one question
//! — *was this effect committed?* — and its authority for that answer is
//! **external**: the provider's receipt or read-back. See ADR-019.
//!
//! # DDD Layers
//!
//! ```text
//! effect_commitment/
//! ├── domain/           # Intent, state machine, outcome, resolver (zero framework imports)
//! ├── application/      # Use cases, key derivation, reconciliation policy, DTOs
//! ├── infrastructure/   # In-memory + HTTP adapters, provider receipt reader, factory
//! └── interfaces/       # The `EffectCommitmentLedger` port + HTTP endpoint contracts
//! ```
//!
//! Dependency rule: `domain ← application ← infrastructure` (inward);
//! `interfaces/` is the contract consumed by `execution_engine`.
//!
//! # Guarantee ceiling
//!
//! The record and the fences are provable **in-process (L1)**. Cross-process
//! settlement needs a contested store **(L2)**. Any claim that an effect
//! *committed* needs provider evidence **(L3)**; without a receipt reader this
//! context can only ever prove **not-settled**. That asymmetry is the honest
//! claim and must appear wherever the feature is described.
//!
//! # Contract Freeze Notice
//!
//! ALL files in this module are frozen contracts created by
//! ISSUE-CONTRACT-FREEZE. The domain model and the reconciliation policy are
//! complete; the adapters and composition seams are typed stubs that fail
//! closed. No implementation change may alter these signatures without an
//! explicit contract change.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;

pub use application::{
    BeginEffectInput, BeginEffectOutput, DependentFence, DispatchObservation, DispatchSeam,
    EffectCommitmentBackendConfig, EffectCommitmentCeiling, EffectCommitmentCompositionRoot,
    EffectCommitmentConfig, EffectCommitmentFactory, EffectCommitmentSeam, EffectCommitmentService,
    EffectCommitmentServiceImpl, EffectCommitmentSetup, EffectCommitmentSetupError,
    IntentKeyDerivation, MarkAmbiguousInput, MarkAmbiguousOutput, NodeStateVocabulary,
    ProviderReceiptConfig, ProviderReceiptReader, QueryEffectInput, QueryEffectOutput,
    ReconciliationPolicy, ResolveEffectInput, ResolveEffectOutput, RetryFence, SettleDefiniteInput,
    SettleDefiniteOutput, Sha256IntentKeyDerivation,
};
pub use domain::{
    CommitmentError, EffectEvent, EffectIntent, EffectQuery, EffectRecord, EffectRecordId,
    EffectState, ObservedOutcome, OutcomeVerdict, ResolverIdentity, ResolverKind,
};
pub use infrastructure::{
    DefaultEffectCommitmentFactory, HttpEffectLedger, HttpEffectLedgerConfig,
    HttpProviderReceiptReader, InMemoryEffectLedger,
};
pub use interfaces::EffectCommitmentLedger;
