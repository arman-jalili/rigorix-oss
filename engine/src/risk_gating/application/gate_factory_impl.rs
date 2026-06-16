//! Implementation of the RiskGateFactory.
//!
//! @canonical .pi/architecture/modules/risk-gating.md
//! Implements: ISSUE-RISK-GATING-1 — RiskGateFactory implementation
//! Issue: #90
//!
//! Provides the concrete `RiskGateFactoryImpl` that constructs
//! `RiskGateServiceImpl` instances with appropriate classifiers
//! and configuration.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::risk_gating::application::factory::RiskGateFactory;
use crate::risk_gating::application::gate_service_impl::RiskGateServiceImpl;
use crate::risk_gating::application::service::RiskGateService;
use crate::risk_gating::domain::{GateStateRegistry, RiskConfig, RiskGatingError, RiskLevel};

/// Implementation of the RiskGateFactory.
///
/// Uses a shared `GateStateRegistry` for cross-execution gate tracking.
pub struct RiskGateFactoryImpl {
    /// Shared gate state registry.
    gate_registry: Arc<GateStateRegistry>,
}

impl RiskGateFactoryImpl {
    /// Create a new `RiskGateFactoryImpl` with a shared gate registry.
    pub fn new(gate_registry: Arc<GateStateRegistry>) -> Self {
        Self { gate_registry }
    }

    /// Create a new `RiskGateFactoryImpl` with a fresh gate registry.
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self {
            gate_registry: Arc::new(GateStateRegistry::new()),
        }
    }
}

#[async_trait]
impl RiskGateFactory for RiskGateFactoryImpl {
    async fn create_from_config(
        &self,
        execution_id: &str,
        config: RiskConfig,
    ) -> Result<Box<dyn RiskGateService>, RiskGatingError> {
        let service = RiskGateServiceImpl::new(
            execution_id.to_string(),
            config,
            Arc::clone(&self.gate_registry),
        );
        Ok(Box::new(service))
    }

    async fn create_default(
        &self,
        execution_id: &str,
    ) -> Result<Box<dyn RiskGateService>, RiskGatingError> {
        let config = RiskConfig::default();
        let service = RiskGateServiceImpl::new(
            execution_id.to_string(),
            config,
            Arc::clone(&self.gate_registry),
        );
        Ok(Box::new(service))
    }

    async fn create_with_overrides(
        &self,
        execution_id: &str,
        config: RiskConfig,
        additional_overrides: HashMap<String, RiskLevel>,
    ) -> Result<Box<dyn RiskGateService>, RiskGatingError> {
        let mut merged_config = config;
        merged_config.tool_overrides.extend(additional_overrides);

        let service = RiskGateServiceImpl::new(
            execution_id.to_string(),
            merged_config,
            Arc::clone(&self.gate_registry),
        );
        Ok(Box::new(service))
    }

    async fn create_with_policy(
        &self,
        execution_id: &str,
        config: RiskConfig,
        auto_confirm_low: bool,
        require_review_medium: bool,
        dry_run_high: bool,
    ) -> Result<Box<dyn RiskGateService>, RiskGatingError> {
        let mut policy_config = config;
        policy_config.auto_confirm_low = auto_confirm_low;
        policy_config.require_review_medium = require_review_medium;
        policy_config.dry_run_high = dry_run_high;

        let service = RiskGateServiceImpl::new(
            execution_id.to_string(),
            policy_config,
            Arc::clone(&self.gate_registry),
        );
        Ok(Box::new(service))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_default() {
        let factory = RiskGateFactoryImpl::default();
        let service = factory.create_default("exec-1").await.unwrap();

        let config = service.get_config().await.unwrap();
        assert!(config.config.auto_confirm_low);
        assert!(config.config.require_review_medium);
        assert!(config.config.dry_run_high);
        assert_eq!(config.override_count, 0);
    }

    #[tokio::test]
    async fn test_create_with_overrides() {
        let factory = RiskGateFactoryImpl::default();
        let mut overrides = HashMap::new();
        overrides.insert("file_read".to_string(), RiskLevel::High);

        let service = factory
            .create_with_overrides("exec-1", RiskConfig::default(), overrides)
            .await
            .unwrap();

        let config = service.get_config().await.unwrap();
        assert_eq!(config.override_count, 1);
        assert_eq!(
            config.config.get_override("file_read"),
            Some(&RiskLevel::High)
        );
    }

    #[tokio::test]
    async fn test_create_with_policy() {
        let factory = RiskGateFactoryImpl::default();
        let service = factory
            .create_with_policy("exec-1", RiskConfig::default(), true, false, true)
            .await
            .unwrap();

        let config = service.get_config().await.unwrap();
        assert!(config.config.auto_confirm_low);
        assert!(!config.config.require_review_medium);
        assert!(config.config.dry_run_high);
    }

    #[tokio::test]
    async fn test_create_from_config() {
        let factory = RiskGateFactoryImpl::default();
        let config = RiskConfig::default();
        let service = factory.create_from_config("exec-1", config).await.unwrap();
        assert!(service.classifier().risk_level("file_read", None).is_low());
    }
}
