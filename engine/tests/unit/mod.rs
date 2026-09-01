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
