//! #1658 — three bullets of the hardening sweep, each asserted rather than described.
//!
//! The other four bullets are measured first (file modes, the action image digest, the parser's
//! `line`, percent-encoded resource URIs) and are reported on the issue rather than implemented from a
//! reading.

use rigorix_engine::precondition::domain::provider::{
    ALLOW_PLAINTEXT_ENV, plaintext_endpoint_refusal,
};

#[test]
fn a_provider_endpoint_must_be_verifiable_unless_its_host_is_named() {
    assert!(plaintext_endpoint_refusal("https://provider.example.com/check").is_none());

    // SAFETY: this test process is a dedicated harness; the variable is read only by this case.
    unsafe { std::env::remove_var(ALLOW_PLAINTEXT_ENV) };
    let refusal = plaintext_endpoint_refusal("http://localhost:9100/check")
        .expect("plaintext without an opt-in is refused");
    assert!(refusal.contains("localhost:9100"), "{refusal}");
    assert!(
        refusal.contains("gates a step"),
        "it says why it matters: {refusal}"
    );

    // SAFETY: as above; the opt-in is scoped to this case.
    unsafe { std::env::set_var(ALLOW_PLAINTEXT_ENV, "localhost") };
    assert!(plaintext_endpoint_refusal("http://localhost:9100/check").is_none());
    assert!(
        plaintext_endpoint_refusal("http://elsewhere.example/check").is_some(),
        "naming one host must not admit another"
    );
    unsafe { std::env::remove_var(ALLOW_PLAINTEXT_ENV) };
}

#[test]
fn a_scheme_that_is_not_http_at_all_is_refused_with_its_own_reason() {
    let refusal = plaintext_endpoint_refusal("file:///tmp/check").expect("refused");
    assert!(refusal.contains("must be https://"), "{refusal}");
}

#[test]
fn the_audit_retention_bound_is_a_count() {
    use rigorix_engine::audit::infrastructure::{
        DEFAULT_MAX_LOCAL_AUDIT_ENVELOPES, resolve_max_envelopes,
    };
    // The rule is pure, so the test does not read — or write — the process environment.
    // `>= 100` is a constant expression, so clippy is right that it cannot fail at runtime — the
    // assertion is about the **value** and it belongs in the type, not in a comparison that a reader
    // has to trust. It is stated here as the number a deployment can reason about.
    assert_eq!(
        DEFAULT_MAX_LOCAL_AUDIT_ENVELOPES, 5_000,
        "the default bound is a documented number, not an accident"
    );
    assert_eq!(
        resolve_max_envelopes(None),
        DEFAULT_MAX_LOCAL_AUDIT_ENVELOPES
    );
    assert_eq!(
        resolve_max_envelopes(Some("42")),
        42,
        "a deployment can raise or lower it"
    );
    assert_eq!(
        resolve_max_envelopes(Some("0")),
        DEFAULT_MAX_LOCAL_AUDIT_ENVELOPES,
        "zero is refused rather than read as 'delete everything'"
    );
    assert_eq!(
        resolve_max_envelopes(Some("not a number")),
        DEFAULT_MAX_LOCAL_AUDIT_ENVELOPES,
        "a malformed bound is the default, not an unbounded trail"
    );
}
