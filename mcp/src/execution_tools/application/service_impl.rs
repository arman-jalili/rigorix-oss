//! Concrete implementations of Execution Tools service traits.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#services
//! Implements: ExecuteHandler, ValidatePlanHandler, CheckEnforcementHandler
//!
//! These are the concrete implementations that wire EngineFacade with
//! application logic for execute, validate, and check enforcement use cases.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

use crate::execution_tools::application::dto::{ExecuteInput, ValidateInput};
use crate::execution_tools::application::service::{
    CheckEnforcementHandler, ExecuteHandler, PlanHandler, ValidatePlanHandler,
};
use crate::execution_tools::domain::entity::SharedEngineFacade;
use crate::execution_tools::domain::error::{HandlerError, ToolCallResult};

/// Implementation of ExecuteHandler.
pub struct ExecuteHandlerImpl {
    engine: SharedEngineFacade,
    timeout_duration: Duration,
}

impl ExecuteHandlerImpl {
    /// Create a new ExecuteHandlerImpl.
    pub fn new(engine: SharedEngineFacade, timeout_duration: Duration) -> Self {
        Self {
            engine,
            timeout_duration,
        }
    }
}

#[async_trait]
impl ExecuteHandler for ExecuteHandlerImpl {
    async fn handle(&self, input: ExecuteInput) -> Result<ToolCallResult, HandlerError> {
        let plan = input.plan.ok_or_else(|| {
            HandlerError::InvalidArguments(
                "Either 'plan' or 'template_name' must be provided".into(),
            )
        })?;

        // Execute the plan through EngineFacade
        let result = self
            .engine
            .execute_with_receipt(
                plan,
                input.repository.clone(),
                input.author.clone(),
                input.identity.clone(),
                input.identity_verification_failure.clone(),
                input
                    .receipt
                    .clone()
                    .map(crate::execution_tools::domain::receipt_input::ReceiptInput::new),
            )
            .await
            .map_err(HandlerError::EngineError)?;

        // Format as MCP tool call content
        let content = serde_json::json!({
            "execution_id": result.execution_id(),
            "status": format!("{:?}", result.status()),
            "duration_ms": result.duration_ms(),
            "tokens_used": result.tokens_used(),
            "audit_uri": result.audit_uri(),
            // #1491: `evidence` is a fact about the run, not a debug field.
            "evidence": result.evidence(),
            // #1630: the declaration's digest, the observation's, and which of the three "no digest"
            // states this response is in.
            "receipt_digest": result.receipt_digest(),
            "attestation_digest": result.attestation_digest(),
            "receipt_state": result.receipt_state(),
            // #1657: the step's **output** is returned, bounded. It was serialized into the DTO
            // and then dropped here, so `file_read` reported `Completed` with no content over both
            // MCP and HTTP — a success the caller could not use and could not tell from an empty
            // file.
            "steps": result.steps().iter().map(|s| {
                let rendered = match s.output() {
                    serde_json::Value::Null => None,
                    serde_json::Value::String(text) => Some(text.clone()),
                    other => Some(other.to_string()),
                };
                let (output, truncated) = bounded_output(rendered.as_deref());
                serde_json::json!({
                    "step_name": s.step_name(),
                    "success": s.is_success(),
                    "error": s.error(),
                    "duration_ms": s.duration_ms(),
                    "output": output,
                    // A truncated output is not a complete one, and the caller must be able to see
                    // which it has without measuring the string.
                    "output_truncated": truncated,
                })
            }).collect::<Vec<_>>(),
        });

        Ok(ToolCallResult {
            content: vec![crate::execution_tools::domain::error::ToolContentItem {
                r#type: "json".into(),
                text: serde_json::to_string_pretty(&content).unwrap_or_else(|_| "{}".to_string()),
            }],
            is_error: false,
        })
    }

    fn timeout_duration(&self) -> Duration {
        self.timeout_duration
    }
}

/// Implementation of ValidatePlanHandler.
pub struct ValidatePlanHandlerImpl {
    engine: SharedEngineFacade,
}

