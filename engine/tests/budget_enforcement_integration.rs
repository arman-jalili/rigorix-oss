//! Integration test: budget enforcement triggers cancellation.
//!
//! Tests that budget exhaustion correctly prevents execution from starting
//! and propagates LlmBudgetError.

use rigorix_engine::budget_tracking::application::dto::{
    CommitReservationInput, GetBudgetStatusInput, ReserveBudgetInput,
};
use rigorix_engine::budget_tracking::application::factory::LlmBudgetFactory;
use rigorix_engine::budget_tracking::application::llm_budget_factory_impl::LlmBudgetFactoryImpl;
use uuid::Uuid;

#[tokio::test]
async fn test_budget_exhaustion_prevents_execution() {
    let factory = LlmBudgetFactoryImpl;
    let budget = factory.create_default().await.unwrap();
    let execution_id = Uuid::new_v4();

    // Default budget has 5 calls. Exhaust them all.
    for i in 0..5 {
        let input = ReserveBudgetInput {
            execution_id,
            estimated_tokens: 100,
            call_label: Some(format!("test-call-{}", i)),
        };
        let result = budget
            .reserve(input)
            .await
            .unwrap_or_else(|_| panic!("Reservation {} should have succeeded", i));

        let commit = CommitReservationInput {
            execution_id,
            call_id: result.reservation.call_id,
            reserved_tokens: result.reservation.reserved_tokens,
            actual_tokens: 50,
        };
        budget.commit(commit).await.unwrap();
    }

    // Verify budget is exhausted
    let status = budget
        .get_status(GetBudgetStatusInput { execution_id })
        .await
        .unwrap();
    assert_eq!(status.calls_used, 5);

    // Next reservation should fail
    let exhausted = ReserveBudgetInput {
        execution_id,
        estimated_tokens: 100,
        call_label: Some("should-fail".to_string()),
    };
    let result = budget.reserve(exhausted).await;
    assert!(
        result.is_err(),
        "Expected budget exhaustion error on 6th call"
    );
}
