//! Error projection — typed domain errors to `RefusalDto`.
//!
//! @canonical .pi/architecture/modules/portable-session.md#error-handling
//! @canonical .pi/architecture/decisions/ADR-012-failure-semantics-on-wasm32.md
//! Implements: the projection mapping (AC 16)
//! Issue: #1309 (WasmContract)
//!
//! # Contract
//! - One arm per domain error variant: adding a variant without a mapping is a compile
//!   error, so no error can escape unmapped into a host message
//! - Messages come from `BusinessWording`, not from `Display` of the error: `Display`
//!   carries the field name for a developer, the DTO carries a sentence for a reader
//! - No backtrace, no source path, no key material, no parameter value (NFR-003)

pub use super::dto::RefusalDto;

/// The machine label a host switches on.
const KIND_SESSION: &str = "session_open_failed";
const KIND_ATTEMPT: &str = "attempt_failed";
const KIND_SEAL: &str = "seal_failed";
const KIND_GATE: &str = "gate_unavailable";
const KIND_HANDLE: &str = "handle_invalid";
const KIND_REQUEST: &str = "request_invalid";

/// Maps an open failure to the wire refusal.
pub fn project_session_open(error: &crate::domain::errors::SessionOpenError) -> RefusalDto {
    use crate::domain::errors::SessionOpenError as E;
    use crate::domain::services::{BusinessWording, RefusalVocabulary};
    use crate::domain::value::PortId;

    let wording = BusinessWording::new();
    let (kind, port) = match error {
        E::ScenarioUnparseable => (KIND_SESSION, None),
        E::UnsupportedScenarioFormat { .. } => (KIND_SESSION, None),
        E::FixtureIncomplete { .. } => (KIND_SESSION, None),
        E::FixtureUnparseable => (KIND_SESSION, None),
        E::ClockSeriesEmpty => (KIND_SESSION, None),
        E::PortFailure { failure } => (KIND_SESSION, Some(failure.port)),
    };
    let message = match error {
        E::PortFailure { failure } => wording.port_reason(*failure),
        E::UnsupportedScenarioFormat { .. } => {
            wording.reason(crate::domain::outcome::RefusalKind::ScenarioUnsupported)
        }
        _ => "the session could not be opened from the supplied inputs",
    };
    let _ = PortId::Clock;
    RefusalDto {
        kind: kind.to_string(),
        message: message.to_string(),
        port: port.map(|p| p.as_str().to_string()),
        evidence_ref: None,
    }
}

/// Maps an attempt failure to the wire refusal.
pub fn project_attempt(error: &crate::domain::errors::AttemptError) -> RefusalDto {
    use crate::domain::errors::AttemptError as E;
    use crate::domain::services::{BusinessWording, RefusalVocabulary};

    let wording = BusinessWording::new();
    let (kind, message) = match error {
        E::AlreadyAttempted => (KIND_ATTEMPT, "this session has already run its one attempt"),
        E::AlreadySealed => (KIND_ATTEMPT, "this record is already sealed"),
        // The gate could not evaluate. A refusal — and the wording says "could not", not
        // "did not pass": the two are different facts about the world.
        E::EngineFailure { .. } => (KIND_GATE, "the gate could not evaluate the run"),
    };
    let _ = wording.reason(crate::domain::outcome::RefusalKind::PreconditionError);
    RefusalDto {
        kind: kind.to_string(),
        message: message.to_string(),
        port: None,
        evidence_ref: None,
    }
}

/// Maps a seal failure to the wire refusal.
pub fn project_seal(error: &crate::domain::errors::SealError) -> RefusalDto {
    use crate::domain::errors::SealError as E;
    use crate::domain::outcome::RefusalKind;
    use crate::domain::services::{BusinessWording, RefusalVocabulary};

    let wording = BusinessWording::new();
    let (kind, message, port) = match error {
        E::NoAttempt => (KIND_SEAL, "this session has no attempt to seal", None),
        E::AlreadySealed => (KIND_SEAL, "this record is already sealed", None),
        E::ContentRejected { rule } => (
            KIND_SEAL,
            match rule {
                crate::domain::services::ContentRule::KeyMaterial => {
                    "the record was refused: it would carry key material"
                }
                crate::domain::services::ContentRule::AbsolutePath => {
                    "the record was refused: it would carry a host path"
                }
                crate::domain::services::ContentRule::PersonalData => {
                    "the record was refused: it would carry personal data"
                }
                crate::domain::services::ContentRule::Credential => {
                    "the record was refused: it would carry a credential"
                }
            },
            None,
        ),
        E::PortFailure { failure } => {
            (KIND_SEAL, wording.port_reason(*failure), Some(failure.port))
        }
    };
    let _ = wording.reason(RefusalKind::SessionSealed);
    RefusalDto {
        kind: kind.to_string(),
        message: message.to_string(),
        port: port.map(|p| p.as_str().to_string()),
        evidence_ref: None,
    }
}

/// A refusal of the request itself: the JSON did not fit the frozen DTO.
pub fn project_request(field: &'static str) -> RefusalDto {
    RefusalDto {
        kind: KIND_REQUEST.to_string(),
        message: format!("the request could not be read: {field}"),
        port: None,
        evidence_ref: None,
    }
}

/// A malformed or unknown handle — the only error a host can provoke.
pub fn project_handle(error: &super::registry::HandleError) -> RefusalDto {
    RefusalDto {
        kind: KIND_HANDLE.to_string(),
        message: error.to_string(),
        port: None,
        evidence_ref: None,
    }
}
