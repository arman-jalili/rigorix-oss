//! Implementation of `CancellationManagerFactory`.
//!
//! @canonical .pi/architecture/modules/cancellation.md#manager
//! Implements: CancellationManagerFactory trait — constructs CancellationService instances
//! Issue: issue-cancellationmanager
//!
//! Creates `CancellationManagerImpl` instances with appropriate wiring,
//! default timeouts, and optional parent token linking for child scopes.

use async_trait::async_trait;

use crate::cancellation::domain::CancellationError;

use super::cancellation_service_impl::CancellationManagerImpl;
use super::factory::CancellationManagerFactory;
use super::service::{CancellationService, CleanupHandler};

/// Implementation of `CancellationManagerFactory`.
///
/// Constructs `CancellationManagerImpl` instances with optional
/// parent token linking, configurable graceful timeouts, and
/// registered cleanup handlers.
pub struct CancellationManagerFactoryImpl;

impl CancellationManagerFactoryImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CancellationManagerFactoryImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CancellationManagerFactory for CancellationManagerFactoryImpl {
    async fn create_default(&self) -> Result<Box<dyn CancellationService>, CancellationError> {
        Ok(Box::new(CancellationManagerImpl::default()))
    }

    async fn create_with_timeout(
        &self,
        graceful_timeout_secs: u64,
    ) -> Result<Box<dyn CancellationService>, CancellationError> {
        Ok(Box::new(CancellationManagerImpl::new(graceful_timeout_secs)))
    }

    async fn create_child(
        &self,
        parent_token: tokio_util::sync::CancellationToken,
        graceful_timeout_secs: u64,
    ) -> Result<Box<dyn CancellationService>, CancellationError> {
        Ok(Box::new(CancellationManagerImpl::child_of(
            parent_token,
            graceful_timeout_secs,
        )))
    }

    async fn register_cleanup_handler(
        &self,
        task_type: &str,
        handler: Box<dyn CleanupHandler>,
    ) {
        // This is a no-op at the factory level — cleanup handlers are
        // registered on the specific CancellationManagerImpl instance
        // after it is created.
        let _ = (task_type, handler);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_create_default() {
        let factory = CancellationManagerFactoryImpl::new();
        let service = factory.create_default().await.unwrap();
        assert!(!service.is_cancelled());
    }

    #[tokio::test]
    async fn test_create_with_timeout() {
        let factory = CancellationManagerFactoryImpl::new();
        let service = factory.create_with_timeout(60).await.unwrap();
        assert!(!service.is_cancelled());
    }

    #[tokio::test]
    async fn test_create_child() {
        let factory = CancellationManagerFactoryImpl::new();
        let parent = tokio_util::sync::CancellationToken::new();
        let service = factory
            .create_child(parent.clone(), 30)
            .await
            .unwrap();

        assert!(!service.is_cancelled());

        // Cancel parent, child should propagate
        parent.cancel();
        assert!(service.is_cancelled());
    }
}
