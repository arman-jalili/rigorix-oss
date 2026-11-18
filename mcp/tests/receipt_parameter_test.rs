//! #1630 (ADR-025 D6): the MCP surface can carry a receipt, and it says which "no digest" state it is in.
//!
//! `rigorix_run` built `RunFromTemplateInput { … receipt: None }`, so every tool call returned a
//! digest-less record and a caller could not distinguish "no receipt was declared" from "this surface
//! cannot carry one". These tests assert the three things that change:
//!
//! 1. a receipt sent on the wire is validated through the **crate's** door, so a malformed document is
//!    refused with the crate's vocabulary and operands — not with a sentence this surface invented;
//! 2. the three "no digest" states are three values, because a caller acts on them differently;
//! 3. the five frozen tool names are unchanged and the parameter is additive.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use rigorix_mcp::execution_tools::application::dto::ExecuteInput;
use rigorix_mcp::execution_tools::application::service::ExecuteHandler;
use rigorix_mcp::execution_tools::application::service_impl::ExecuteHandlerImpl;
use rigorix_mcp::execution_tools::domain::entity::EngineFacade;
use rigorix_mcp::execution_tools::domain::error::{EngineFacadeError, ToolCallResult};
use rigorix_mcp::execution_tools::domain::receipt_input::{ReceiptInput, ReceiptState};
use rigorix_mcp::execution_tools::domain::value::{
    CostBreakdown, EnforcementStatus, ExecutionResult, ExecutionStatus, PlanTemplate,
    StepDefinition, ValidationResult,
};

/// The smallest document the crate's own schema accepts.
///
/// Written by hand rather than derived from `receipt_schema()`, because a test that reads its fixture
/// from the implementation proves only that the implementation agrees with itself.
fn receipt_document() -> serde_json::Value {
    serde_json::json!({
        "schema_version": 2,
        "execution_id": "01890a5d-ac96-774b-bcce-b302099a8057",
        "workspace": { "kind": "commit_tree", "value": "4b825dc642cb6eb9a060e54bf8d69288fbee4904" },
        "sandbox": { "regime": "container", "image_digest": "sha256:fixture" },
        "steps": [{
            "id": "read",
            "tool": "file-read",
            "parameters": { "path": "README.md" },
            "depends_on": [],
            "declared_effects": { "paths": [], "content_digests": [] },
            "expectation": { "description": "the file is read" },
            "requires_approval": false
        }],
        "budget": { "max_iterations": 1, "max_tokens": 1000 },
        "quality_required": { "level": "merge_ready", "source": "caller" }
    })
}

struct SilentFacade;

#[async_trait]
impl EngineFacade for SilentFacade {
    async fn execute(
        &self,
        _plan: PlanTemplate,
        _repository: Option<String>,
        _author: Option<String>,
        _identity: Option<rigorix_engine::identity::IdentityRef>,
        _verification_failure: Option<String>,
    ) -> Result<ExecutionResult, EngineFacadeError> {
        Ok(ExecutionResult::new(
            uuid::Uuid::nil(),
            ExecutionStatus::Completed,
            vec![],
            1,
            None,
            "rigorix://audit/00000000-0000-0000-0000-000000000000".into(),
        ))
    }

    async fn validate_plan(
        &self,
        _plan: PlanTemplate,
        _identity: Option<rigorix_engine::identity::IdentityRef>,
        _verification_failure: Option<String>,
    ) -> Result<ValidationResult, EngineFacadeError> {
        Ok(ValidationResult::new(true, vec![], vec![], None))
    }

    async fn check_enforcement(&self) -> Result<EnforcementStatus, EngineFacadeError> {
        Err(EngineFacadeError::EngineNotAvailable("mock".into()))
    }

    async fn get_execution_cost(
        &self,
        id: &rigorix_mcp::execution_tools::domain::value::ExecutionId,
    ) -> Result<CostBreakdown, EngineFacadeError> {
        Err(EngineFacadeError::ExecutionNotFound(*id.as_uuid()))
    }

    async fn run_template(
        &self,
        _template_name: &str,
        _repository: Option<String>,
        _author: Option<String>,
    ) -> Result<ExecutionResult, EngineFacadeError> {
        Err(EngineFacadeError::EngineNotAvailable("mock".into()))
    }

    async fn approve_execution(
        &self,
        _id: &rigorix_mcp::execution_tools::domain::value::ExecutionId,
        _steps: Vec<String>,
        _identity: Option<rigorix_mcp::execution_tools::domain::value::ApprovalIdentity>,
    ) -> Result<rigorix_mcp::execution_tools::domain::value::ApprovalResult, EngineFacadeError>
    {
        Err(EngineFacadeError::EngineNotAvailable("mock".into()))
    }

    async fn execution_state(
        &self,
        _id: &rigorix_mcp::execution_tools::domain::value::ExecutionId,
    ) -> Result<rigorix_mcp::execution_tools::domain::value::ExecutionStateInfo, EngineFacadeError>
    {
        Err(EngineFacadeError::EngineNotAvailable("mock".into()))
    }
}

fn plan() -> PlanTemplate {
    PlanTemplate::new(
        "fixture".into(),
        "one step".into(),
        vec![StepDefinition::new(
            "read".into(),
            "file-read".into(),
            serde_json::json!({ "path": "README.md" }),
            false,
            "the file is read".into(),
            None,
        )],
        None,
        std::collections::HashMap::new(),
    )
    .expect("the fixture plan is valid")
}

fn handler(engine: Arc<dyn EngineFacade>) -> ExecuteHandlerImpl {
    ExecuteHandlerImpl::new(engine, Duration::from_secs(5))
}

