//! What the engine observed, signed — and not producible from outside (#1527).
//!
//! The tests here are about the *surface*: an attestation cannot be minted from JSON without a valid
//! signature, a tampered byte fails verification, and a quality level the execution did not establish
//! is recorded as the caller's claim.

use rigorix_engine::quality_gates::domain::level::QualityLevel;
use rigorix_engine::receipt::domain::attestation::ObservedStep;
use rigorix_engine::receipt::{
    AttestationBuilder, BudgetDeclared, QualitySource, Receipt, Sandbox, SandboxRegime, Workspace,
    WorkspaceKind, sign, verify, verify_attestation,
};
use uuid::Uuid;

const KEY: &[u8] = b"receipt-attestation-test-key";

fn verified() -> rigorix_engine::receipt::VerifiedReceipt {
    let receipt = Receipt::new(
        Uuid::nil(),
        Workspace {
            kind: WorkspaceKind::CommitTree,
            value: "4b825dc642cb6eb9a060e54bf8d69288fbee4904".to_string(),
        },
        Sandbox {
            regime: SandboxRegime::Container,
            image_digest: "sha256:1111".to_string(),
        },
        Vec::new(),
        Vec::new(),
        BudgetDeclared {
            max_iterations: 3,
            max_tokens: 30,
        },
        QualityLevel::Package,
    )
    .expect("no steps, no credentials");
    verify(&receipt, "4b825dc642cb6eb9a060e54bf8d69288fbee4904").expect("verifies")
}

fn attestation_with_observation() -> rigorix_engine::receipt::Attestation {
    let verified = verified();
    let mut builder = AttestationBuilder::from_verified(&verified);
    builder.observe_step(ObservedStep {
        step_id: "a".to_string(),
        observed_effects: vec![],
        expectation_met: true,
        expectation_detail: None,
    });
    builder.settle_effect("effect-1");
    builder.build(1, 5)
}

#[test]
fn attestation_verification_a_signed_attestation_verifies_and_a_flipped_digest_does_not() {
    let mut attestation = attestation_with_observation();
    sign(&mut attestation, KEY).expect("signs");
    assert!(attestation.is_signed());

    let bytes = serde_json::to_vec(&attestation).expect("serializes");
    let verified = verify_attestation(&bytes, KEY).expect("verifies");
    assert_eq!(verified.attestation().execution_id(), Uuid::nil());

    // Flip one character of one *value* — the settled effect id — and the signature no longer
    // answers for the bytes. (Touching a field name would only prove the JSON was invalid.)
    let mut tampered = bytes.clone();
    let position = tampered
        .windows(8)
        .position(|w| w == b"effect-1")
        .expect("the settled effect id is in the body");
    tampered[position + 7] = b'2';
    let err = verify_attestation(&tampered, KEY).unwrap_err();
    assert!(err.contains("signature"), "{err}");
}

#[test]
fn attestation_verification_an_unsigned_attestation_is_not_an_attestation() {
    let attestation = attestation_with_observation();
    assert!(!attestation.is_signed());
    let bytes = serde_json::to_vec(&attestation).expect("serializes");
    let err = verify_attestation(&bytes, KEY).unwrap_err();
    assert!(err.contains("not signed"), "{err}");
}

#[test]
fn attestation_verification_another_key_does_not_verify() {
    let mut attestation = attestation_with_observation();
    sign(&mut attestation, KEY).expect("signs");
    let bytes = serde_json::to_vec(&attestation).expect("serializes");
    let err = verify_attestation(&bytes, b"a-different-key").unwrap_err();
    assert!(err.contains("signature"), "{err}");
}

#[test]
fn attestation_verification_the_receipt_digest_is_echoed_not_restated() {
    // The attestation answers a receipt by digest. It does not carry a copy of the receipt, so it
    // cannot disagree with one.
    let verified = verified();
    let builder = AttestationBuilder::from_verified(&verified);
    let attestation = builder.build(0, 0);
    assert_eq!(attestation.receipt_digest(), verified.digest());
}

#[test]
fn attestation_verification_the_signature_covers_the_body_and_not_itself() {
    let mut attestation = attestation_with_observation();
    let body_before = attestation.canonical_body().expect("canonical");
    sign(&mut attestation, KEY).expect("signs");
    let body_after = attestation.canonical_body().expect("canonical");
    assert_eq!(
        body_before, body_after,
        "signing must not change the bytes it signs"
    );
}

#[test]
fn attestation_verification_the_type_cannot_be_minted_from_json_without_a_signature() {
    // The `Deserialize` impl belongs to a private wire mirror, so the only path from bytes to a
    // value is `verify_attestation`. This test asserts the surface: the module exports no `new`, no
    // `Default`, and no public deserializer for the type.
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/receipt/domain/attestation.rs"),
    )
    .expect("the module is readable");
    assert!(
        !source.contains("impl<'de> Deserialize<'de> for Attestation"),
        "Attestation must not implement Deserialize itself"
    );
    assert!(
        !source.contains("impl Default for Attestation"),
        "Attestation must not have a default: an empty observation reads as success"
    );
    assert!(
        !source.contains("pub fn new("),
        "Attestation must not have a public constructor"
    );
}

#[test]
fn attestation_verification_a_caller_level_is_never_reported_as_passed() {
    let verified = verified();
    let builder = AttestationBuilder::from_verified(&verified);
    let attestation = builder.build(0, 0);
    // `Package` was the caller's claim; no step executed it.
    assert_eq!(attestation.quality().level, QualityLevel::Package);
    assert_eq!(attestation.quality().source, QualitySource::Caller);
}
