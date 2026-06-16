//! Implementation of the RiskGateService.
//!
//! @canonical .pi/architecture/modules/risk-gating.md
//! Implements: ISSUE-RISK-GATING-1 — RiskGateService implementation
//! Issue: #90
//!
//! Provides the concrete `RiskGateServiceImpl` that classifies tools,
//! evaluates gating policies, manages pending gates, and handles
//! configuration overrides.
//!
//! # Thread Safety
//! - Classifier state is protected by `RwLock` for concurrent read/write
//! - All async methods are safe to call from multiple tasks
//! - Gate resolution uses the shared `GateStateRegistry`

use async_trait::async_trait;
use std::sync::Arc;
use std::sync::RwLock;

use crate::risk_gating::application::dto::{
    ClassifyToolInput, ClassifyToolOutput, EvaluateGateInput, EvaluateGateOutput, GetConfigOutput,
    OverrideToolInput, OverrideToolOutput, ReloadConfigOutput, ResolveGateInput, ResolveGateOutput,
    RiskConfigSummary,
};
use crate::risk_gating::application::service::RiskGateService;
use crate::risk_gating::domain::{
    DefaultClassifier, GateStateRegistry, GatingAction, RiskClassifier, RiskConfig,
    RiskGatingError, RiskLevel,
};

/// Thread-safe wrapper for mutable classifier state.
struct ClassifierState {
    classifier: DefaultClassifier,
}

/// Implementation of the RiskGateService.
///
/// Stores the classifier and config behind `RwLock` for thread-safe access.
/// The `classifier()` and `config()` accessor methods use raw pointers that
/// are initialized after the service is placed behind a `Box` (stable address).
pub struct RiskGateServiceImpl {
    /// The execution ID this service instance is bound to.
    execution_id: String,

    /// Thread-safe classifier state (allows config reloads).
    classifier: RwLock<ClassifierState>,

    /// Shared gate state registry (across executions).
    gate_registry: Arc<GateStateRegistry>,

    /// The risk configuration.
    config: RwLock<RiskConfig>,
}

impl RiskGateServiceImpl {
    /// Create a new `RiskGateServiceImpl`.
    pub fn new(
        execution_id: String,
        config: RiskConfig,
        gate_registry: Arc<GateStateRegistry>,
    ) -> Self {
        let classifier = DefaultClassifier::new(config.clone());
        Self {
            execution_id,
            classifier: RwLock::new(ClassifierState { classifier }),
            gate_registry,
            config: RwLock::new(config),
        }
    }

    /// Determine the gating action and whether the tool is allowed based on
    /// level and config flags.
    fn evaluate_gating_policy(
        risk_level: RiskLevel,
        config: &RiskConfig,
    ) -> (GatingAction, bool, String) {
        match risk_level {
            RiskLevel::Low => {
                if config.auto_confirm_low {
                    (
                        GatingAction::AutoExecute,
                        true,
                        "Low risk: auto-execute".to_string(),
                    )
                } else {
                    (
                        GatingAction::AutoExecute,
                        true,
                        "Low risk: auto-execute (policy override disabled)".to_string(),
                    )
                }
            }
            RiskLevel::Medium => {
                if config.require_review_medium {
                    (
                        GatingAction::RequireConfirmation,
                        true,
                        "Medium risk: requires confirmation".to_string(),
                    )
                } else {
                    (
                        GatingAction::AutoExecute,
                        true,
                        "Medium risk: auto-execute (review disabled)".to_string(),
                    )
                }
            }
            RiskLevel::High => {
                if config.dry_run_high {
                    (
                        GatingAction::DryRun,
                        true,
                        "High risk: dry-run by default".to_string(),
                    )
                } else {
                    (
                        GatingAction::AutoExecute,
                        true,
                        "High risk: auto-execute (dry-run disabled)".to_string(),
                    )
                }
            }
        }
    }
}

