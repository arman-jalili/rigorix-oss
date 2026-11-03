//! The four exported functions — the frozen host contract.
//!
//! @canonical .pi/architecture/decisions/ADR-004-api-design.md
//! @canonical .pi/architecture/decisions/ADR-013-who-verifies.md
//! @canonical .pi/architecture/decisions/ADR-012-failure-semantics-on-wasm32.md
//! Implements: Contract Freeze — the four exports, ContractVersion
//! Issue: #1294 (contract freeze)
//!
//! ```text
//! open_session(scenario, fixtures, clock, seed) -> handle
//! attempt_run(handle, action)                   -> run_outcome
//! export_artifact_set(handle)                   -> artifact_set
//! contract_version()                            -> { format, engine }
//! ```
//!
//! # Contract (Frozen)
//! - JSON strings in, JSON strings out. The `#[wasm_bindgen]` attribute layer is applied
//!   by the interface implementation issue; the **signatures** are frozen here
//! - Every DTO rejects unknown fields (`deny_unknown_fields`), so a typo is a refusal,
//!   not a silently ignored field (criterion 15)
//! - A refusal is `Ok(refusal_json)` — an outcome, not an error. Only a malformed
//!   handle is a `JsValue` error, because that is a programmer mistake in the host
//!   (criterion 16)
//! - No panic may cross this line (ADR-012 tier 2/3): a panic fails the build, a trap is
//!   the host's to surface as *fatal*, never as an allow

use serde::{Deserialize, Serialize};

/// The frozen list of exported function names.
///
/// # Contract (Frozen)
/// - Exactly four. A fifth requires an ADR: the fifth was verification, and ADR-013
///   removed it deliberately
pub const CONTRACT_FUNCTIONS: [&str; 4] = [
    "open_session",
    "attempt_run",
    "export_artifact_set",
    "contract_version",
];

/// The version triple a host checks before it trusts the shape of the JSON.
///
/// # Contract (Frozen)
/// - Three numbers, deliberately distinct: package version (release train),
///   contract version (this surface), artifact format version (the bytes)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractVersion {
    /// Semver of this host contract.
    pub contract: String,
    /// Engine version the bundle was built against.
    pub engine: String,
    /// Artifact container format this bundle emits.
    pub artifact_format: u32,
}

/// Assemble a replayable session.
///
/// TODO(#1300): parse DTOs, call `OpenReplaySession`, render a handle.
///
/// # Contract (Frozen)
/// - Four JSON inputs, all required: scenario, fixtures, clock, seed
/// - Returns a JSON handle (opaque to the host) or a JSON refusal
pub fn open_session(
    scenario_json: String,
    fixtures_json: String,
    clock_json: String,
    seed_json: String,
) -> Result<String, String> {
    let _ = (scenario_json, fixtures_json, clock_json, seed_json);
    todo!("contract freeze: open_session lands with #1300")
}

/// Drive the session's one attempt.
///
/// TODO(#1300): parse handle + action, call `AttemptRun`, render the outcome.
///
/// # Contract (Frozen)
/// - Returns `{ status, steps[], evidence[] }`; `steps` preserves engine order
/// - A refusal is a normal return value
pub fn attempt_run(handle_json: String, action_json: String) -> Result<String, String> {
    let _ = (handle_json, action_json);
    todo!("contract freeze: attempt_run lands with #1300")
}

/// Seal and export the artifact set.
///
/// TODO(#1300): call `ExportArtifactSet`, render the sealed bytes + stamp.
///
/// # Contract (Frozen)
/// - Returns the stamped container; the host verifies it with `rigorix-verifier`
/// - A content-rule violation is a refusal, and the bytes do not leave
pub fn export_artifact_set(handle_json: String) -> Result<String, String> {
    let _ = handle_json;
    todo!("contract freeze: export_artifact_set lands with #1300")
}

/// Report the three version numbers.
pub fn contract_version() -> String {
    let _version = ContractVersion {
        contract: env!("CARGO_PKG_VERSION").to_string(),
        engine: env!("CARGO_PKG_VERSION").to_string(),
        artifact_format: crate::domain::artifact::ArtifactSet::FORMAT_VERSION,
    };
    todo!("contract freeze: contract_version lands with #1300")
}
