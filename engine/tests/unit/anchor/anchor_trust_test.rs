//! #1663 — the anchor client requires a verifiable channel and refuses the published seed.
//!
//! The anchor is how a host proves its history has not been rewritten. The audit showed the seam
//! failing from both sides: the enterprise anchor signs with `07…07` when its environment is unset,
//! and the OSS client required no scheme — so an on-path attacker could answer `RIGORIX_ANCHOR_URL`
//! without knowing any key at all, and that answer gates a step.

use rigorix_engine::audit::domain::anchor::HistorySlice;
use rigorix_engine::audit::infrastructure::anchor::{
    ANCHOR_ALLOW_PLAINTEXT_ENV, PUBLISHED_DEV_SEED_PUBLIC_KEY, key_fingerprint,
    plaintext_anchor_refusal,
};
use rigorix_engine::audit::infrastructure::anchor::{AnchorError, verify_slice};

#[test]
fn an_https_anchor_needs_no_opt_in() {
    assert!(plaintext_anchor_refusal("https://anchor.example.com/v1").is_none());
}

#[test]
fn a_plaintext_anchor_is_refused_unless_the_host_is_named() {
    // SAFETY: this test process is a dedicated harness; the variable is read only by this case.
    unsafe { std::env::remove_var(ANCHOR_ALLOW_PLAINTEXT_ENV) };
    let refusal = plaintext_anchor_refusal("http://localhost:8080/v1")
        .expect("plaintext without an opt-in is refused");
    assert!(refusal.contains("localhost:8080"), "{refusal}");
    assert!(
        refusal.contains("on-path"),
        "it says what the risk is: {refusal}"
    );
    assert!(refusal.contains(ANCHOR_ALLOW_PLAINTEXT_ENV), "{refusal}");

    // SAFETY: as above; the opt-in is scoped to this case and removed at the end.
    unsafe { std::env::set_var(ANCHOR_ALLOW_PLAINTEXT_ENV, "localhost") };
    assert!(
        plaintext_anchor_refusal("http://localhost:8080/v1").is_none(),
        "an operator who names the host has made a decision"
    );
    assert!(
        plaintext_anchor_refusal("http://evil.example/v1").is_some(),
        "naming one host must not admit another"
    );
    unsafe { std::env::remove_var(ANCHOR_ALLOW_PLAINTEXT_ENV) };
}

#[test]
fn a_scheme_that_is_neither_http_nor_https_is_refused() {
    let refusal = plaintext_anchor_refusal("ftp://anchor.example.com").expect("refused");
    assert!(
        refusal.contains("does not start with https://"),
        "{refusal}"
    );
}

#[test]
fn a_slice_signed_with_the_published_seed_is_refused_by_name() {
    let slice = HistorySlice {
        scope: "default".to_string(),
        since: chrono::Utc::now(),
        actions: Vec::new(),
        head_hash: "00".repeat(32),
        sig: Some("00".repeat(64)),
    };
    let error = verify_slice(&slice, PUBLISHED_DEV_SEED_PUBLIC_KEY)
        .expect_err("the published seed must not verify anything");
    match error {
        AnchorError::SignatureInvalid(message) => {
            assert!(message.contains("published development seed"), "{message}");
            assert!(
                message.contains(&key_fingerprint(PUBLISHED_DEV_SEED_PUBLIC_KEY)),
                "the refusal names the fingerprint so an operator can recognise the key: {message}"
            );
        }
        other => panic!("expected SignatureInvalid, got {other:?}"),
    }
}

#[test]
fn the_fingerprint_is_stable_and_short() {
    let first = key_fingerprint(PUBLISHED_DEV_SEED_PUBLIC_KEY);
    let second = key_fingerprint(&format!("  {}  ", PUBLISHED_DEV_SEED_PUBLIC_KEY));
    assert_eq!(
        first, second,
        "the fingerprint ignores surrounding whitespace"
    );
    assert_eq!(first.len(), 16, "eight bytes, hex: {first}");
    assert_ne!(
        first,
        key_fingerprint(&"11".repeat(32)),
        "different keys differ"
    );
}
