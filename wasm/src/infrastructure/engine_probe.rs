//! The reachable engine call — the subject of the C.3 measurement (#1383).
//!
//! Sized before it is designed, on purpose: the question this module answers is "what does
//! one reachable call into the engine's gate cost the shipped bundle", and a call the
//! linker could drop would answer a different question.

/// Drive the engine's gate composition once, with no ports configured.
///
/// Measurement scaffolding, not the adapter: it proves the composition links into
/// `wasm32-unknown-unknown` and lets the size gate see the real cost. `EngineGate` replaces
/// it once the number is in.
#[cfg(feature = "engine")]
pub fn probe_engine_gate(execution_id: &str) -> Option<bool> {
    use rigorix_engine::audit::application::dto::PinnedInputs;
    //  arrives through the engine's dependency; re-exported use below.
    use rigorix_engine::gate::application::{
        evaluate_gate_sync, GatePorts, GateRequest, GatedStep,
    };

    let step = GatedStep::new(
        "probe",
        "filesystem.read",
        serde_json::json!({"path": "probe"}),
    );
    // The host mints the id (Ports::ids); parsing it here keeps this crate free of a
    // dependency it does not otherwise need.
    let execution_id = uuid::Uuid::parse_str(execution_id).ok()?;
    let pins = PinnedInputs::none().with_execution_id(execution_id);
    evaluate_gate_sync(GateRequest {
        step: &step,
        proposed_action: "probe",
        identity_attested: false,
        pins,
        ports: GatePorts::none(),
        run_context: None,
    })
    .ok()
    .map(|decision| decision.verdict.permits_dispatch())
}
