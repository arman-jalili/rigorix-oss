//! Failure-injection tests for the Audit module.

#[cfg(test)]
mod tests {
    use crate::audit::application::circuit_breaker_factory_impl::CircuitBreakerFactoryImpl;
    use crate::audit::application::factory::CircuitBreakerFactory;

    #[tokio::test]
    async fn test_circuit_breaker_opens_after_threshold() {
        let factory = CircuitBreakerFactoryImpl::new();
        let breaker = factory
            .create_default("http://test:8080".to_string())
            .await
            .unwrap();

        let result = breaker.allow_request().await;
        assert!(result.is_ok(), "Circuit should be closed initially");

        for _ in 0..5 {
            breaker.record_failure().await.unwrap();
        }

        let allowed = breaker.allow_request().await;
        assert!(
            allowed.is_err(),
            "Circuit should be open after threshold failures"
        );
    }

    #[tokio::test]
    async fn test_circuit_breaker_records_success() {
        let factory = CircuitBreakerFactoryImpl::new();
        let breaker = factory
            .create_default("http://test:8080".to_string())
            .await
            .unwrap();

        breaker.record_success().await.unwrap();
        let result = breaker.allow_request().await;
        assert!(result.is_ok(), "After success, requests should be allowed");
    }

    #[tokio::test]
    async fn test_timeout_handling() {
        let result = tokio::time::timeout(std::time::Duration::from_millis(1), async {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            "done"
        })
        .await;

        match result {
            Ok(_) => {}  // Operation completed (unlikely with 1ms timeout)
            Err(_) => {} // Timed out as expected
        }
    }
}
