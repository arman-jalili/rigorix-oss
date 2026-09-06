//! Infrastructure layer interfaces for the Sequence Policy bounded context.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#ddd-layers
//! Implements: Contract Freeze — SequencePolicyRepository +
//!   TomlSequencePolicyRepository
//! Issue: #838 (sequence-policy epic — contract freeze)
//!
//! Rule-config loading from `.rigorix/sequence-policy.toml` (filesystem),
//! authored by platform/security operators (R5). All persistence is hidden
//! behind `SequencePolicyRepository` — no caller touches the file format.
//!
//! # Contract (Frozen)
//! - `load_config` → `Ok(None)` when the file is absent (fail-open-absent);
//!   `Err(InvalidConfig | RuleExceedsCaps)` when it is corrupt (fail-closed)
//! - Degradation: config is read per-run from disk (same pattern as
//!   hooks/permissions); rules changed between runs apply to the next plan
//!   evaluation — prior decisions stand (documented in the module spec)

mod history;
pub mod repository;

pub use history::{EnvelopeHistoryAdapter, ExecutionHistory};
pub use repository::{SequencePolicyRepository, TomlSequencePolicyRepository};
