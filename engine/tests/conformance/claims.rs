//! Claims conformance (F-20260907-02): engine identity types vs rigorix-sdk
//! `schemas/claims.json` v1.
//!
//! Serializes real `IdentityClaim` variants (idp_token / local_principal /
//! unverified; token_ref null + set) and `IdentityRef` through engine serde
//! and validates against the SDK schema. Options serialize as null (no skips);
//! source is snake_case; degradation is a first-class state.

use rigorix_engine::identity::domain::{IdentityClaim, IdentityRef, IdentitySource};

use crate::{assert_valid, load_schema};

fn base_claim() -> IdentityClaim {
    IdentityClaim {
        subject: "user@org".to_string(),
        issuer: "https://idp.example.com".to_string(),
        authority: Some("admin".to_string()),
        source: IdentitySource::IdpToken,
        auth_method: Some("device_code".to_string()),
        issued_at: chrono::Utc::now() - chrono::Duration::minutes(5),
        expires_at: Some(chrono::Utc::now() + chrono::Duration::minutes(10)),
        token_ref: Some("keychain://default/rigorix/idp-token".to_string()),
    }
}

#[test]
fn idp_token_claim_conforms() {
    let claim = base_claim();
    let value = serde_json::to_value(&claim).expect("engine serde");
    assert_valid(
        &load_schema("claims.json"),
        &value,
        "IdentityClaim (idp_token)",
    );
}

#[test]
fn local_principal_claim_conforms() {
    let claim = IdentityClaim {
        source: IdentitySource::LocalPrincipal,
        authority: None,
        auth_method: None,
        token_ref: None, // local principal has no token reference
        ..base_claim()
    };
    let value = serde_json::to_value(&claim).expect("engine serde");
    assert_valid(
        &load_schema("claims.json"),
        &value,
        "IdentityClaim (local_principal, token_ref null)",
    );
}

#[test]
fn unverified_claim_conforms() {
    let claim = IdentityClaim {
        source: IdentitySource::Unverified,
        authority: None,
        expires_at: None,
        token_ref: None,
        ..base_claim()
    };
    let value = serde_json::to_value(&claim).expect("engine serde");
    assert_valid(
        &load_schema("claims.json"),
        &value,
        "IdentityClaim (unverified — explicit degradation)",
    );
}

#[test]
fn identity_ref_conforms() {
    let claim = base_claim();
    let reference = IdentityRef::from_claim(&claim);
    let value = serde_json::to_value(&reference).expect("engine serde");
    // IdentityRef must never carry token_ref / auth_method.
    assert!(!value.as_object().unwrap().contains_key("token_ref"));
    assert!(!value.as_object().unwrap().contains_key("auth_method"));
    // claims.json root is `$ref: identityClaim`; IdentityRef validates
    // against the redacted `$defs.identityRef` sub-schema.
    let schema = load_schema("claims.json");
    let identity_ref_schema = schema.get("$defs").unwrap().get("identityRef").unwrap();
    assert_valid(identity_ref_schema, &value, "IdentityRef (redacted)");
}

#[test]
fn identity_ref_no_expiry_conforms() {
    let claim = IdentityClaim {
        expires_at: None,
        ..base_claim()
    };
    let reference = IdentityRef::from_claim(&claim);
    let value = serde_json::to_value(&reference).expect("engine serde");
    let schema = load_schema("claims.json");
    let identity_ref_schema = schema.get("$defs").unwrap().get("identityRef").unwrap();
    assert_valid(identity_ref_schema, &value, "IdentityRef (no expiry)");
}
