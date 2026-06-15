//! Concurrent-safety tests for the budget tracking module.
//!
//! Exercises LlmBudgetService::reserve() and commit() under concurrent load
//! to verify atomic counters remain consistent.

#[cfg(test)]
mod tests {
    use crate::budget_tracking::application::dto::{
        CommitReservationInput, GetBudgetStatusInput, ReserveBudgetInput,
    };
    use crate::budget_tracking::application::factory::LlmBudgetFactory;
    use crate::budget_tracking::application::llm_budget_factory_impl::LlmBudgetFactoryImpl;
    use crate::budget_tracking::application::service::LlmBudgetService;
    use std::sync::Arc;
    use tokio::sync::Barrier;
    use uuid::Uuid;

    async fn create_test_budget() -> Arc<dyn LlmBudgetService> {
        let factory = LlmBudgetFactoryImpl;
        let boxed = factory.create_default().await.unwrap();
        // Box<dyn T> -> Arc<dyn T> via into
        Arc::from(boxed)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_parallel_reservations_no_drift() {
        let budget = create_test_budget().await;
        let num_tasks = 5;
        let execution_id = Uuid::new_v4();

        let barrier = Arc::new(Barrier::new(num_tasks));

        let mut handles = Vec::new();
        for _ in 0..num_tasks {
            let budget = budget.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                let input = ReserveBudgetInput {
                    execution_id,
                    estimated_tokens: 100,
                    call_label: Some("concurrency-test".to_string()),
                };
                let result = budget.reserve(input).await.unwrap();

                let commit = CommitReservationInput {
                    execution_id,
                    call_id: result.reservation.call_id,
                    actual_tokens: 90,
                };
                budget.commit(commit).await.unwrap();
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let status = budget
            .get_status(GetBudgetStatusInput { execution_id })
            .await
            .unwrap();
        assert_eq!(
            status.calls_used, 5,
            "Expected 5 calls used after 5 parallel reservations + commits"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_budget_exhaustion_under_concurrent_load() {
        let budget = create_test_budget().await;
        let num_tasks = 12;
        let execution_id = Uuid::new_v4();

        let barrier = Arc::new(Barrier::new(num_tasks));
        let mut handles = Vec::new();
        let successes = Arc::new(std::sync::atomic::AtomicU32::new(0));

        for _ in 0..num_tasks {
            let budget = budget.clone();
            let barrier = barrier.clone();
            let successes = successes.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                let input = ReserveBudgetInput {
                    execution_id,
                    estimated_tokens: 100,
                    call_label: Some("concurrency-test".to_string()),
                };
                match budget.reserve(input).await {
                    Ok(result) => {
                        let commit = CommitReservationInput {
                            execution_id,
                            call_id: result.reservation.call_id,
                            actual_tokens: 90,
                        };
                        budget.commit(commit).await.unwrap();
                        successes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                    Err(_) => {
                        // Budget exhausted — expected
                    }
                }
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let count = successes.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            count, 5,
            "Expected exactly 5 successful reservations out of 12 concurrent callers"
        );
    }
}
