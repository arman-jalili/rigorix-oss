//! Value objects for the Execution Tools bounded context.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#value-objects
//! Implements: Contract Freeze — PlanTemplate, StepDefinition, ExecutionResult,
//! ValidationResult, EnforcementStatus, CostBreakdown, BudgetStatus, CircuitBreakerStatus
//!
//! Value objects are immutable, interchangeable, and defined by their attributes,
//! not identity. They carry validation in their constructors and are serializable
//! for API transmission.
//!
//! # Contract (Frozen)
//!
//! - All value objects are immutable (no pub fields, no setters)
//! - All value objects implement PartialEq + Eq + Hash based on ALL fields
//! - Constructors validate invariants — return Result<_, Error> on failure
//! - All types derive Serialize + Deserialize for JSON transmission
//! - No behavior beyond field accessors and validation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// PlanTemplate — structured plan for execution
// ---------------------------------------------------------------------------

/// A structured plan with steps, constraints, and metadata.
///
/// Shared between execution and template contexts. Represents a plan that
/// can be executed, validated, or checked against enforcement policies.
///
/// # Contract (Frozen)
///
/// - Must have at least one step
/// - Step order is significant (DAG execution order)
/// - Constraints are optional enforcement boundaries
/// - Metadata is opaque key-value storage for extensibility
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanTemplate {
    /// Name identifying the plan.
    name: String,

    /// Human-readable description of the plan's purpose.
    description: String,

    /// Ordered list of steps to execute.
    steps: Vec<StepDefinition>,

    /// Optional enforcement constraints.
    #[serde(default)]
    constraints: Option<Constraints>,

    /// Extensible metadata (e.g., source, session context).
    #[serde(default)]
    metadata: HashMap<String, String>,
}

impl PlanTemplate {
    /// Create a new PlanTemplate with validation.
    ///
    /// # Errors
    /// Returns `PlanTemplateError::EmptySteps` if `steps` is empty.
    pub fn new(
        name: String,
        description: String,
        steps: Vec<StepDefinition>,
        constraints: Option<Constraints>,
        metadata: HashMap<String, String>,
    ) -> Result<Self, PlanTemplateError> {
        if steps.is_empty() {
            return Err(PlanTemplateError::EmptySteps);
        }
        Ok(Self {
            name,
            description,
            steps,
            constraints,
            metadata,
        })
    }

    /// Create a PlanTemplate from a JSON value.
    ///
    /// Deserializes and validates the plan in one step.
    pub fn from_json(value: serde_json::Value) -> Result<Self, PlanTemplateError> {
        let template: Self = serde_json::from_value(value)
            .map_err(|e| PlanTemplateError::DeserializationFailed(e.to_string()))?;
        if template.steps.is_empty() {
            return Err(PlanTemplateError::EmptySteps);
        }
        Ok(template)
    }

    /// Plan name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Plan description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Ordered list of steps.
    pub fn steps(&self) -> &[StepDefinition] {
        &self.steps
    }

    /// Optional enforcement constraints.
    pub fn constraints(&self) -> Option<&Constraints> {
        self.constraints.as_ref()
    }

    /// Extensible metadata.
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }
}

// ---------------------------------------------------------------------------
// StepDefinition — a single step in a plan
// ---------------------------------------------------------------------------

/// A single step in a plan: which tool to call, with what parameters,
/// and whether human approval is required.
///
/// # Contract (Frozen)
///
/// - Tool name must match a registered MCP tool
/// - Parameters are opaque JSON — validated by the target tool
/// - If `requires_approval` is true, execution pauses for human sign-off
/// - Optional timeout overrides the default step timeout
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepDefinition {
    /// Step name (unique within the plan).
    name: String,

    /// MCP tool name to invoke.
    tool: String,

    /// Tool-specific parameters as a JSON object.
    parameters: serde_json::Value,

    /// Whether human approval is required before execution.
    requires_approval: bool,

    /// Human-readable description of the step's purpose.
    description: String,

    /// Optional timeout in seconds for this step.
    #[serde(default)]
    timeout_secs: Option<u64>,
}