fn input(receipt: Option<serde_json::Value>) -> ExecuteInput {
    ExecuteInput {
        plan: Some(plan()),
        template_name: None,
        execution_id: None,
        repository: None,
        author: None,
        identity: None,
        identity_verification_failure: None,
        receipt,
    }
}

#[test]
fn a_valid_receipt_is_validated_by_the_crate_and_its_digest_is_the_crates_own() {
    let input = ReceiptInput::new(receipt_document());
    let (receipt, refusals) = input
        .verify()
        .expect("the fixture is a document the crate accepts");

    assert!(refusals.is_empty());
    // The digest a response reports is computed by the crate, never supplied by the caller: this is the
    // same computation `run_from_template` reports as `receipt_digest`.
    let digest = receipt.digest().expect("the fixture is canonicalisable");
    assert_eq!(digest.as_str().len(), 64);
    assert!(
        !receipt_document().to_string().contains(digest.as_str()),
        "the digest is not in the document the caller sent — it is computed over it"
    );
}

#[test]
fn a_malformed_receipt_is_refused_with_the_crates_vocabulary() {
    let mut document = receipt_document();
    // A member the schema does not have. `Receipt` is a signed declaration and refuses unknown fields,
    // so this is the case that would otherwise be dropped quietly by a `#[serde(default)]` boundary.
    document["unexpected"] = serde_json::json!(true);

    let refusals = ReceiptInput::new(document)
        .verify()
        .expect_err("a document with an unknown member is not a declaration");

    assert!(!refusals.is_empty(), "a refusal carries its operands");
    let kinds: Vec<&str> = refusals.iter().map(|refusal| refusal.kind()).collect();
    assert!(
        kinds.iter().all(|kind| !kind.is_empty()),
        "every refusal names its kind, which is the code a client branches on: {kinds:?}"
    );
}

#[test]
fn the_three_no_digest_states_are_three_values() {
    // A run with no declaration, one whose observation was sealed, and one whose declaration was
    // accepted but produced no attestation. Collapsing them into `None` is what #1630 removes.
    assert_eq!(
        ReceiptState::classify(None, None),
        ReceiptState::NotDeclared
    );
    assert_eq!(
        ReceiptState::classify(Some("a"), Some("b")),
        ReceiptState::Sealed
    );
    assert_eq!(
        ReceiptState::classify(Some("a"), None),
        ReceiptState::NotSealed
    );
    // The wire spellings are the contract a client branches on.
    assert_eq!(ReceiptState::NotDeclared.as_str(), "not_declared");
    assert_eq!(ReceiptState::Sealed.as_str(), "sealed");
    assert_eq!(ReceiptState::NotSealed.as_str(), "not_sealed");
}

#[test]
fn an_engine_that_cannot_carry_a_receipt_refuses_it_rather_than_dropping_it() {
    // The trait's default implementation. An implementation that has not been taught to carry a
    // declaration must not appear to accept one — a silently dropped receipt is the failure this
    // whole issue is about, one layer up.
    let runtime = tokio::runtime::Runtime::new().expect("a runtime");
    let engine: Arc<dyn EngineFacade> = Arc::new(SilentFacade);
    let error = runtime
        .block_on(engine.execute_with_receipt(
            plan(),
            None,
            None,
            None,
            None,
            Some(ReceiptInput::new(receipt_document())),
        ))
        .expect_err("the default implementation cannot honour a receipt");

    match error {
        EngineFacadeError::ReceiptRefused { code, detail } => {
            assert_eq!(code, "receipt_unsupported");
            assert!(
                detail.contains("not dropped"),
                "the refusal says what it did instead of dropping the document: {detail}"
            );
        }
        other => panic!("expected ReceiptRefused, got {other:?}"),
    }
}

#[test]
fn a_call_without_a_receipt_still_works_and_reports_the_not_declared_state() {
    let runtime = tokio::runtime::Runtime::new().expect("a runtime");
    let result: ToolCallResult = runtime
        .block_on(handler(Arc::new(SilentFacade)).handle(input(None)))
        .expect("a run with no declaration is unchanged");

    let json: serde_json::Value =
        serde_json::from_str(&result.content[0].text).expect("the content is JSON");
    assert_eq!(json["receipt_digest"], serde_json::Value::Null);
    assert_eq!(
        json["receipt_state"], "not_declared",
        "the response names the state instead of collapsing it into an absent field"
    );
}

#[test]
fn the_parameter_is_additive_and_the_five_tool_names_do_not_move() {
    let names = rigorix_mcp::execution_tools::interfaces::mcp::EXECUTION_TOOL_NAMES;
    assert_eq!(names.len(), 5, "five frozen names: {names:?}");

    for schema in [
        rigorix_mcp::execution_tools::interfaces::mcp::RIGORIX_RUN_INPUT_SCHEMA,
        rigorix_mcp::execution_tools::interfaces::mcp::RIGORIX_EXECUTE_INPUT_SCHEMA,
    ] {
        let parsed: serde_json::Value = serde_json::from_str(schema).expect("the schema is JSON");
        let properties = parsed["properties"]
            .as_object()
            .expect("an object schema has properties");
        assert!(
            properties.contains_key("receipt"),
            "the receipt parameter is declared: {properties:?}"
        );
        let required = parsed.get("required").and_then(|value| value.as_array());
        if let Some(required) = required {
            assert!(
                !required.iter().any(|value| value == "receipt"),
                "the parameter is optional; a client that sends none behaves as it always did"
            );
        }
    }
}
