//! Contract: `seal` is once, stamped, and refuses rather than sanitises (AC 6, 7).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::domain::artifact::ArtifactSet;
use rigorix_gate_wasm::domain::errors::SealError;
use rigorix_gate_wasm::domain::services::{ArtifactContentPolicy, ArtifactStamp, ContentRule};
use rigorix_gate_wasm::domain::session::ReplaySession;

#[test]
fn seal_signature_requires_a_stamp_and_a_content_policy() {
    // Neither is optional: a seal without a stamp produces an unattributable record, and
    // a seal without a scan produces a leaking one.
    let _seal: fn(
        &mut ReplaySession,
        &dyn ArtifactStamp,
        &dyn ArtifactContentPolicy,
    ) -> Result<ArtifactSet, SealError> = ReplaySession::seal;
}

#[test]
fn content_rules_are_enumerable_and_value_free() {
    // The rule label is what reaches a message. It never contains the offending value —
    // the value would be the leak.
    let rules = [
        ContentRule::KeyMaterial,
        ContentRule::AbsolutePath,
        ContentRule::PersonalData,
        ContentRule::Credential,
    ];
    let labels: Vec<&str> = rules.iter().map(|r| r.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "key_material",
            "absolute_path",
            "personal_data",
            "credential"
        ]
    );
    assert_eq!(ContentRule::AbsolutePath.to_string(), "absolute_path");
}

#[test]
fn seal_failures_are_exhaustive() {
    let modes = [
        SealError::NoAttempt,
        SealError::AlreadySealed,
        SealError::ContentRejected {
            rule: ContentRule::KeyMaterial,
        },
    ];
    assert_eq!(modes.len(), 3);
    // The rejection message names the rule, never the bytes.
    assert_eq!(
        modes[2].to_string(),
        "the artifact set was rejected: key_material"
    );
}
