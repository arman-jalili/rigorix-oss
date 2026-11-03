//! `ScenarioRef` — a pointer to an engine scenario, never a re-typed policy.
//!
//! @canonical .pi/architecture/modules/portable-session.md#value-objects-entity-vs-vo-justification
//! Implements: Contract Freeze — ScenarioRef, ScenarioFormatVersion
//! Issue: #1294 (contract freeze)
//!
//! The scenario is authored in the **engine's** template and sequence-policy format.
//! This crate references it by digest and format version so that (a) it cannot drift
//! into a second policy language and (b) an export can name exactly which scenario
//! produced it.

use serde::{Deserialize, Serialize};

use super::Digest;

/// Version of the engine scenario format this crate was built against.
///
/// # Contract (Frozen)
/// - Serialised as an integer; an unknown value is a refusal, never a guess
///   (criterion 12, FR-010)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ScenarioFormatVersion(pub u16);

impl std::fmt::Display for ScenarioFormatVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A digest-bearing reference to an engine scenario.
///
/// # Contract (Frozen)
/// - Equal by `digest` **and** `format_version`
/// - Carries no policy content: the crate never parses rules of its own
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioRef {
    /// Digest of the scenario as parsed by the engine.
    pub digest: Digest,
    /// Declared engine format version.
    pub format_version: ScenarioFormatVersion,
}

impl ScenarioRef {
    /// Build a reference from a digest and a declared format version.
    pub fn new(digest: Digest, format_version: ScenarioFormatVersion) -> Self {
        Self {
            digest,
            format_version,
        }
    }

    /// Identify a scenario supplied as JSON in the engine's own format.
    ///
    /// This does **not** parse the scenario — the engine owns that grammar and this crate
    /// never re-implements it. It does two things the record needs: it refuses input it
    /// cannot identify, and it computes the digest that binds the artifact to the policy
    /// that produced it.
    ///
    /// # Contract
    /// - A non-object is `ScenarioUnparseable`; a missing or non-integer
    ///   `format_version` is `ScenarioUnparseable` too, because defaulting it would mean
    ///   evaluating an unknown format against today's rules (FR-010, criterion 1)
    /// - The digest is over the canonical bytes of the supplied JSON, so the same
    ///   scenario always yields the same reference
    pub fn identify(scenario: &serde_json::Value) -> Result<Self, ScenarioRefError> {
        let object = scenario.as_object().ok_or(ScenarioRefError::NotAnObject)?;

        let declared = object
            .get("format_version")
            .and_then(serde_json::Value::as_u64)
            .ok_or(ScenarioRefError::NoFormatVersion)?;

        let format_version =
            u16::try_from(declared).map_err(|_| ScenarioRefError::NoFormatVersion)?;
        let canonical = serde_json::to_vec(scenario).map_err(|_| ScenarioRefError::NotAnObject)?;

        Ok(Self {
            digest: Digest::of(&canonical),
            format_version: ScenarioFormatVersion(format_version),
        })
    }
}

/// Why a scenario could not be identified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ScenarioRefError {
    /// The value was not a JSON object.
    #[error("the scenario is not a JSON object")]
    NotAnObject,
    /// The object carried no usable `format_version`.
    #[error("the scenario declares no usable format_version")]
    NoFormatVersion,
}