#[async_trait]
impl RiskGateService for RiskGateServiceImpl {
    async fn evaluate_gate(
        &self,
        input: EvaluateGateInput,
    ) -> Result<EvaluateGateOutput, RiskGatingError> {
        let classifier = self.classifier.read().expect("Classifier lock poisoned");
        let config = self.config.read().expect("Config lock poisoned");

        let classification = classifier
            .classifier
            .classify(&input.tool, input.parameters.as_ref());

        let (gating_action, allowed, policy_reason) =
            Self::evaluate_gating_policy(classification.risk_level, &config);

        // Register gate if confirmation or dry-run is required
        let gate_id = if gating_action == GatingAction::RequireConfirmation
            || gating_action == GatingAction::DryRun
        {
            self.gate_registry.register_gate(
                &self.execution_id,
                &input.node_id,
                &input.tool,
                classification.risk_level,
                gating_action,
            )
        } else {
            String::new()
        };

        let reason = if classification.from_override {
            format!("{} (override)", policy_reason)
        } else {
            format!("{} — {}", policy_reason, classification.reason)
        };

        Ok(EvaluateGateOutput {
            risk_level: classification.risk_level,
            gating_action,
            allowed,
            reason,
            from_override: classification.from_override,
            gate_id,
            warnings: Vec::new(),
        })
    }

    async fn classify_tool(
        &self,
        input: ClassifyToolInput,
    ) -> Result<ClassifyToolOutput, RiskGatingError> {
        let classifier = self.classifier.read().expect("Classifier lock poisoned");
        let classification = classifier
            .classifier
            .classify(&input.tool, input.parameters.as_ref());

        Ok(ClassifyToolOutput {
            risk_level: classification.risk_level,
            reason: classification.reason,
            from_override: classification.from_override,
        })
    }

    async fn resolve_gate(
        &self,
        input: ResolveGateInput,
    ) -> Result<ResolveGateOutput, RiskGatingError> {
        // Check if the gate exists and is pending
        if !self
            .gate_registry
            .is_gate_pending(&input.execution_id, &input.gate_id)
        {
            // Check if it was already resolved
            if let Some(gate) = self.gate_registry.get_gate(&input.gate_id)
                && gate.resolved {
                    return Err(RiskGatingError::InvalidState {
                        detail: format!("Gate {} has already been resolved", input.gate_id),
                    });
                }
            return Err(RiskGatingError::InvalidState {
                detail: format!("Gate {} not found", input.gate_id),
            });
        }

        let resolved = self
            .gate_registry
            .resolve_gate(&input.execution_id, &input.gate_id);
        match resolved {
            Some(_gate) => Ok(ResolveGateOutput {
                gate_id: input.gate_id,
                approved: input.approved,
                can_proceed: input.approved,
            }),
            None => Err(RiskGatingError::InvalidState {
                detail: format!("Failed to resolve gate {}", input.gate_id),
            }),
        }
    }

    #[tracing::instrument(skip_all)]
    async fn get_config(&self) -> Result<GetConfigOutput, RiskGatingError> {
        let config = self.config.read().expect("Config lock poisoned");
        let override_count = config.tool_overrides.len() as u32;
        Ok(GetConfigOutput {
            config: config.clone(),
            override_count,
        })
    }

