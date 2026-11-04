//! #1355 — the pinned-input carrier and the one site it changes.
//!
//! These tests exist so the measurement's two claims cannot rot silently:
//!
//! 1. an unpinned factory derives exactly what it always did (no behaviour change), and
//! 2. a pinned factory produces the same bytes twice, and different bytes when a pin
//!    differs by one second — the comparison must be able to fail, or it proves nothing.
//!
//! They also hold the semver decision: `BuildEnvelopeInput` has no `pinned_*` field, so
//! the other crates in this workspace that construct it literally keep compiling.

use chrono::{TimeZone, Utc};
use rigorix_engine::audit::application::dto::PinnedInputs;
use rigorix_engine::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use rigorix_engine::audit::application::factory::AuditEnvelopeFactory;
use rigorix_engine::audit::application::pinned::{parse_pinned_clock, refusal_envelope_input};

const KEY: &str = "m2-pinned-key";

/// The witness, in one place (#1357).
///
/// This is the digest the *shipped* operation produces for the pins in
/// `docs/spike-portable-slice-2026-11.md` §2 — and the only place the value is stated. The
/// report cites this test instead of holding a second copy, because a value kept in two
/// places is two values that drift: the previous version of the report published
/// `8b2cc83d…`, which belonged to the discarded apparatus and reproduced nowhere on the
/// merged tree, and **no assertion noticed** because both runs still agreed with each other.
///
/// Editing the fixture in `audit::application::pinned` (a rename, a dropped field, a
/// different event instant) now fails here, which forces the report's number to move in the
/// same commit.
const GOLDEN_DIGEST: &str = "c47b27f9300ed91b06e9712ac5df60714aa77a2a23d9bf41aa7d52f87393bc96";

/// The pins the report documents.
const GOLDEN_PINS: (&str, &str) = (
    "2026-11-03T12:00:00Z",
    "018f5b8e-0000-7000-8000-000000000001",
);

fn pinned_at() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 3, 12, 0, 0).unwrap()
}

async fn signed_digest(pins: &PinnedInputs) -> (String, usize, String) {
    let factory =
        AuditEnvelopeFactoryImpl::new(Some(KEY.to_string())).with_pinned_inputs(pins.clone());
    let mut envelope = factory
        .build_envelope(refusal_envelope_input(pins))
        .await
        .expect("the fixture builds");
    factory.sign(&mut envelope).expect("the fixture signs");
    let bytes = AuditEnvelopeFactoryImpl::canonical_envelope_bytes(&envelope)
        .expect("the fixture canonicalises");
    let digest = {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(&bytes))
    };
    (digest, bytes.len(), envelope.timestamp.to_rfc3339())
}

#[tokio::test]
async fn the_documented_pins_produce_the_golden_digest() {
    use rigorix_engine::audit::application::pinned::parse_pinned_id;

    let (clock, ids) = GOLDEN_PINS;
    let pins = PinnedInputs::none()
        .with_timestamp(parse_pinned_clock(clock).expect("the documented clock parses"))
        .with_execution_id(parse_pinned_id(ids).expect("the documented id parses"));
    let (digest, bytes, _) = signed_digest(&pins).await;

    assert_eq!(
        digest, GOLDEN_DIGEST,
        "the shipped fixture no longer produces the digest the report cites.\n\
         If the fixture changed on purpose, re-measure with\n  \
         cargo run -q -p rigorix-engine --example pinned_refusal -- \\\n    \
         --pinned-clock {clock} --pinned-ids {ids}\n\
         and move this constant and docs/spike-portable-slice-2026-11.md §2 in the same commit."
    );
    assert_eq!(
        bytes, 853,
        "the golden envelope's length moved too — a length is not a fingerprint, but a change \
         in both means the subject changed, not just its content"
    );
}

#[tokio::test]
async fn a_pinned_operation_produces_the_same_bytes_twice() {
    let pins = PinnedInputs::none().with_timestamp(pinned_at());
    let first = signed_digest(&pins).await;
    let second = signed_digest(&pins).await;
    assert_eq!(
        first.0, second.0,
        "the same pins must produce the same digest, or the operation is not deterministic"
    );
    assert!(
        first.2.starts_with("2026-11-03T12:00:00"),
        "the pin must reach the envelope's own timestamp, got {}",
        first.2
    );
}

#[tokio::test]
async fn one_second_of_difference_changes_the_digest() {
    let base = signed_digest(&PinnedInputs::none().with_timestamp(pinned_at())).await;
    let off_by_one = signed_digest(
        &PinnedInputs::none().with_timestamp(Utc.with_ymd_and_hms(2026, 11, 3, 12, 0, 1).unwrap()),
    )
    .await;
    assert_ne!(
        base.0, off_by_one.0,
        "a one-second pin difference MUST change the digest: a comparison that absorbs it \
         is not a comparison"
    );
    assert_eq!(
        base.1, off_by_one.1,
        "the two envelopes are the same length, so only the bytes differ — not the size"
    );
}

#[tokio::test]
async fn an_unpinned_operation_reads_the_clock_and_seals_deeper_bytes() {
    let unpinned = signed_digest(&PinnedInputs::none()).await;
    let pinned = signed_digest(&PinnedInputs::none().with_timestamp(pinned_at())).await;
    assert!(
        !unpinned.2.starts_with("2026-11-03T12:00:00"),
        "an unpinned run must not carry the pinned instant, got {}",
        unpinned.2
    );
    // A whole-second instant serialises shorter than a nanosecond one. Recorded because a
    // comparison that normalised timestamp precision would hide a real byte difference.
    assert!(
        pinned.1 < unpinned.1,
        "the pinned envelope should be shorter (whole-second precision): pinned={} unpinned={}",
        pinned.1,
        unpinned.1
    );
    assert_ne!(pinned.0, unpinned.0);
}

#[tokio::test]
async fn a_pinned_id_reaches_the_envelope_and_moves_the_digest() {
    let id = uuid::Uuid::parse_str("018f5b8e-0000-7000-8000-000000000002").unwrap();
    let with_id = signed_digest(
        &PinnedInputs::none()
            .with_timestamp(pinned_at())
            .with_execution_id(id),
    )
    .await;
    let without = signed_digest(&PinnedInputs::none().with_timestamp(pinned_at())).await;
    assert_ne!(
        with_id.0, without.0,
        "a pinned execution id must reach the signed bytes"
    );
}

#[test]
fn pinned_inputs_are_empty_until_something_is_pinned() {
    assert!(PinnedInputs::none().is_empty());
    assert!(!PinnedInputs::none().with_timestamp(pinned_at()).is_empty());
    assert_eq!(
        PinnedInputs::none().with_timestamp(pinned_at()).timestamp(),
        Some(pinned_at())
    );
    assert_eq!(PinnedInputs::none().execution_id(), None);
}

#[test]
fn the_cli_forms_are_parsed_strictly_rather_than_defaulted() {
    assert_eq!(
        parse_pinned_clock("2026-11-03T12:00:00Z").unwrap(),
        pinned_at()
    );
    // A pin that silently did not apply would be an unpinned measurement wearing a pinned
    // label, so an unparseable value is an error, never a fallback.
    assert!(parse_pinned_clock("2026-11-03 12:00:00").is_err());
    assert!(parse_pinned_clock("yesterday").is_err());
}