impl StepDefinition {
    /// Create a new StepDefinition.
    pub fn new(
        name: String,
        tool: String,
        parameters: serde_json::Value,
        requires_approval: bool,
        description: String,
        timeout_secs: Option<u64>,
    ) -> Self {
        Self {
            name,
            tool,
            parameters,
            requires_approval,
            description,
            timeout_secs,
        }
    }

    /// Step name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// MCP tool name.
    pub fn tool(&self) -> &str {
        &self.tool
    }

    /// Tool-specific parameters.
    pub fn parameters(&self) -> &serde_json::Value {
        &self.parameters
    }

    /// Whether human approval is required.
    pub fn requires_approval(&self) -> bool {
        self.requires_approval
    }

    /// Step description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Optional timeout in seconds.
    pub fn timeout_secs(&self) -> Option<u64> {
        self.timeout_secs
    }
}

// ---------------------------------------------------------------------------
// Constraints — enforcement boundaries for plan execution
// ---------------------------------------------------------------------------

/// Enforcement constraints for plan execution.
///
/// Optional boundaries that the plan must not exceed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Constraints {
    /// Maximum number of tool calls allowed.
    pub max_tool_calls: Option<u64>,

    /// Maximum total tokens allowed.
    pub max_tokens: Option<u64>,

    /// Maximum execution time in seconds.
    pub max_duration_secs: Option<u64>,

    /// List of disallowed tools.
    pub blocked_tools: Vec<String>,

    /// Additional constraint key-value pairs.
    pub extensions: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// ExecutionResult — result of plan execution
// ---------------------------------------------------------------------------

/// Result returned by `rigorix_execute`.
///
/// Contains the execution outcome, per-step results, duration, token usage,
/// and link to the audit trail.
///
/// # Contract (Frozen)
///
/// - `execution_id` is always present (generated by rigorix-engine)
/// - `status` reflects the final execution state
/// - `steps` mirrors plan step order with per-step outcomes
/// - `audit_uri` links to the persistent audit record
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Unique execution identifier.
    execution_id: Uuid,

    /// Overall execution status.
    status: ExecutionStatus,

    /// Per-step results in plan order.
    steps: Vec<StepResult>,

    /// Total execution duration in milliseconds.
    duration_ms: u64,

    /// Optional token usage count.
    #[serde(default)]
    tokens_used: Option<u64>,

    /// URI to the persistent audit record.
    #[serde(default)]
    audit_uri: String,
}

impl ExecutionResult {
    /// Create a new ExecutionResult.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        execution_id: Uuid,
        status: ExecutionStatus,
        steps: Vec<StepResult>,
        duration_ms: u64,
        tokens_used: Option<u64>,
        audit_uri: String,
    ) -> Self {
        Self {
            execution_id,
            status,
            steps,
            duration_ms,
            tokens_used,
            audit_uri,
        }
    }

    /// Execution ID.
    pub fn execution_id(&self) -> &Uuid {
        &self.execution_id
    }

    /// Overall execution status.
    pub fn status(&self) -> &ExecutionStatus {
        &self.status
    }

    /// Per-step results.
    pub fn steps(&self) -> &[StepResult] {
        &self.steps
    }

    /// Duration in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    /// Optional token usage.
    pub fn tokens_used(&self) -> Option<u64> {
        self.tokens_used
    }

    /// Audit record URI.
    pub fn audit_uri(&self) -> &str {
        &self.audit_uri
    }
}

// ---------------------------------------------------------------------------
// ExecutionStatus — execution state machine
// ---------------------------------------------------------------------------

/// Overall execution status for a plan.
///
/// Mirrors the aggregate state transitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// All steps completed successfully.
    Completed,
    /// A step failed with no retry possible.
    Failed,
    /// Some steps failed but partial results are available.
    PartialFailed,
    /// Execution timed out or was cancelled.
    Cancelled,
    /// Execution was blocked by enforcement (budget exceeded).
    EnforcementBlocked,
}

