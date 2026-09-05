//! Guardian TDD unit tests (GAP-A-24 corrected): these files were inert
//! `assert!(false)` scaffolds that were never compiled (no `unit/mod.rs`).
//! Each is preserved with its Guardian TDD structure and corrected to
//! exercise the real implementation. Runs under `cargo test --test unit`.

mod identity {
    mod identityattestationservice {
        mod identityattestationservice_test;
    }
    mod identityclaim {
        mod identityclaim_test;
    }
    mod identityerror {
        mod identityerror_test;
    }
    mod identityrepository {
        mod identityrepository_test;
    }
    mod identitysource {
        mod identitysource_test;
    }
    mod tokenverifier {
        mod tokenverifier_test;
    }
}
mod approval {
    mod approvalerror {
        mod approvalerror_test;
    }
    mod approvalrecord {
        mod approvalrecord_test;
    }
    mod approvalservice {
        mod approvalservice_test;
    }
    #[path = "approveinput-approveoutput"]
    mod approveinput_approveoutput {
        #[path = "approveinput-approveoutput_test.rs"]
        mod approveinput_approveoutput_test;
    }
    mod decisioncontext {
        mod decisioncontext_test;
    }
    mod executionintent {
        mod executionintent_test;
    }
    mod intenthash {
        mod intenthash_test;
    }
    mod scopeviolation {
        mod scopeviolation_test;
    }
}
#[path = "scored-evaluation"]
mod scored_evaluation {
    #[path = "application-layer-application"]
    mod application_layer_application {
        #[path = "application-layer-application_test.rs"]
        mod application_layer_application_test;
    }
    #[path = "domain-layer-domain"]
    mod domain_layer_domain {
        #[path = "domain-layer-domain_test.rs"]
        mod domain_layer_domain_test;
    }
    #[path = "evaluateinput-evaluateoutput"]
    mod evaluateinput_evaluateoutput {
        #[path = "evaluateinput-evaluateoutput_test.rs"]
        mod evaluateinput_evaluateoutput_test;
    }
    mod httpbackend {
        mod httpbackend_test;
    }
    #[path = "infrastructure-layer-infrastructure"]
    mod infrastructure_layer_infrastructure {
        #[path = "infrastructure-layer-infrastructure_test.rs"]
        mod infrastructure_layer_infrastructure_test;
    }
    mod localbackend {
        mod localbackend_test;
    }
    mod mcpbackend {
        mod mcpbackend_test;
    }
    mod rubric {
        mod rubric_test;
    }
    #[path = "scoreabove-policy-condition"]
    mod scoreabove_policy_condition {
        #[path = "scoreabove-policy-condition_test.rs"]
        mod scoreabove_policy_condition_test;
    }
    #[path = "scorebelow-policy-condition"]
    mod scorebelow_policy_condition {
        #[path = "scorebelow-policy-condition_test.rs"]
        mod scorebelow_policy_condition_test;
    }
    mod scoredevaluationerror {
        mod scoredevaluationerror_test;
    }
    mod scoredevaluationevent {
        mod scoredevaluationevent_test;
    }
    mod scoredevaluationnode {
        mod scoredevaluationnode_test;
    }
    mod scoredevaluationservice {
        mod scoredevaluationservice_test;
    }
    mod scoredimension {
        mod scoredimension_test;
    }
    mod scoringbackend {
        mod scoringbackend_test;
    }
    #[path = "scoringbackend-trait"]
    mod scoringbackend_trait {
        #[path = "scoringbackend-trait_test.rs"]
        mod scoringbackend_trait_test;
    }
    mod scoringresult {
        mod scoringresult_test;
    }
}
#[path = "sequence-policy"]
mod sequence_policy {
    #[path = "audit-r6"]
    mod audit_r6 {
        #[path = "audit-r6_test.rs"]
        mod audit_r6_test;
    }
    #[path = "execution-engine-r3"]
    mod execution_engine_r3 {
        #[path = "execution-engine-r3_test.rs"]
        mod execution_engine_r3_test;
    }
    #[path = "fail-closed"]
    mod fail_closed {
        #[path = "fail-closed_test.rs"]
        mod fail_closed_test;
    }
    #[path = "fail-open-absent"]
    mod fail_open_absent {
        #[path = "fail-open-absent_test.rs"]
        mod fail_open_absent_test;
    }
    mod matcher {
        mod matcher_test;
    }
    #[path = "mcp-surface"]
    mod mcp_surface {
        #[path = "mcp-surface_test.rs"]
        mod mcp_surface_test;
    }
    #[path = "orchestrator-r2"]
    mod orchestrator_r2 {
        #[path = "orchestrator-r2_test.rs"]
        mod orchestrator_r2_test;
    }
    #[path = "permission-r5"]
    mod permission_r5 {
        #[path = "permission-r5_test.rs"]
        mod permission_r5_test;
    }
    mod sequencepolicyerror {
        mod sequencepolicyerror_test;
    }
    mod sequencepolicyservice {
        mod sequencepolicyservice_test;
    }
    mod sequencerule {
        mod sequencerule_test;
    }
    mod steppredicate {
        mod steppredicate_test;
    }
}