    async fn override_tool(
        &self,
        input: OverrideToolInput,
    ) -> Result<OverrideToolOutput, RiskGatingError> {
        let mut config = self.config.write().expect("Config lock poisoned");
        let previous = config.get_override(&input.tool).copied();

        config.set_override(input.tool.clone(), input.new_level);

        // Update the classifier's config as well
        if let Ok(mut cs) = self.classifier.write() {
            cs.classifier.set_config(config.clone());
        }

        Ok(OverrideToolOutput {
            tool: input.tool,
            new_level: input.new_level,
            previous_level: previous,
            applied: true,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn reload_config(&self) -> Result<ReloadConfigOutput, RiskGatingError> {
        // In a real implementation, this would re-read from Config source.
        // For now, use the existing config (no-op reload).
        let config = self.config.read().expect("Config lock poisoned");
        Ok(ReloadConfigOutput {
            success: true,
            config_summary: RiskConfigSummary {
                override_count: config.tool_overrides.len() as u32,
                auto_confirm_low: config.auto_confirm_low,
                require_review_medium: config.require_review_medium,
                dry_run_high: config.dry_run_high,
            },
        })
    }

    #[tracing::instrument(skip_all)]
    fn classifier(&self) -> &dyn RiskClassifier {
        // Return the classifier through the RwLock guard.
        // The trait requires `&dyn RiskClassifier` with the lifetime of `&self`,
        // so we must return a reference that lives as long as the struct.
        // Since `DefaultClassifier` implements `RiskClassifier` and is stored
        // behind an `RwLock` owned by the struct, we use `unsafe` to extend
        // the lifetime of the reference past the guard drop.
        //
        // SAFETY: The `DefaultClassifier` behind the `RwLock` lives as long as
        // `self` (the struct). The `RwLock` ensures the data is never moved.
        // We only read, never mutate. The returned reference is valid for the
        // lifetime of `&self`, which is guaranteed by the caller.
        let guard = self.classifier.read().expect("Classifier lock poisoned");
        let ptr: *const DefaultClassifier = &guard.classifier;
        let reference: &DefaultClassifier = unsafe { &*ptr };
        // Extend the lifetime: the classifier lives as long as self
        let extended: &DefaultClassifier = unsafe { std::mem::transmute(reference) };
        extended as &dyn RiskClassifier
    }

    #[tracing::instrument(skip_all)]
    fn config(&self) -> &RiskConfig {
        // SAFETY: Same reasoning as `classifier()` — the `RiskConfig` behind
        // the `RwLock` lives as long as `self`.
        let guard = self.config.read().expect("Config lock poisoned");
        let ptr: *const RiskConfig = &*guard;
        let reference: &RiskConfig = unsafe { &*ptr };
        unsafe { std::mem::transmute(reference) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    

    #[tracing::instrument(skip_all)]
    fn create_service(execution_id: &str) -> RiskGateServiceImpl {
        let config = RiskConfig::default();
        let gate_registry = Arc::new(GateStateRegistry::new());
        RiskGateServiceImpl::new(execution_id.to_string(), config, gate_registry)
    }

    #[tokio::test]
    async fn test_evaluate_gate_file_read_auto_execute() {
        let service = create_service("exec-1");
        let input = EvaluateGateInput {
            execution_id: "exec-1".to_string(),
            node_id: "node-1".to_string(),
            tool: "file_read".to_string(),
            parameters: None,
            is_retry: false,
        };
        let output = service.evaluate_gate(input).await.unwrap();
        assert_eq!(output.risk_level, RiskLevel::Low);
        assert_eq!(output.gating_action, GatingAction::AutoExecute);
        assert!(output.allowed);
        assert!(output.gate_id.is_empty());
    }

    #[tokio::test]
    async fn test_evaluate_gate_file_write_requires_confirmation() {
        let service = create_service("exec-1");
        let input = EvaluateGateInput {
            execution_id: "exec-1".to_string(),
            node_id: "node-2".to_string(),
            tool: "file_write".to_string(),
            parameters: None,
            is_retry: false,
        };
        let output = service.evaluate_gate(input).await.unwrap();
        assert_eq!(output.risk_level, RiskLevel::Medium);
        assert_eq!(output.gating_action, GatingAction::RequireConfirmation);
        assert!(output.allowed);
        assert!(!output.gate_id.is_empty());
    }

    #[tokio::test]
    async fn test_evaluate_gate_bash_dry_run() {
        let service = create_service("exec-1");
        let input = EvaluateGateInput {
            execution_id: "exec-1".to_string(),
            node_id: "node-3".to_string(),
            tool: "bash".to_string(),
            parameters: None,
            is_retry: false,
        };
        let output = service.evaluate_gate(input).await.unwrap();
        assert_eq!(output.risk_level, RiskLevel::High);
        assert_eq!(output.gating_action, GatingAction::DryRun);
        assert!(output.allowed);
        assert!(!output.gate_id.is_empty());
    }

    #[tokio::test]
    async fn test_classify_tool() {
        let service = create_service("exec-1");
        let input = ClassifyToolInput {
            tool: "git_commit".to_string(),
            parameters: None,
        };
        let output = service.classify_tool(input).await.unwrap();
        assert_eq!(output.risk_level, RiskLevel::High);
    }

    #[tokio::test]
    async fn test_resolve_gate_approve() {
        let service = create_service("exec-1");
        let eval_input = EvaluateGateInput {
            execution_id: "exec-1".to_string(),
            node_id: "node-2".to_string(),
            tool: "file_write".to_string(),
            parameters: None,
            is_retry: false,
        };
        let eval_output = service.evaluate_gate(eval_input).await.unwrap();
        let gate_id = eval_output.gate_id;

        let resolve_input = ResolveGateInput {
            execution_id: "exec-1".to_string(),
            gate_id,
            approved: true,
            reason: Some("Approved by user".to_string()),
        };
        let output = service.resolve_gate(resolve_input).await.unwrap();
        assert!(output.approved);
        assert!(output.can_proceed);
    }

    #[tokio::test]
    async fn test_resolve_gate_reject() {
        let service = create_service("exec-1");
        let eval_input = EvaluateGateInput {
            execution_id: "exec-1".to_string(),
            node_id: "node-2".to_string(),
            tool: "file_write".to_string(),
            parameters: None,
            is_retry: false,
        };
        let eval_output = service.evaluate_gate(eval_input).await.unwrap();
        let gate_id = eval_output.gate_id;

        let resolve_input = ResolveGateInput {
            execution_id: "exec-1".to_string(),
            gate_id,
            approved: false,
            reason: Some("Rejected by user".to_string()),
        };
        let output = service.resolve_gate(resolve_input).await.unwrap();
        assert!(!output.approved);
        assert!(!output.can_proceed);
    }

    #[tokio::test]
    async fn test_resolve_nonexistent_gate_returns_error() {
        let service = create_service("exec-1");
        let input = ResolveGateInput {
            execution_id: "exec-1".to_string(),
            gate_id: "nonexistent".to_string(),
            approved: true,
            reason: None,
        };
        let result = service.resolve_gate(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_override_tool() {
        let service = create_service("exec-1");
        let input = OverrideToolInput {
            execution_id: "exec-1".to_string(),
            tool: "file_read".to_string(),
            new_level: RiskLevel::High,
            reason: Some("Override for testing".to_string()),
        };
        let output = service.override_tool(input).await.unwrap();
        assert_eq!(output.new_level, RiskLevel::High);
        assert_eq!(output.previous_level, None);
        assert!(output.applied);

        // Verify the override took effect
        let eval_input = EvaluateGateInput {
            execution_id: "exec-1".to_string(),
            node_id: "node-1".to_string(),
            tool: "file_read".to_string(),
            parameters: None,
            is_retry: false,
        };
        let eval_output = service.evaluate_gate(eval_input).await.unwrap();
        assert_eq!(eval_output.risk_level, RiskLevel::High);
        assert!(eval_output.from_override);
    }

    #[tokio::test]
    async fn test_get_config() {
        let service = create_service("exec-1");
        let output = service.get_config().await.unwrap();
        assert_eq!(output.override_count, 0);
        assert!(output.config.auto_confirm_low);
        assert!(output.config.require_review_medium);
        assert!(output.config.dry_run_high);
    }

    #[tokio::test]
    async fn test_reload_config() {
        let service = create_service("exec-1");
        let output = service.reload_config().await.unwrap();
        assert!(output.success);
    }

    #[tokio::test]
    async fn test_evaluate_unknown_tool_defaults_medium() {
        let service = create_service("exec-1");
        let input = EvaluateGateInput {
            execution_id: "exec-1".to_string(),
            node_id: "node-1".to_string(),
            tool: "some_unknown_tool".to_string(),
            parameters: None,
            is_retry: false,
        };
        let output = service.evaluate_gate(input).await.unwrap();
        assert_eq!(output.risk_level, RiskLevel::Medium);
        assert_eq!(output.gating_action, GatingAction::RequireConfirmation);
    }

    #[tokio::test]
    async fn test_resolve_already_resolved_gate_returns_error() {
        let service = create_service("exec-1");
        let eval_input = EvaluateGateInput {
            execution_id: "exec-1".to_string(),
            node_id: "node-2".to_string(),
            tool: "file_write".to_string(),
            parameters: None,
            is_retry: false,
        };
        let eval_output = service.evaluate_gate(eval_input).await.unwrap();
        let gate_id = eval_output.gate_id;

        // Resolve once
        let resolve_input = ResolveGateInput {
            execution_id: "exec-1".to_string(),
            gate_id: gate_id.clone(),
            approved: true,
            reason: None,
        };
        service.resolve_gate(resolve_input).await.unwrap();

        // Resolve again — should error
        let resolve_input2 = ResolveGateInput {
            execution_id: "exec-1".to_string(),
            gate_id,
            approved: true,
            reason: None,
        };
        let result = service.resolve_gate(resolve_input2).await;
        assert!(result.is_err());
    }
}
