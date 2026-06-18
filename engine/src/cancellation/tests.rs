//! Integration tests for the Cancellation module.

#[cfg(test)]
mod tests {
    use crate::cancellation::application::cancellation_manager_factory_impl::CancellationManagerFactoryImpl;
    use crate::cancellation::application::dto::CancelExecutionInput;
    use crate::cancellation::application::factory::CancellationManagerFactory;

    fn cancel_input(execution_id: &str, reason: &str) -> CancelExecutionInput {
        CancelExecutionInput {
            execution_id: execution_id.to_string(),
            reason: Some(reason.to_string()),
            source: "test".to_string(),
        }
    }

    #[tokio::test]
    async fn test_create_default() {
        let factory = CancellationManagerFactoryImpl::new();
        let service = factory.create_default().await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_token_available() {
        let factory = CancellationManagerFactoryImpl::new();
        let service = factory.create_default().await.unwrap();
        let token = service.cancellation_token();
        assert!(!token.is_cancelled());
    }

    #[tokio::test]
    async fn test_graceful_shutdown_cancels_token() {
        let factory = CancellationManagerFactoryImpl::new();
        let service = factory.create_default().await.unwrap();
        let token = service.cancellation_token();
        let result = service
            .request_graceful_shutdown(cancel_input("exec-1", "test"))
            .await;
        assert!(result.is_ok());
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_immediate_abort() {
        let factory = CancellationManagerFactoryImpl::new();
        let service = factory.create_default().await.unwrap();
        let token = service.cancellation_token();
        let result = service
            .request_immediate_abort(cancel_input("exec-2", "immediate"))
            .await;
        assert!(result.is_ok());
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_double_cancellation_idempotent() {
        let factory = CancellationManagerFactoryImpl::new();
        let service = factory.create_default().await.unwrap();
        // First call should succeed
        let first = service
            .request_graceful_shutdown(cancel_input("exec-3", "first"))
            .await;
        // Second call may return AlreadyCancelling — that's also valid
        let second = service
            .request_graceful_shutdown(cancel_input("exec-3", "second"))
            .await;
        // Either ok or AlreadyCancelling are acceptable
        match second {
            Ok(_) => {}
            Err(crate::cancellation::domain::CancellationError::AlreadyCancelling { .. }) => {}
            Err(e) => panic!("Unexpected error on second cancellation: {}", e),
        }
        // First should always succeed
        assert!(first.is_ok(), "First cancellation should always succeed");
    }

    #[tokio::test]
    async fn test_is_cancelled_state() {
        let factory = CancellationManagerFactoryImpl::new();
        let service = factory.create_default().await.unwrap();
        assert!(!service.is_cancelled());
        service
            .request_graceful_shutdown(cancel_input("exec-4", "test"))
            .await
            .unwrap();
        assert!(service.is_cancelled());
    }
}
