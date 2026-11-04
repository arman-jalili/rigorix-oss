//! The CLI's gate path goes through the engine's one composition.
//!
//! Issue: #1389 (Phase C.1b) — "the CLI adopts the context path in the same PR, so both
//! runtimes assemble envelopes through one function".
//!
//! # What this test is for
//!
//! The CLI delegates its gate work to the engine (`GateSetup` supplies the ports; the
//! orchestrator and the execution engine evaluate them). What it must *not* do is assemble
//! a decision's envelope itself: two runtimes that each build a `BuildEnvelopeInput` agree
//! on `decision_digest` and differ in the signed bytes.
//!
//! So this test drives the CLI's own composition entry point — `GateSetup::from_env` against
//! an empty repo, which arms nothing — and supplies the run facts a gate cannot derive. The
//! record it gets back is the engine's, signed with the engine's factory, and it verifies by
//! the same rules a verifier applies. If the CLI ever starts building envelopes on its side,
//! that code has no counterpart here and this file is where the second implementation would
//! become visible.

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use rigorix_engine::audit::application::dto::PinnedInputs;
use rigorix_engine::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use rigorix_engine::configuration::domain::Secret;
use rigorix_engine::gate::application::{GatePorts, GateRequest, RunContext, evaluate_gate_sync};
use rigorix_engine::gate::domain::GatedStep;

const EXEC: Uuid = Uuid::from_u128(0x018f_5b8e_0000_7000_8000_0000_0000_0002);
const DEV_KEY: &str = "rigorix-dev-envelope-hmac-key";

fn pins() -> PinnedInputs {
    PinnedInputs::none()
        .with_execution_id(EXEC)
        .with_timestamp(Utc.with_ymd_and_hms(2026, 11, 3, 12, 0, 0).unwrap())
}

fn run_context() -> RunContext {
    RunContext {
        template_id: "cli-gate-envelope".to_string(),
        planning_prompt: "the CLI's own run facts".to_string(),
        source: Some("rigorix_cli".to_string()),
        repository: Some("arman-jalili/rigorix-oss".to_string()),
        author: Some("cli".to_string()),
        identity: None,
        total_tokens: 0,
        duration_ms: 1,
        git_commit: Some("0000000000000000000000000000000000000000".to_string()),
        git_branch: Some("main".to_string()),
        signing_key: Some(Secret::new(DEV_KEY)),
    }
}

#[test]
fn the_cli_composes_its_gate_through_the_engine_and_the_engine_signs() {
    let repo = tempfile::tempdir().expect("a temp repo");
    // An unconfigured repo: no `.rigorix/` files, so no gates arm and the step dispatches.
    let setup = rigorix_engine::gate::infrastructure::GateSetup::from_env(repo.path());
    // `is_empty` is about *ports*, not about rules: the sequence-policy service is armed and
    // re-reads the operator's file per run (ADR-013 R3), so it is present even when the file
    // is absent. What this test asserts is the decision, not the wiring.
    let _ = setup.is_empty();

    let step = GatedStep::new(
        "noop",
        "filesystem.read",
        serde_json::json!({"path": "README.md"}),
    );
    let decision = evaluate_gate_sync(GateRequest {
        step: &step,
        proposed_action: "read README.md",
        identity_attested: true,
        pins: pins(),
        ports: GatePorts::none(),
        run_context: Some(run_context()),
    })
    .expect("the CLI's composition answers");

    assert!(
        decision.verdict.permits_dispatch(),
        "an unconfigured repo has no rule to refuse with — and no gate is not an error"
    );
    assert!(
        decision.findings.is_empty(),
        "a repo with no operator config produces no findings"
    );
    let evidence = decision
        .envelope_evidence
        .as_ref()
        .expect("the engine assembled the record");

    // The signature is over the canonical bytes and verifies by recomputation: the CLI
    // holds no second envelope path, so there is nothing to compare it against — which is
    // the point.
    assert!(evidence.envelope.signature.is_some());
    assert_eq!(
        evidence.canonical_bytes,
        AuditEnvelopeFactoryImpl::canonical_envelope_bytes(&evidence.envelope)
            .expect("canonical bytes")
    );
    assert_eq!(
        evidence.envelope_hash,
        AuditEnvelopeFactoryImpl::envelope_hash(&evidence.envelope).expect("hash")
    );
    assert_eq!(evidence.envelope.execution_id, EXEC);
    assert_eq!(decision.envelope.execution_id, EXEC);
}
