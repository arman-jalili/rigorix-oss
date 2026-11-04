//! #1355 — the in-crate half of the M2 measurement.
//!
//! One precondition refusal, one signed envelope, printed as the SHA-256 of the canonical
//! bytes. The CLI half is `rigorix pinned-refusal` (behind `--features pinned-inputs`), which
//! calls the same operation in `rigorix_engine::audit::application::pinned`; if the two
//! digests differ, the difference is in the runtime.
//!
//! ```text
//! cargo run -q -p rigorix-engine --example pinned_refusal            # wall clock (control)
//! cargo run -q -p rigorix-engine --example pinned_refusal -- \
//!     --pinned-clock 2026-11-03T12:00:00Z --pinned-ids 018f5b8e-0000-7000-8000-000000000001
//! ```
//!
//! The comparison normalises nothing. A pin that silently did not apply would produce an
//! unpinned digest, so the pinned and unpinned runs must differ in size: a whole-second
//! instant serialises shorter than a nanosecond one.

use rigorix_engine::audit::application::dto::PinnedInputs;
use rigorix_engine::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use rigorix_engine::audit::application::factory::AuditEnvelopeFactory;
use rigorix_engine::audit::application::pinned::{
    parse_pinned_clock, parse_pinned_id, refusal_envelope_input,
};

/// The same fixed development key the CLI uses. Not a secret: the measurement is about the
/// envelope's bytes, and a key from the environment would make two runs differ for a reason
/// unrelated to the question.
const DEV_KEY: &str = "m2-pinned-key";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut clock = None;
    let mut ids = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--pinned-clock" => clock = args.next(),
            "--pinned-ids" => ids = args.next(),
            other => return Err(format!("unrecognised argument `{other}`").into()),
        }
    }

    let mut pins = PinnedInputs::none();
    if let Some(value) = clock.as_deref() {
        pins = pins.with_timestamp(parse_pinned_clock(value)?);
    }
    if let Some(value) = ids.as_deref() {
        pins = pins.with_execution_id(parse_pinned_id(value)?);
    }

    let factory =
        AuditEnvelopeFactoryImpl::new(Some(DEV_KEY.to_string())).with_pinned_inputs(pins.clone());
    let mut envelope = factory
        .build_envelope(refusal_envelope_input(&pins))
        .await?;
    factory.sign(&mut envelope)?;

    let bytes = AuditEnvelopeFactoryImpl::canonical_envelope_bytes(&envelope)?;
    let digest = {
        use sha2::{Digest, Sha256};
        Sha256::digest(&bytes)
    };
    println!(
        "pinned_clock={} timestamp={} bytes={} sha256={}",
        clock.is_some(),
        envelope.timestamp.to_rfc3339(),
        bytes.len(),
        hex::encode(digest)
    );
    Ok(())
}
