//! Seed a producer's genesis envelope for `demo/anchor-e2e` — signed, not hand-written.
//!
//! Issue: #1278 — `demo/anchor-e2e` could not pass: it seeded an envelope with
//! `"signature": null` and ingest (correctly) rejects unsigned envelopes, so the
//! demo stopped before the step that asserts its four outcomes.
//!
//! The envelope is an [`AuditEnvelope`] and the signature is
//! [`AuditEnvelopeFactoryImpl::sign`] — the engine's canonical bytes and the
//! engine's HMAC. Nothing about the canonical form is re-implemented here: a
//! copy of it in a shell heredoc (or a Python dict) is a copy that drifts, and
//! the drift surfaces as `INVALID_SIGNATURE` on somebody's clean machine.
//!
//! # Usage
//!
//! ```text
//! AUDIT_ENVELOPE_HMAC_SECRET=<the key the ingest verifies with> \
//! PRODUCER_ID=local \
//!   cargo run -q -p rigorix-engine --example seed_envelope
//! ```
//!
//! Prints the signed envelope as JSON on stdout (everything else goes to
//! stderr, so `$(...)` capture stays clean).
//!
//! # Env
//!
//! | Var | Required | Meaning |
//! |---|---|---|
//! | `AUDIT_ENVELOPE_HMAC_SECRET` | **yes** | The key the enterprise verifies with. Absent or empty is a hard error: the seed would be refused by ingest, and a demo that posts an unsigned envelope has learned nothing |
//! | `PRODUCER_ID` | no (default `local`) | The chain this genesis envelope opens |
//! | `SEED_REQUIREMENT_STEP` | no (default `registration_remove`) | Step name carried by the seeded `node_completed` event |
//! | `SEED_TEMPLATE_ID` | no (default `e2e`) | Template id recorded on the envelope |
//!
//! # Why the timestamp is generated here
//!
//! The live assertion reads history with `since = now - 24h`, so the seed must
//! be timestamped at run time; a fixed date goes stale and yields an empty
//! slice a day later. `Utc::now()` is the only honest source for it.

use rigorix_engine::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use rigorix_engine::audit::domain::{
    AuditEnvelope, EventStatus, ExecutionEventRef, HistoryIntegrity,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The key comes from the same variable the enterprise reads, so the two
    // sides cannot drift (the demo script exports one value to both).
    let key = std::env::var("AUDIT_ENVELOPE_HMAC_SECRET").unwrap_or_default();
    if key.trim().is_empty() {
        eprintln!(
            "seed_envelope: AUDIT_ENVELOPE_HMAC_SECRET is not set — an unsigned envelope is \
             refused by ingest (INVALID_SIGNATURE), so there is nothing to seed. Export the key \
             the enterprise verifies with."
        );
        std::process::exit(2);
    }

    let producer_id = env_or("PRODUCER_ID", "local");
    let step = env_or("SEED_REQUIREMENT_STEP", "registration_remove");
    let template_id = env_or("SEED_TEMPLATE_ID", "e2e");

    let now = chrono::Utc::now();
    let envelope = AuditEnvelope {
        execution_id: uuid::Uuid::new_v4(),
        // Run-time timestamp: the history read window is relative to it.
        timestamp: now,
        template_id,
        // Deterministic planning hash — this is a fixture, not a real plan.
        planning_hash: "ab".repeat(32),
        source: Some("rigorix_cli".to_string()),
        repository: Some("e2e/repo".to_string()),
        author: Some("jeff@corp".to_string()),
        identity: None,
        effect_key: None,
        total_tokens: 0,
        duration_ms: 0,
        git_commit: None,
        git_branch: None,
        model_version: None,
        planning_prompt: None,
        file_paths: Vec::new(),
        events: vec![ExecutionEventRef {
            event_type: "node_completed".to_string(),
            summary: step.clone(),
            occurred_at: now,
            correlation_id: None,
            status: EventStatus::Success,
            payload: Some(serde_json::json!({ "step_name": step })),
        }],
        scoring_results: std::collections::HashMap::new(),
        signature: None,
        // Signed below, so the evidence is not degraded. Flipping this to `true`
        // would be a false claim about the record this program just signed.
        evidence_degraded: false,
        producer_id: Some(producer_id),
        // Genesis: sequence 0, no predecessor.
        sequence: Some(0),
        prev_hash: None,
        // Prior evidence for the demo's "local, unanchored" starting point.
        history_integrity: Some(HistoryIntegrity::LocalUnanchored),
        history_policy: None,
        anchor_head: None,
        approval_events: Vec::new(),
        scope_violations: Vec::new(),
        // Left `None` on purpose: the ingest's canonical mirror does not carry
        // `scope_check` yet, so a value here would change the bytes the
        // enterprise re-serializes and the signature would not match. A seed
        // that predates the field is exactly what "legacy-shaped evidence" means.
        scope_check: None,
        sequence_policy_findings: Vec::new(),
        requirement_findings: Vec::new(),
        precondition_findings: Vec::new(),
        decision_context_ref: None,
    };

    let factory = AuditEnvelopeFactoryImpl::new(Some(key));
    let mut signed = envelope;
    factory.sign(&mut signed)?;

    println!("{}", serde_json::to_string(&signed)?);
    Ok(())
}

/// Read an env var, falling back to `default` when it is unset **or empty**
/// (an exported-but-empty var in a shell must not become an empty field).
fn env_or(name: &str, default: &str) -> String {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => default.to_string(),
    }
}
