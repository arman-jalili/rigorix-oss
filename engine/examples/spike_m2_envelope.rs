//! #1351 M2/M4 — one gate operation, pinned, printed as bytes.
//!
//! The operation is the issue's: **a precondition refusal that produces a signed
//! envelope**. It is built here from pinned inputs and printed as the SHA-256 of the
//! signed canonical bytes, so two processes can be compared with `sha256sum`.
//!
//! ```text
//! cargo run -p rigorix-engine --example spike_m2_envelope            # unpinned clock
//! SPIKE_PINNED_CLOCK=1 cargo run -p rigorix-engine --example spike_m2_envelope
//! ```
//!
//! `SPIKE_PINNED_CLOCK=1` exercises the `BuildEnvelopeInput::pinned_timestamp` seam added
//! by the spike. Without it the factory reads `Utc::now()` — the one site the operation
//! touches that a caller cannot supply.

use std::collections::HashMap;

use chrono::{TimeZone, Utc};
use rigorix_engine::audit::application::dto::BuildEnvelopeInput;
use rigorix_engine::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use rigorix_engine::audit::application::factory::AuditEnvelopeFactory;
use rigorix_engine::audit::domain::EventStatus;
use rigorix_engine::audit::domain::ExecutionEventRef;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Every id and every timestamp the *caller* can supply, supplied.
    let execution_id = uuid::Uuid::parse_str("018f5b8e-0000-7000-8000-000000000001")?;
    let pinned_at = Utc.with_ymd_and_hms(2026, 11, 3, 12, 0, 0).unwrap();
    let pinned_event_at = Utc.with_ymd_and_hms(2026, 11, 3, 11, 59, 59).unwrap();

    let pin_clock = std::env::var("SPIKE_PINNED_CLOCK").as_deref() == Ok("1");

    let input = BuildEnvelopeInput {
        execution_id,
        template_id: "spike-m2-precondition-refusal".to_string(),
        planning_prompt: "refuse the step: precondition beneficiary-eligible failed".to_string(),
        events: vec![ExecutionEventRef {
            event_type: "precondition_refused".to_string(),
            summary: "precondition 'beneficiary-eligible' refused the step".to_string(),
            occurred_at: pinned_event_at,
            correlation_id: None,
            status: EventStatus::Failure,
            payload: None,
        }],
        source: Some("spike-m2".to_string()),
        repository: Some("arman-jalili/rigorix-oss".to_string()),
        author: Some("spike".to_string()),
        identity: None,
        effect_key: Some("sha256:spike-effect".to_string()),
        producer_id: Some("spike-producer".to_string()),
        sequence: Some(1),
        prev_hash: None,
        history_policy: None,
        total_tokens: 0,
        duration_ms: 7,
        git_commit: Some("0000000000000000000000000000000000000000".to_string()),
        git_branch: Some("issue/1351".to_string()),
        model_version: None,
        planning_prompt_content: None,
        file_paths: vec!["engine/src/precondition/application/service_impl.rs".to_string()],
        metadata: Some(HashMap::from([("spike".to_string(), "m2".to_string())])),
        sign: true,
        scoring_results: HashMap::new(),
        // #1351: the seam. `None` = the factory reads the wall clock, which is what
        // makes two runs differ; `Some` = the caller owns the instant.
        pinned_timestamp: pin_clock.then_some(pinned_at),
    };

    let factory = AuditEnvelopeFactoryImpl::new(Some("spike-hmac-key".to_string()));
    let runtime = tokio::runtime::Builder::new_current_thread().build()?;
    let mut envelope = runtime.block_on(factory.build_envelope(input))?;
    factory.sign(&mut envelope)?;

    let bytes = AuditEnvelopeFactoryImpl::canonical_envelope_bytes(&envelope)?;
    let digest = {
        use sha2::{Digest, Sha256};
        Sha256::digest(&bytes)
    };
    println!(
        "pinned_clock={pin_clock} timestamp={} bytes={} sha256={}",
        envelope.timestamp.to_rfc3339(),
        bytes.len(),
        hex::encode(digest)
    );
    Ok(())
}
