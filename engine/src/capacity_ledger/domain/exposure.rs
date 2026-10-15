//! Exposure domain type.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#exposure
//! Implements: ISSUE-CONTRACT-FREEZE — Exposure aggregate view
//! Issue: #1021
//!
//! The aggregate view. **Committed and outstanding are separate fields, never
//! summed into one number**: "what is actually spent" and "what is in flight"
//! are different operational questions, and collapsing them destroys the ability
//! to answer either.
//!
//! # Contract (Frozen)
//! - `committed` and `outstanding` stay distinct on the wire and in memory
//! - `available = hard_limit - committed - outstanding` (saturating at zero)

use serde::{Deserialize, Serialize};

use super::{ResourceId, Window};

/// Point-in-time exposure of a resource.
///
/// # Fields (Frozen)
/// - `resource: ResourceId` — the measured resource
/// - `hard_limit: u64` — the configured cap
/// - `committed: u64` — capacity actually spent
/// - `outstanding: u64` — capacity held but not yet resolved
/// - `window: Window` — aggregation period this snapshot covers
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exposure {
    /// The measured resource.
    pub resource: ResourceId,

    /// The configured cap.
    pub hard_limit: u64,

    /// Capacity actually spent.
    pub committed: u64,

    /// Capacity held but not yet resolved.
    pub outstanding: u64,

    /// Aggregation period this snapshot covers.
    pub window: Window,
}

impl Exposure {
    /// Construct an exposure snapshot.
    pub fn new(
        resource: ResourceId,
        hard_limit: u64,
        committed: u64,
        outstanding: u64,
        window: Window,
    ) -> Self {
        Self {
            resource,
            hard_limit,
            committed,
            outstanding,
            window,
        }
    }

    /// Remaining capacity that may still be reserved.
    pub fn available(&self) -> u64 {
        self.hard_limit
            .saturating_sub(self.committed)
            .saturating_sub(self.outstanding)
    }
}
