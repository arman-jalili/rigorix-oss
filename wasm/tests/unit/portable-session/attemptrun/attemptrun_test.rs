//! Contract: `AttemptRun` orchestrates; it cannot request an allow (AC 10; FR-009).
//!
//! Issue: #1294 (contract freeze)

use rigorix_gate_wasm::application::AttemptRun;
use rigorix_gate_wasm::domain::engine_seam::GateEvaluator;
use rigorix_gate_wasm::domain::errors::AttemptError;
use rigorix_gate_wasm::domain::outcome::RunOutcome;
use rigorix_gate_wasm::domain::ports::Ports;
use rigorix_gate_wasm::domain::services::EvidenceProjection;
use rigorix_gate_wasm::domain::session::ReplaySession;
use rigorix_gate_wasm::domain::value::GatedAction;

struct NoProjection;

impl EvidenceProjection for NoProjection {
    fn project(
        &self,
        findings: &[rigorix_gate_wasm::domain::engine_seam::EngineFinding],
    ) -> Vec<rigorix_gate_wasm::domain::outcome::StepOutcome> {
        assert!(findings.is_empty(), "no findings in a contract test");
        Vec::new()
    }
}

#[test]
fn execute_has_no_way_to_ask_for_an_allow() {
    // Five parameters, none of which is a verdict, a flag, an override or an approval.
    // This is the structural half of "the host cannot influence a decision".
    fn assert_signature<S: AttemptRun>(
        service: &S,
        session: &mut ReplaySession,
        action: GatedAction,
        ports: Ports<'_>,
        gate: &mut dyn GateEvaluator,
    ) -> Result<RunOutcome, AttemptError> {
        service.execute(session, action, ports, gate)
    }
    let _ = assert_signature::<rigorix_gate_wasm::application::AttemptRunService<NoProjection>>;
}

#[test]
fn the_service_is_constructed_with_its_projection() {
    // The projection is injected once, at construction — not passed per call, so it
    // cannot vary between the run and its evidence.
    let service = rigorix_gate_wasm::application::AttemptRunService::new(NoProjection);
    let _ = service.projection();
}

#[test]
fn engine_failure_is_an_error_not_an_allow() {
    use rigorix_gate_wasm::domain::engine_seam::EngineBindingError;
    let failure = AttemptError::EngineFailure {
        source: EngineBindingError::Rejected {
            detail: "scenario version mismatch".into(),
        },
    };
    assert!(failure.to_string().contains("could not evaluate"));
    assert!(!failure.to_string().to_lowercase().contains("allowed"));
}
