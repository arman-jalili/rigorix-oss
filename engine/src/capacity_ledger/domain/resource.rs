//! Resource domain type.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#resource
//! Implements: ISSUE-CONTRACT-FREEZE — Resource (`ResourceId` + `ResourceSpec`)
//! Issue: #1021
//!
//! A named, limited capacity. `ResourceId` is the identity; `ResourceSpec`
//! carries `hard_limit` and the window over which exposure is aggregated. The
//! engine attaches no domain meaning to a resource name — it is an
//! operator-declared label.
//!
//! # Contract (Frozen)
//! - `ResourceId` is an opaque, stable identity (comparable + hashable)
//! - `ResourceSpec` is immutable configuration; `hard_limit` is a hard cap
//! - `Window` bounds the period over which exposure is aggregated

use std::fmt;

use serde::{Deserialize, Serialize};

/// Identity of an operator-declared capacity resource.
///
/// The name is intentionally opaque to the engine: `"gpu"`, `"api_calls"`, and
/// `"db_connections"` are all just labels chosen by the operator.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceId(String);

impl ResourceId {
    /// Construct a resource identity from an operator-declared label.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrow the underlying label.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The aggregation window over which exposure is measured.
///
/// A window of `duration_secs == 0` denotes an unbounded (all-time) window,
/// which is the safe default for callers that do not specify one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Window {
    /// Window length in whole seconds. `0` means unbounded.
    pub duration_secs: u64,
}

impl Window {
    /// An unbounded (all-time) aggregation window.
    pub const UNBOUNDED: Self = Self { duration_secs: 0 };

    /// Construct a bounded window.
    pub fn bounded(duration_secs: u64) -> Self {
        Self { duration_secs }
    }
}

/// Static declaration of a named, limited capacity.
///
/// # Fields (Frozen)
/// - `id: ResourceId` — stable identity
/// - `name: String` — human-readable operator label
/// - `hard_limit: u64` — maximum capacity that may ever be outstanding+committed
/// - `window: Window` — exposure aggregation period
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceSpec {
    /// Stable identity of the resource.
    pub id: ResourceId,

    /// Human-readable operator label.
    pub name: String,

    /// Maximum capacity that may be claimed at once.
    pub hard_limit: u64,

    /// Period over which exposure is aggregated.
    pub window: Window,
}

impl ResourceSpec {
    /// Construct a resource declaration.
    pub fn new(id: ResourceId, name: String, hard_limit: u64, window: Window) -> Self {
        Self {
            id,
            name,
            hard_limit,
            window,
        }
    }

    /// The resource identity.
    pub fn id(&self) -> &ResourceId {
        &self.id
    }

    /// The hard capacity cap.
    pub fn hard_limit(&self) -> u64 {
        self.hard_limit
    }
}
