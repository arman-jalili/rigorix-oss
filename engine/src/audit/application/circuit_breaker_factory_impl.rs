//! Implementation of `CircuitBreakerFactory`.
//!
//! @canonical .pi/architecture/modules/audit.md#breaker
//! Implements: CircuitBreakerFactory trait — creates circuit breaker instances
//! Issue: #14
//!
//! Creates circuit breaker instances with default or explicit configuration
//! for audit backend resilience.

use async_trait::async_trait;

use crate::audit::domain::AuditError;

use super::circuit_breaker_impl::CircuitBreakerImpl;
use super::factory::CircuitBreakerFactory;
use super::service::CircuitBreaker as CircuitBreakerTrait;

/// Implementation of `CircuitBreakerFactory`.
pub struct CircuitBreakerFactoryImpl;

impl CircuitBreakerFactoryImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CircuitBreakerFactoryImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CircuitBreakerFactory for CircuitBreakerFactoryImpl {
    async fn create(
        &self,
        backend_url: String,
        threshold: u32,
        half_open_timeout_secs: u64,
    ) -> Result<Box<dyn CircuitBreakerTrait>, AuditError> {
        Ok(Box::new(CircuitBreakerImpl::new(
            backend_url,
            threshold,
            half_open_timeout_secs,
        )))
    }

    async fn create_default(
        &self,
        backend_url: String,
    ) -> Result<Box<dyn CircuitBreakerTrait>, AuditError> {
        Ok(Box::new(CircuitBreakerImpl::new(
            backend_url,
            5,   // default threshold
            60,  // default half-open timeout (seconds)
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create() {
        let factory = CircuitBreakerFactoryImpl::new();
        let cb = factory
            .create("https://audit.example.com".to_string(), 3, 30)
            .await
            .unwrap();
        assert!(cb.allow_request().await.is_ok());
    }

    #[tokio::test]
    async fn test_create_default() {
        let factory = CircuitBreakerFactoryImpl::new();
        let cb = factory
            .create_default("https://audit.example.com".to_string())
            .await
            .unwrap();
        let stats = cb.stats().await.unwrap();
        assert_eq!(stats.threshold, 5);
    }
}
