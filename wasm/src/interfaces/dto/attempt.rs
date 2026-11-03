//! `SessionHandleDto` and `GatedActionDto`.
//!
//! @canonical .pi/architecture/decisions/ADR-004-api-design.md
//! Implements: Contract Freeze — SessionHandleDto, GatedActionDto
//! Issue: #1294 (contract freeze)

use serde::{Deserialize, Serialize};

/// The opaque handle a session is addressed by.
///
/// # Contract (Frozen)
/// - Opaque: the host must not construct or inspect it beyond passing it back
/// - A malformed handle is the **only** error a host can provoke, and it is a
///   programmer error, not a gate decision (criterion 16)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionHandleDto {
    /// Opaque session token.
    pub handle: String,
}

/// The action a host proposes.
///
/// # Contract (Frozen)
/// - `name` is matched by the engine's predicate; `params` is an object
/// - No field here can request an allow, an override or an approval (FR-009)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatedActionDto {
    /// Tool/step name.
    pub name: String,
    /// Parameters the policy may inspect.
    pub params: serde_json::Map<String, serde_json::Value>,
}
