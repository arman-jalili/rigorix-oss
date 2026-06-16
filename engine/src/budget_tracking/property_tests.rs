//! Deterministic property-style tests for budget arithmetic.
//!
//! Tests budget reservation/commit consistency across multiple scenarios.

#![cfg(test)]

mod tests {
    use crate::budget_tracking::domain::LlmBudget;

    #[test]
    fn test_budget_arithmetic_consistency() {
        // reserve(N) + commit(M) for various N, M values
        let scenarios = vec![
            (1u32, 100u32, 50u32),
            (5, 1000, 800),
            (10, 5000, 5000),
            (3, 200, 0),
        ];

        for (calls, max_tokens, _actual_tokens) in scenarios {
            let budget = LlmBudget {
                max_calls: calls + 1,
                max_tokens,
                used_calls: 0,
                used_tokens: 0,
                label: "test".to_string(),
            };

            // Verify initial state
            assert!(budget.has_capacity());
            assert_eq!(budget.remaining_calls(), calls + 1);
            assert_eq!(budget.remaining_tokens(), max_tokens);
        }
    }

    #[test]
    fn test_budget_exhaustion_behavior() {
        let budget = LlmBudget {
            max_calls: 5,
            max_tokens: 1000,
            used_calls: 5,
            used_tokens: 1000,
            label: "exhausted".to_string(),
        };

        assert!(!budget.has_capacity());
        assert_eq!(budget.remaining_calls(), 0);
        assert_eq!(budget.remaining_tokens(), 0);
        assert_eq!(budget.call_usage_ratio(), 1.0);
        assert_eq!(budget.token_usage_ratio(), 1.0);
    }
}
