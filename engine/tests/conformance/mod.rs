//! Contract-sync conformance suite (F-20260907-01/02) — rigorix-sdk schemas.
//!
//! Validates REAL engine output (built through the real factory / real
//! execution flow — never hand-rolled JSON) against rigorix-sdk schema files:
//!   - `envelope.json` v1  (F-20260907-01) — engine envelope serialization
//!   - `policy.json` v1    (F-20260907-02) — parsed sequence-policy config
//!   - `claims.json` v1    (F-20260907-02) — IdentityClaim / IdentityRef
//!
//! # How to run (CI conformance job)
//!
//! The conformance job checks out `arman-jalili/rigorix-sdk` (same-owner
//! private, workflow token `RIGORIX_SDK_TOKEN`) and points
//! `RIGORIX_SDK_SCHEMAS` at its `schemas/` dir, then:
//!
//! ```bash
//! RIGORIX_SDK_SCHEMAS=<sdk>/schemas \
//!   cargo test -p rigorix-engine --features conformance --test conformance
//! ```
//!
//! Locally (schemas checked out anywhere):
//! ```bash
//! RIGORIX_SDK_SCHEMAS=/path/to/rigorix-sdk/schemas cargo test -p rigorix-engine --features conformance
//! ```

mod claims;
mod envelope;
mod fixtures;
mod policy;

use std::path::PathBuf;

/// Resolve the rigorix-sdk schemas directory.
///
/// Priority: `RIGORIX_SDK_SCHEMAS` env → sibling `rigorix-sdk/schemas`
/// checkout next to this repo (local default). Missing → clear failure (the
/// suite must never silently skip — the CI job always sets the env).
pub(crate) fn schema_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("RIGORIX_SDK_SCHEMAS") {
        let p = PathBuf::from(dir);
        assert!(
            p.join("envelope.json").exists(),
            "RIGORIX_SDK_SCHEMAS={} lacks envelope.json",
            p.display()
        );
        return p;
    }
    // Local default: sibling checkout of rigorix-sdk next to rigorix-oss.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")); // .../engine
    let sibling = manifest
        .parent()
        .expect("engine under repo root")
        .parent()
        .expect("repo root")
        .join("rigorix-sdk")
        .join("schemas");
    if sibling.join("envelope.json").exists() {
        return sibling;
    }
    panic!(
        "conformance: set RIGORIX_SDK_SCHEMAS to a rigorix-sdk checkout's \
         schemas/ dir (e.g. <rigorix-sdk>/schemas). Tried env and {}.",
        sibling.display()
    );
}

/// Read + parse a schema file as a JSON value.
pub(crate) fn load_schema(name: &str) -> serde_json::Value {
    let path = schema_dir().join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("conformance: read {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("conformance: parse {}: {e}", path.display()))
}

/// Validate an instance JSON value against a loaded schema (draft 2020-12 via
/// the `jsonschema` crate — the same validator rigorix-sdk's rigorix-verifier
/// uses). Panics with all validation errors on failure.
pub(crate) fn assert_valid(schema: &serde_json::Value, instance: &serde_json::Value, label: &str) {
    let compiled = jsonschema::validator_for(schema).unwrap_or_else(|e| {
        panic!("conformance: compile schema for {label}: {e}");
    });
    let errors: Vec<String> = compiled
        .iter_errors(instance)
        .map(|e| e.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "conformance: {label} failed schema validation:\n  {}",
        errors.join("\n  ")
    );
}