// ---------------------------------------------------------------------------
// StepResult — outcome of a single step
// ---------------------------------------------------------------------------

/// Result of a single step execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepResult {
    /// Step name (matches StepDefinition.name).
    step_name: String,

    /// Whether the step succeeded.
    success: bool,

    /// Optional error message if step failed.
    #[serde(default)]
    error: Option<String>,

    /// Step output data (tool-specific).
    #[serde(default)]
    output: serde_json::Value,

    /// Duration of this step in milliseconds.
    #[serde(default)]
    duration_ms: u64,
}

impl StepResult {
    /// Create a new StepResult.
    pub fn new(
        step_name: String,
        success: bool,
        error: Option<String>,
        output: serde_json::Value,
        duration_ms: u64,
    ) -> Self {
        Self {
            step_name,
            success,
            error,
            output,
            duration_ms,
        }
    }

    /// Step name.
    pub fn step_name(&self) -> &str {
        &self.step_name
    }

    /// Whether the step succeeded.
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// Optional error message.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Step output.
    pub fn output(&self) -> &serde_json::Value {
        &self.output
    }

    /// Duration in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
}

// ---------------------------------------------------------------------------
// ValidationResult — result of plan validation
// ---------------------------------------------------------------------------

/// Result returned by `rigorix_validate_plan`.
///
/// Indicates whether the plan is valid against enforcement policies,
/// with warnings, errors, and optional cost estimates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the plan is valid.
    valid: bool,

    /// Warning messages (non-blocking issues).
    warnings: Vec<String>,

    /// Error messages (blocking issues).
    errors: Vec<String>,

    /// Optional estimated cost of execution.
    #[serde(default)]
    estimated_cost: Option<CostEstimate>,
}

impl ValidationResult {
    /// Create a new ValidationResult.
    pub fn new(
        valid: bool,
        warnings: Vec<String>,
        errors: Vec<String>,
        estimated_cost: Option<CostEstimate>,
    ) -> Self {
        Self {
            valid,
            warnings,
            errors,
            estimated_cost,
        }
    }

    /// Whether the plan is valid.
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Warning messages.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Error messages.
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// Optional cost estimate.
    pub fn estimated_cost(&self) -> Option<&CostEstimate> {
        self.estimated_cost.as_ref()
    }
}

// ---------------------------------------------------------------------------
// CostEstimate — estimated cost of plan execution
// ---------------------------------------------------------------------------

/// Estimated cost for executing a plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostEstimate {
    /// Estimated token count.
    pub estimated_tokens: u64,

    /// Estimated tool calls.
    pub estimated_tool_calls: u64,

    /// Optional monetary estimate (in micro-units).
    pub estimated_cost_micro: Option<u64>,
}

// ---------------------------------------------------------------------------
// EnforcementStatus — current enforcement state
// ---------------------------------------------------------------------------

/// Result returned by `rigorix_check_enforcement`.
///
/// Contains the current enforcement status: whether enforcement is active,
/// which preset is applied, remaining budget, and circuit breaker states.
///
/// # Contract (Frozen)
///
/// - `active` reflects whether any enforcement policy is currently applied
/// - `preset` is the active enforcement preset name
/// - `budget` contains remaining call/token counts
/// - `circuit_breakers` lists any active circuit breaker states
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnforcementStatus {
    /// Whether enforcement is currently active.
    active: bool,

    /// Name of the active enforcement preset.
    preset: String,

    /// Current budget status.
    budget: BudgetStatus,

    /// Active circuit breaker states.
    circuit_breakers: Vec<CircuitBreakerStatus>,
}

impl EnforcementStatus {
    /// Create a new EnforcementStatus.
    pub fn new(
        active: bool,
        preset: String,
        budget: BudgetStatus,
        circuit_breakers: Vec<CircuitBreakerStatus>,
    ) -> Self {
        Self {
            active,
            preset,
            budget,
            circuit_breakers,
        }
    }

    /// Whether enforcement is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Active preset name.
    pub fn preset(&self) -> &str {
        &self.preset
    }

