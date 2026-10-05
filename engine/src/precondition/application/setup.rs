//! PreconditionSetup — composition-root wiring for the ADR-017 gate.
//!
//! @canonical .pi/architecture/modules/precondition.md#dispatch-integration
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: ISSUE-PF-ACT-1 — arm the dispatch-time precondition gate from
//!   `.rigorix/preconditions.toml` at composition time
//! Issue: #967 (epic EPIC-PRECONDITION-FOLLOWUPS)
//!
//! The `PreconditionDispatchGate` and `with_precondition_gate` existed but had
//! **zero callers**: in production the gate was always `None`, so the operator
//! config was never read and no check ran end-to-end. This module is the bridge
//! that reads the operator file, builds the service + runner, and hands the
//! armed gate to the execution-engine factory.
//!
//! Loading semantics mirror the repository boundary exactly:
//! - **missing file** → `Ok(None)` — fail-open-absent (status quo, no gate)
//! - **valid file** → `Ok(Some(setup))` — an armed gate + `[gating]` mode
//! - **malformed / over-cap** → `Err` — fail closed (the caller must refuse,
//!   never silently downgrade)

use std::path::Path;
use std::sync::Arc;

use crate::precondition::application::gate::DispatchGate;
use crate::precondition::application::gate_impl::PreconditionDispatchGate;
use crate::precondition::application::service_impl::PreconditionServiceImpl;
use crate::precondition::domain::{GatingMode, PreconditionError};
use crate::precondition::infrastructure::ProcessPreconditionRunner;
use crate::precondition::infrastructure::repository::TomlPreconditionRepository;

/// ADR-017 dispatch-time precondition setup: an armed gate plus the R2
/// step-outcome gating mode, both read from `.rigorix/preconditions.toml`.
///
/// `Clone` so it can travel inside `ParallelExecutionFactoryConfig`.
#[derive(Clone)]
pub struct PreconditionSetup {
    /// The armed (or explicitly unarmed) dispatch choke-point gate.
    pub gate: Arc<dyn DispatchGate>,
    /// R2 step-outcome gating (`[gating].release_dependents_on_failure`).
    pub gating_mode: GatingMode,
}

impl PreconditionSetup {
    /// Build the precondition setup from the repository's operator config.
    ///
    /// Reads `<repo_root>/.rigorix/preconditions.toml` synchronously (the same
    /// path the per-dispatch `TomlPreconditionRepository` re-reads — ADR-017
    /// forbids caching the answer across the T₀→Tₙ gap, so this only decides
    /// whether/what to arm). The gate's service re-reads the file at each
    /// dispatch.
    ///
    /// * `Ok(None)` — no config file: no gate (fail-open-absent, status quo).
    /// * `Ok(Some(setup))` — config valid: an **armed** gate plus the parsed
    ///   `[gating]` mode.
    /// * `Err(PreconditionError::ConfigInvalid)` — config present but corrupt
    ///   or over safety caps: the caller must fail closed (build an unarmed
    ///   gate or refuse to start), never fall back to no-gating.
    pub fn from_env(repo_root: &Path) -> Result<Option<Self>, PreconditionError> {
        let config_path = repo_root.join(".rigorix").join("preconditions.toml");
        let repository = TomlPreconditionRepository::new(config_path.clone());
        let Some(config) = repository.load_config_blocking()? else {
            tracing::debug!(
                path = %config_path.display(),
                "precondition: no operator config — gate not armed (fail-open-absent)"
            );
            return Ok(None);
        };
        let gating_mode = config.gating;
        let runner = ProcessPreconditionRunner::new(repo_root);
        let service = PreconditionServiceImpl::new(Box::new(repository), Box::new(runner));
        let gate: Arc<dyn DispatchGate> =
            Arc::new(PreconditionDispatchGate::new(Box::new(service)));
        tracing::info!(
            path = %config_path.display(),
            release_dependents_on_failure = gating_mode.release_dependents_on_failure,
            "precondition: dispatch gate ARMED from operator config (ADR-017)"
        );
        Ok(Some(Self { gate, gating_mode }))
    }

    /// Build a fail-closed setup that refuses every assessed step.
    ///
    /// Used by composition roots when the operator config is present but
    /// malformed: refusing configured-but-unarmed work is deliberate (ADR-017
    /// — never a silent downgrade). Keeps the default gating mode.
    pub fn unarmed(repo_root: &Path, detail: impl Into<String>) -> Self {
        let config_path = repo_root.join(".rigorix").join("preconditions.toml");
        let repository = TomlPreconditionRepository::new(config_path);
        let runner = ProcessPreconditionRunner::new(repo_root);
        let service = PreconditionServiceImpl::new(Box::new(repository), Box::new(runner));
        let gate: Arc<dyn DispatchGate> =
            Arc::new(PreconditionDispatchGate::unarmed(Box::new(service), detail));
        Self {
            gate,
            gating_mode: GatingMode::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::precondition::application::dto::DispatchStep;

    fn step(tool: &str) -> DispatchStep {
        DispatchStep {
            name: "pay".to_string(),
            tool: tool.to_string(),
            parameters: serde_json::json!({}),
        }
    }

    fn write_config(dir: &Path, content: &str) {
        let rigorix = dir.join(".rigorix");
        std::fs::create_dir_all(&rigorix).expect("create .rigorix");
        std::fs::write(rigorix.join("preconditions.toml"), content).expect("write config");
    }

    const VALID: &str = r#"
[[preconditions]]
id = "beneficiary-eligible"
match = { tool = "payment_execute" }
require_params = ["/beneficiary", "/amount"]
command = ["/bin/true"]

[gating]
release_dependents_on_failure = false
"#;

    #[test]
    fn missing_file_returns_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(
            PreconditionSetup::from_env(dir.path())
                .expect("from_env")
                .is_none()
        );
    }

    #[test]
    fn malformed_file_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_config(dir.path(), "this is not = valid toml [[");
        let error = match PreconditionSetup::from_env(dir.path()) {
            Err(error) => error,
            Ok(_) => panic!("malformed config must fail closed"),
        };
        assert!(matches!(error, PreconditionError::ConfigInvalid(_)));
    }

    #[tokio::test]
    async fn valid_file_arms_gate_and_reads_gating_mode() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_config(dir.path(), VALID);
        let setup = PreconditionSetup::from_env(dir.path())
            .expect("from_env")
            .expect("present");
        assert!(
            setup.gate.is_armed().await.expect("is_armed"),
            "a valid config must produce an armed gate"
        );
        assert!(!setup.gating_mode.release_dependents_on_failure);
        // A step that does not match the configured tool dispatches unchanged.
        assert!(
            setup
                .gate
                .assess_with_findings(uuid::Uuid::new_v4(), &step("other_tool"))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn unarmed_gate_refuses_every_step() {
        let dir = tempfile::tempdir().expect("tempdir");
        let setup = PreconditionSetup::unarmed(dir.path(), "malformed config");
        assert!(!setup.gate.is_armed().await.expect("is_armed"));
        let error = setup
            .gate
            .assess_with_findings(uuid::Uuid::new_v4(), &step("any_tool"))
            .await
            .expect_err("unarmed must refuse");
        assert!(matches!(error, PreconditionError::NotArmed { .. }));
    }
}