impl ValidatePlanHandlerImpl {
    /// Create a new ValidatePlanHandlerImpl.
    pub fn new(engine: SharedEngineFacade) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl ValidatePlanHandler for ValidatePlanHandlerImpl {
    async fn handle(&self, input: ValidateInput) -> Result<ToolCallResult, HandlerError> {
        let result = self
            .engine
            .validate_plan(
                input.plan.clone(),
                input.identity.clone(),
                input.identity_verification_failure.clone(),
            )
            .await
            .map_err(HandlerError::EngineError)?;

        // #1657: the one call whose **name is "validate"** could not see the failure a reader most
        // needs — `tool: "definitely_not_a_tool"` returned `valid: true, errors: []`, because tool
        // names and parameters were checked only at dispatch. The tool set now has one home
        // (`rigorix_engine::...tools::DISPATCHABLE_TOOL_NAMES`), and every unknown name is reported
        // **at once** with the closest known ones, instead of one per round trip.
        let mut unknown_tools: Vec<serde_json::Value> = Vec::new();
        for step in input.plan.steps() {
            if step.tool().is_empty() || claim_is_dispatchable(step.tool()) {
                continue;
            }
            unknown_tools.push(serde_json::json!({
                "step": step.name(),
                "tool": step.tool(),
                "closest_known": closest_tool_names(step.tool()),
            }));
        }
        let mut errors: Vec<String> = result.errors().iter().map(|e| e.to_string()).collect();
        for unknown in &unknown_tools {
            errors.push(format!(
                "unknown tool `{}` in step `{}` — the dispatcher knows {} (closest: {})",
                unknown["tool"].as_str().unwrap_or_default(),
                unknown["step"].as_str().unwrap_or_default(),
                rigorix_engine::execution_engine::application::service_impl::DISPATCHABLE_TOOL_NAMES
                    .len(),
                unknown["closest_known"]
                    .as_array()
                    .map(|names| names
                        .iter()
                        .filter_map(|n| n.as_str())
                        .collect::<Vec<_>>()
                        .join(", "))
                    .unwrap_or_default(),
            ));
        }

        let mut content = serde_json::json!({
            // The verdict includes the tool check: a caller that reads only `valid` must not be told
            // a plan is fine when one of its steps names nothing the dispatcher can run.
            "valid": result.is_valid() && unknown_tools.is_empty(),
            "unknown_tools": unknown_tools,
            "warnings": result.warnings(),
            "errors": errors,
            "estimated_cost": result.estimated_cost().map(|c| {
                serde_json::json!({
                    "estimated_tokens": c.estimated_tokens,
                    "estimated_tool_calls": c.estimated_tool_calls,
                })
            }),
        });

        // Structured sequence-policy findings (R2 plan-time): machine-readable
        // `{rule_id, later_step, action}` entries so a matched sequence is
        // visible to the agent BEFORE a run (module sequence-policy AC#8).
        let findings = result.findings();
        if !findings.is_empty() {
            content["sequence_findings"] = serde_json::to_value(findings).unwrap_or_default();
        }

        // Structured R9 operator step-requirement findings (ADR-015):
        // machine-readable `{requirement_id, step, action, unmet}` entries so
        // a matched operator obligation is visible to the agent BEFORE a run.
        // Only pointer NAMES are surfaced — never parameter values.
        let requirement_findings = result.requirement_findings();
        if !requirement_findings.is_empty() {
            content["requirement_findings"] =
                serde_json::to_value(requirement_findings).unwrap_or_default();
        }

        Ok(ToolCallResult {
            content: vec![crate::execution_tools::domain::error::ToolContentItem {
                r#type: "json".into(),
                text: serde_json::to_string_pretty(&content).unwrap_or_else(|_| "{}".to_string()),
            }],
            is_error: !result.is_valid(),
        })
    }
}

/// Implementation of CheckEnforcementHandler.
pub struct CheckEnforcementHandlerImpl {
    engine: SharedEngineFacade,
}

impl CheckEnforcementHandlerImpl {
    /// Create a new CheckEnforcementHandlerImpl.
    pub fn new(engine: SharedEngineFacade) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl CheckEnforcementHandler for CheckEnforcementHandlerImpl {
    async fn handle(&self) -> Result<ToolCallResult, HandlerError> {
        let status = self
            .engine
            .check_enforcement()
            .await
            .map_err(HandlerError::EngineError)?;

        let content = serde_json::json!({
            "active": status.is_active(),
            "preset": status.preset(),
            "budget": {
                "tool_calls_total": status.budget().tool_calls_total,
                "tool_calls_remaining": status.budget().tool_calls_remaining,
                "tokens_total": status.budget().tokens_total,
                "tokens_remaining": status.budget().tokens_remaining,
            },
            "circuit_breakers": status.circuit_breakers().iter().map(|cb| {
                serde_json::json!({
                    "name": cb.name,
                    "is_tripped": cb.is_tripped,
                    "description": cb.description,
                })
            }).collect::<Vec<_>>(),
        });

        Ok(ToolCallResult {
            content: vec![crate::execution_tools::domain::error::ToolContentItem {
                r#type: "json".into(),
                text: serde_json::to_string_pretty(&content).unwrap_or_else(|_| "{}".to_string()),
            }],
            is_error: false,
        })
    }
}

/// Implementation of PlanHandler.
pub struct PlanHandlerImpl {
    engine: SharedEngineFacade,
}

impl PlanHandlerImpl {
    /// Create a new PlanHandlerImpl.
    pub fn new(engine: SharedEngineFacade) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl PlanHandler for PlanHandlerImpl {
    async fn handle(
        &self,
        template: &crate::template_tools::domain::value::PlanTemplate,
    ) -> Result<ToolCallResult, HandlerError> {
        // 1. Convert template_tools::PlanTemplate → execution_tools::PlanTemplate via JSON
        let json = serde_json::to_value(template).unwrap_or_default();
        let exec_plan: crate::execution_tools::domain::value::PlanTemplate =
            serde_json::from_value(json).map_err(|e| {
                HandlerError::InvalidArguments(format!("Failed to convert template: {}", e))
            })?;

        // 2. Validate against enforcement policies (plan preview path — no
        // session identity available here; require_identity steps are gated
        // on the rigorix_run / rigorix_validate_plan tool surface where the
        // attested session identity is injected).
        let validation = self
            .engine
            .validate_plan(exec_plan.clone(), None, None)
            .await
            .map_err(HandlerError::EngineError)?;

        // 3. Build graph nodes from exec_plan steps
        let mut nodes = Vec::new();
        for step in exec_plan.steps() {
            nodes.push(crate::execution_tools::application::dto::GraphNodeInfo {
                name: step.name().to_string(),
                tool: step.tool().to_string(),
                description: step.description().to_string(),
                dependencies: vec![], // current engine: flat, no cross-node deps
            });
        }

        // 4. Build constraints DTO
        let constraints = exec_plan.constraints().map(|c| {
            crate::execution_tools::application::dto::ConstraintsDto {
                max_tool_calls: c.max_tool_calls,
                max_tokens: c.max_tokens,
                max_duration_secs: c.max_duration_secs,
            }
        });

        // 5. Build PlanOutput
        let output = crate::execution_tools::application::dto::PlanOutput {
            template_name: template.name().to_string(),
            description: template.description().to_string(),
            version: template.version().to_string(),
            tags: template.tags().to_vec(),
            graph: crate::execution_tools::application::dto::GraphInfo {
                sealed: true,
                node_count: nodes.len(),
                nodes,
            },
            constraints,
            enforcement: crate::execution_tools::application::dto::PlanEnforcementInfo {
                valid: validation.is_valid(),
                warnings: validation.warnings().to_vec(),
                errors: validation.errors().to_vec(),
                estimated_cost: validation.estimated_cost().map(|c| {
                    crate::execution_tools::application::dto::CostEstimateDto {
                        estimated_tokens: c.estimated_tokens,
                        estimated_tool_calls: c.estimated_tool_calls,
                        estimated_cost_micro: c.estimated_cost_micro,
                    }
                }),
            },
        };

        let content = serde_json::json!(output);

        Ok(ToolCallResult {
            content: vec![crate::execution_tools::domain::error::ToolContentItem {
                r#type: "json".into(),
                text: serde_json::to_string_pretty(&content).unwrap_or_else(|_| "{}".to_string()),
            }],
            is_error: !output.enforcement.valid,
        })
    }
}

// ---------------------------------------------------------------------------
// Factory function
// ---------------------------------------------------------------------------

/// Create all handler instances from shared dependencies.
pub fn create_handler_instances(
    engine: SharedEngineFacade,
    execute_timeout: Duration,
) -> HandlerInstanceSet {
    HandlerInstanceSet {
        execute: Arc::new(ExecuteHandlerImpl::new(engine.clone(), execute_timeout)),
        validate: Arc::new(ValidatePlanHandlerImpl::new(engine.clone())),
        check_enforcement: Arc::new(CheckEnforcementHandlerImpl::new(engine.clone())),
        plan: Arc::new(PlanHandlerImpl::new(engine)),
    }
}

/// Set of all handler instances.
pub struct HandlerInstanceSet {
    /// Execute handler.
    pub execute: Arc<dyn ExecuteHandler>,
    /// Validate plan handler.
    pub validate: Arc<dyn ValidatePlanHandler>,
    /// Check enforcement handler.
    pub check_enforcement: Arc<dyn CheckEnforcementHandler>,
    /// Plan handler.
    pub plan: Arc<dyn PlanHandler>,
}

/// The cap on a step's output in a tool result (#1657).
///
/// 32 KiB: enough for a file read, a diff or a test log tail, small enough that ten steps cannot blow
/// a context window. The truncation is **stated** in the response (`output_truncated`) because a
/// silently shortened output is a result the caller cannot distinguish from a short one.
pub const MAX_STEP_OUTPUT_BYTES: usize = 32 * 1024;

/// Bound one step's output, returning the text and whether it was cut.
fn bounded_output(output: Option<&str>) -> (Option<String>, bool) {
    match output {
        None => (None, false),
        Some(text) if text.len() <= MAX_STEP_OUTPUT_BYTES => (Some(text.to_string()), false),
        Some(text) => {
            let mut end = MAX_STEP_OUTPUT_BYTES;
            // Never split a UTF-8 sequence: walking back to a char boundary is cheaper than handing a
            // client an invalid string that its JSON parser will reject for an unrelated reason.
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            (
                Some(format!(
                    "{}\n\n[truncated: {} of {} bytes shown — this is not the complete output]",
                    &text[..end],
                    end,
                    text.len()
                )),
                true,
            )
        }
    }
}

/// Whether a step's tool names something the dispatcher can run (#1657).
///
/// A `ToolCall` node names a **registry** tool, and the registry is a runtime composition, so a name
/// this crate cannot see is not automatically wrong. The dispatcher's own list covers the nine step
/// kinds; anything else is reported as unknown, which is the direction a validator must err in.
fn claim_is_dispatchable(tool: &str) -> bool {
    rigorix_engine::execution_engine::application::service_impl::DISPATCHABLE_TOOL_NAMES
        .contains(&tool)
}

/// The known tool names closest to `tool`, so the refusal helps (#1657).
fn closest_tool_names(tool: &str) -> Vec<String> {
    let lowercase = tool.to_ascii_lowercase();
    let mut scored: Vec<(usize, &str)> =
        rigorix_engine::execution_engine::application::service_impl::DISPATCHABLE_TOOL_NAMES
            .iter()
            .map(|known| (edit_distance(&lowercase, known), *known))
            .collect();
    scored.sort_by_key(|(distance, name)| (*distance, *name));
    scored
        .into_iter()
        .take(3)
        .map(|(_, name)| name.to_string())
        .collect()
}

/// Levenshtein distance, for "did you mean" (#1657). Two rows of memory, no dependency.
fn edit_distance(left: &str, right: &str) -> usize {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0usize; right.len() + 1];
    for (i, a) in left.iter().enumerate() {
        current[0] = i + 1;
        for (j, b) in right.iter().enumerate() {
            let cost = usize::from(a != b);
            current[j + 1] = (previous[j + 1] + 1)
                .min(current[j] + 1)
                .min(previous[j] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}
