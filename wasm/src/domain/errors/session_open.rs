//! `SessionOpenError` — the session could not be assembled.
//!
//! @canonical .pi/architecture/modules/portable-session.md#error-handling
//! Implements: Contract Freeze — SessionOpenError
//! Issue: #1294 (contract freeze)

use thiserror::Error;

use crate::domain::value::{PortFailure, ScenarioFormatVersion};

/// Why a session could not be opened.
///
/// # Contract (Frozen)
/// - A parse failure is a typed refusal, never a default and never a panic (criterion 1)
/// - `UnsupportedScenarioFormat` is produced before any evaluation, so an unknown
///   policy format is never evaluated against today's rules (FR-010)
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SessionOpenError {
    /// The scenario did not parse under the engine's own types.
    #[error("the scenario did not parse under the engine's types")]
    ScenarioUnparseable,

    /// The scenario declared a format version this build does not support.
    #[error("scenario format version {found} is not supported by this build")]
    UnsupportedScenarioFormat {
        /// The version the input declared.
        found: ScenarioFormatVersion,
    },

    /// The fixture transcript is missing something the seam requires.
    #[error("the fixture transcript is incomplete: {field}")]
    FixtureIncomplete {
        /// Which part is missing. A field name, never a value.
        field: &'static str,
    },

    /// The fixture transcript did not parse.
    #[error("the fixture transcript did not parse")]
    FixtureUnparseable,

    /// No clock readings were supplied.
    #[error("no clock readings were supplied")]
    ClockSeriesEmpty,

    /// A capability could not answer while the session was being pinned.
    #[error("capability {failure} could not answer while opening the session")]
    PortFailure {
        /// Which capability, and how it failed.
        failure: PortFailure,
    },
}
