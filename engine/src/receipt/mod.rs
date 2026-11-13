//! The execution receipt: what a host declares, and what the engine can prove it observed.
//!
//! @canonical .pi/architecture/modules/receipt.md
//! Implements: ADR-024 §4 (D3), §5 (D4), §6 (E4), §8, §10
//! Issue: #1527
//!
//! A host that submits an execution declares what it expects to happen. Before this module the
//! engine had nothing to compare that declaration against and nothing it could sign: the only
//! request-side artifact was a step list with no effects and no expectations, and the only
//! response-side artifact was an iteration report carrying counts. "The execution did what was
//! asked" was a claim made by the side that asked.
//!
//! # Two types, two directions of trust
//!
//! - [`domain::receipt::Receipt`] is the **declaration**: digest-pinned, verifiable as a whole
//!   before anything runs. A host can build one.
//! - [`domain::attestation::Attestation`] is the **observation**, signed. Only the engine's builder
//!   can construct one, because every field is read from something this execution did — and a value
//!   the engine did not observe is recorded as the caller's claim (`QualitySource::Caller`), never
//!   as a pass.
//!
//! The asymmetry is the point. A receipt that lies is refused at
//! [`application::service::verify`]; an attestation that lies cannot be produced, and one that is
//! altered afterwards fails [`domain::attestation::verify_attestation`].

pub mod application;
pub mod domain;
pub mod infrastructure;

pub use application::service::{
    AttestationBuilder, VerifiedReceipt, compare_declared_observed, default_state_dir,
    refuse_state_dir_inside_workspace, sign, verify,
};
pub use domain::attestation::{Attestation, QualityRecord, QualitySource, verify_attestation};
pub use domain::digest::{ReceiptDigest, canonical_json, sha256_hex};
pub use domain::receipt::{
    BudgetDeclared, DeclaredEffect, Expectation, Receipt, ReceiptStep, Role, Sandbox,
    SandboxRegime, Workspace, WorkspaceKind,
};
pub use domain::refusal::{ReceiptRefusal, ReceiptVerdict};
