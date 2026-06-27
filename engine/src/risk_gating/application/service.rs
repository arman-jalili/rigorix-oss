//! Service interfaces (use cases) for the Risk Gating bounded context.
//!
//! @canonical .pi/architecture/modules/risk-gating.md
//! Implements: Contract Freeze — RiskGateService trait
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for risk gating:
//! tool classification, gating evaluation, configuration management,
//! and override handling. All methods are async and return domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::risk_gating::domain::{RiskClassifier, RiskConfig, RiskGatingError};

use super::dto::{
    ClassifyToolInput, ClassifyToolOutput, EvaluateGateInput, EvaluateGateOutput, GetConfigOutput,
    OverrideToolInput, OverrideToolOutput, ReloadConfigOutput, ResolveGateInput, ResolveGateOutput,
};

/// Central risk gate service that classifies tools and evaluates gating policies.
///
/// The RiskGateService sits between the execution engine and the tool
/// invocation. Every tool call passes through the risk gate, which:
///
/// 1. Classifies the tool into a risk level (via RiskClassifier)
/// 2. Evaluates the gating policy based on RiskConfig
/// 3. Returns the gating decision (auto-execute, confirm, or dry-run)
///
/// # Integration
///
/// The service cooperates with the Enforcement module:
/// - The gating decision feeds into the ExecutionEnforcer's tool evaluation
/// - Budget limits are checked after the gate is resolved
/// - Events are emitted for audit trail tracking
#[async_trait]
pub trait RiskGateService: Send + Sync {
    /// Classify a tool and evaluate the gating policy in one operation.
    ///
    /// This is the main entry point for the execution engine. It:
    /// 1. Classifies the tool using the RiskClassifier
    /// 2. Applies the gating policy from RiskConfig
    /// 3. Returns the complete gating decision
    ///
    /// Returns `RiskGatingError::UnknownTool` if no rule or override exists.
    async fn evaluate_gate(
        &self,
        input: EvaluateGateInput,
    ) -> Result<EvaluateGateOutput, RiskGatingError>;

    /// Classify a tool without evaluating the gate.
    ///
    /// Useful for informational queries (e.g., UI display) or when
    /// the caller wants to apply its own gating logic.
    async fn classify_tool(
        &self,
        input: ClassifyToolInput,
    ) -> Result<ClassifyToolOutput, RiskGatingError>;

    /// Resolve a pending gate (approve or reject).
    ///
    /// Called when a user responds to a confirmation request or
    /// explicitly approves a dry-run execution.
    async fn resolve_gate(
        &self,
        input: ResolveGateInput,
    ) -> Result<ResolveGateOutput, RiskGatingError>;

    /// Get the current risk configuration.
    ///
    /// Returns the active RiskConfig including all tool overrides
    /// and gating policy flags.
    async fn get_config(&self) -> Result<GetConfigOutput, RiskGatingError>;

    /// Override the risk level for a specific tool at runtime.
    ///
    /// Updates the `RiskConfig.tool_overrides` map. The override
    /// takes effect immediately for subsequent classifications.
    async fn override_tool(
        &self,
        input: OverrideToolInput,
    ) -> Result<OverrideToolOutput, RiskGatingError>;

    /// Reload risk configuration from the source.
    ///
    /// Re-reads the RiskConfig from the configuration source
    /// (e.g., file, environment) and applies any changes.
    async fn reload_config(&self) -> Result<ReloadConfigOutput, RiskGatingError>;

    /// Get a clone of the underlying RiskClassifier.
    ///
    /// Provides access to the classifier for direct queries
    /// (e.g., listing all classification rules).
    fn classifier(&self) -> Box<dyn RiskClassifier>;

    /// Get a clone of the underlying RiskConfig.
    fn config(&self) -> RiskConfig;
}