    /// Current budget status.
    pub fn budget(&self) -> &BudgetStatus {
        &self.budget
    }

    /// Active circuit breaker states.
    pub fn circuit_breakers(&self) -> &[CircuitBreakerStatus] {
        &self.circuit_breakers
    }
}

// ---------------------------------------------------------------------------
// BudgetStatus — remaining budget information
// ---------------------------------------------------------------------------

/// Remaining budget for tool calls and tokens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetStatus {
    /// Total tool call limit.
    pub tool_calls_total: u64,

    /// Remaining tool calls.
    pub tool_calls_remaining: u64,

    /// Total token limit.
    pub tokens_total: u64,

    /// Remaining tokens.
    pub tokens_remaining: u64,
}

// ---------------------------------------------------------------------------
// CircuitBreakerStatus — state of a single circuit breaker
// ---------------------------------------------------------------------------

/// State of one enforcement circuit breaker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CircuitBreakerStatus {
    /// Circuit breaker name (e.g., "rate_limit", "budget_threshold").
    pub name: String,

    /// Whether the circuit breaker is currently tripped (open).
    pub is_tripped: bool,

    /// Human-readable description of the breaker state.
    pub description: String,
}

// ---------------------------------------------------------------------------
// CostBreakdown — detailed cost breakdown
// ---------------------------------------------------------------------------

/// Detailed cost breakdown for an execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostBreakdown {
    /// Execution identifier.
    execution_id: Uuid,

    /// Token usage breakdown per step.
    step_costs: Vec<StepCost>,

    /// Total token count consumed.
    total_tokens: u64,

    /// Total tool calls made.
    total_tool_calls: u64,

    /// Optional total monetary cost (in micro-units).
    total_cost_micro: Option<u64>,
}

impl CostBreakdown {
    /// Create a new CostBreakdown.
    pub fn new(
        execution_id: Uuid,
        step_costs: Vec<StepCost>,
        total_tokens: u64,
        total_tool_calls: u64,
        total_cost_micro: Option<u64>,
    ) -> Self {
        Self {
            execution_id,
            step_costs,
            total_tokens,
            total_tool_calls,
            total_cost_micro,
        }
    }

    /// Execution ID.
    pub fn execution_id(&self) -> &Uuid {
        &self.execution_id
    }

    /// Per-step costs.
    pub fn step_costs(&self) -> &[StepCost] {
        &self.step_costs
    }

    /// Total tokens.
    pub fn total_tokens(&self) -> u64 {
        self.total_tokens
    }

    /// Total tool calls.
    pub fn total_tool_calls(&self) -> u64 {
        self.total_tool_calls
    }

    /// Optional total cost.
    pub fn total_cost_micro(&self) -> Option<u64> {
        self.total_cost_micro
    }
}

// ---------------------------------------------------------------------------
// StepCost — cost for a single step
// ---------------------------------------------------------------------------

/// Cost breakdown for one step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepCost {
    /// Step name.
    pub step_name: String,

    /// Tokens consumed by this step.
    pub tokens: u64,

    /// Tool calls made in this step.
    pub tool_calls: u64,

    /// Optional monetary cost (in micro-units).
    pub cost_micro: Option<u64>,
}

// ---------------------------------------------------------------------------
// PlanTemplateError — validation errors for plan construction
// ---------------------------------------------------------------------------

/// Errors that occur during PlanTemplate construction or deserialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum PlanTemplateError {
    /// Plan must contain at least one step.
    #[error("Plan must have at least one step")]
    EmptySteps,

    /// Failed to deserialize plan from JSON.
    #[error("Failed to deserialize plan: {0}")]
    DeserializationFailed(String),
}

// ---------------------------------------------------------------------------
// ExecutionId — strongly-typed identifier for executions
// ---------------------------------------------------------------------------

/// Strongly-typed identifier for a plan execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionId(Uuid);

impl ExecutionId {
    /// Create a new random execution ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create an execution ID from an existing UUID.
    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    /// Return the inner UUID.
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for ExecutionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ExecutionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for ExecutionId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}
