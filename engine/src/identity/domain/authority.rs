//! Authority — optional structured role / policy identifier (captured fact).
//!
//! @canonical .pi/architecture/modules/identity.md#authority
//! Implements: Contract Freeze — Authority value object
//! Issue: #700 (identity epic — contract freeze)
//!
//! `Authority` is a **captured fact, not a judgment**: it records the role or
//! policy identifier that was *presented* with the identity claim. OSS never
//! evaluates whether that role may act — Enterprise consumes this field for
//! authorization (scope/RBAC enforcement).
//!
//! The `IdentityClaim` value object carries the simplified
//! `authority: Option<String>` form (see `.pi/architecture/modules/identity.md#identityclaim`);
//! this structured form is used by Enterprise/approval binding when a
//! role→policy mapping is needed.
//!
//! # Contract (Frozen)
//! - Immutable value object after construction
//! - All fields optional — an absent authority is `None`, never a sentinel string
//! - Serialization support for claims forwarded to Enterprise

use serde::{Deserialize, Serialize};

/// Structured role / policy identifier attached to an identity claim.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Authority {
    /// Role presented (captured fact, not judgment).
    pub role: Option<String>,
    /// Policy identifier the role maps to (resolved Enterprise-side).
    pub policy_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authority_default_is_absent() {
        let authority = Authority::default();
        assert_eq!(authority.role, None);
        assert_eq!(authority.policy_id, None);
    }

    #[test]
    fn authority_serde_round_trip() {
        let authority = Authority {
            role: Some("admin".to_string()),
            policy_id: Some("policy:production-switch".to_string()),
        };
        let json = serde_json::to_string(&authority).expect("serialize authority");
        let restored: Authority = serde_json::from_str(&json).expect("deserialize authority");
        assert_eq!(authority, restored);
    }
}
