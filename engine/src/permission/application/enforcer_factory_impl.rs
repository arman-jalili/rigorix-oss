//! Concrete implementation of the PermissionEnforcerFactory.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md#application
//! Implements: ISSUE-PERMISSION-ENFORCER-3 — PermissionEnforcerFactory
//! Issue: issue-permissionenforcer
//!
//! Provides the concrete factory for constructing `PermissionEnforcerImpl`
//! instances from configuration, modes, and presets.

use async_trait::async_trait;

use crate::permission::application::enforcer::PermissionEnforcer;
use crate::permission::application::enforcer_impl::PermissionEnforcerImpl;
use crate::permission::application::factory::PermissionEnforcerFactory;
use crate::permission::domain::{
    PermissionConfig, PermissionError, PermissionMode, PermissionPolicy,
};

/// Factory for creating `PermissionEnforcerImpl` instances.
///
/// Constructs enforcers from permission configuration with specified
/// modes and rules.
pub struct PermissionEnforcerFactoryImpl;

impl PermissionEnforcerFactoryImpl {
    /// Build a PermissionPolicy from a PermissionConfig and mode.
    fn build_policy(config: &PermissionConfig, mode: PermissionMode) -> PermissionPolicy {
        PermissionPolicy::new(
            mode,
            config.tool_permissions.clone(),
            config.allow.clone(),
            config.deny.clone(),
            config.ask.clone(),
        )
    }
}

#[async_trait]
impl PermissionEnforcerFactory for PermissionEnforcerFactoryImpl {
    async fn create_from_config(
        &self,
        config: PermissionConfig,
    ) -> Result<Box<dyn PermissionEnforcer>, PermissionError> {
        let mode = config.default_mode;
        let policy = Self::build_policy(&config, mode);
        Ok(Box::new(PermissionEnforcerImpl::new(policy, ".")))
    }

    async fn create_with_mode(
        &self,
        mode: PermissionMode,
    ) -> Result<Box<dyn PermissionEnforcer>, PermissionError> {
        let config = PermissionConfig {
            default_mode: mode,
            ..PermissionConfig::default()
        };
        let policy = Self::build_policy(&config, mode);
        Ok(Box::new(PermissionEnforcerImpl::new(policy, ".")))
    }

    async fn create_default(&self) -> Result<Box<dyn PermissionEnforcer>, PermissionError> {
        let config = PermissionConfig::default();
        let mode = config.default_mode;
        let policy = Self::build_policy(&config, mode);
        Ok(Box::new(PermissionEnforcerImpl::new(policy, ".")))
    }

    async fn create_permissive(&self) -> Result<Box<dyn PermissionEnforcer>, PermissionError> {
        let config = PermissionConfig::permissive();
        let mode = config.default_mode;
        let policy = Self::build_policy(&config, mode);
        Ok(Box::new(PermissionEnforcerImpl::new(policy, ".")))
    }

    async fn create_read_only(&self) -> Result<Box<dyn PermissionEnforcer>, PermissionError> {
        let config = PermissionConfig::read_only();
        let mode = config.default_mode;
        let policy = Self::build_policy(&config, mode);
        Ok(Box::new(PermissionEnforcerImpl::new(policy, ".")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_factory_default() {
        let factory = PermissionEnforcerFactoryImpl;
        let enforcer = factory.create_default().await.unwrap();
        assert_eq!(enforcer.active_mode(), PermissionMode::WorkspaceWrite);
    }

    #[tokio::test]
    async fn test_factory_read_only() {
        let factory = PermissionEnforcerFactoryImpl;
        let enforcer = factory.create_read_only().await.unwrap();
        assert_eq!(enforcer.active_mode(), PermissionMode::ReadOnly);
    }

    #[tokio::test]
    async fn test_factory_permissive() {
        let factory = PermissionEnforcerFactoryImpl;
        let enforcer = factory.create_permissive().await.unwrap();
        assert_eq!(enforcer.active_mode(), PermissionMode::DangerousFullAccess);
    }

    #[tokio::test]
    async fn test_factory_with_mode() {
        let factory = PermissionEnforcerFactoryImpl;
        let enforcer = factory
            .create_with_mode(PermissionMode::DangerousFullAccess)
            .await
            .unwrap();
        assert_eq!(enforcer.active_mode(), PermissionMode::DangerousFullAccess);
    }

    #[tokio::test]
    async fn test_factory_from_config() {
        let factory = PermissionEnforcerFactoryImpl;
        let config = PermissionConfig {
            default_mode: PermissionMode::ReadOnly,
            allow: vec!["read_file".to_string()],
            ask: vec![], // remove ask rules for this test
            ..PermissionConfig::default()
        };
        let enforcer = factory.create_from_config(config).await.unwrap();
        assert_eq!(enforcer.active_mode(), PermissionMode::ReadOnly);

        // read_file is in allow rules, so it should be allowed even in read_only
        let outcome = enforcer.check("read_file", "test.txt", None).await;
        assert!(outcome.is_allowed());
    }

    #[tokio::test]
    async fn test_factory_enforcer_works() {
        let factory = PermissionEnforcerFactoryImpl;
        let enforcer = factory.create_default().await.unwrap();

        // Verify the enforcer actually works
        assert!(
            enforcer
                .check("read_file", "test.txt", None)
                .await
                .is_allowed()
        );
        assert!(
            enforcer
                .check("write_file", "/tmp/test.txt", None)
                .await
                .is_allowed()
        );
    }

    #[tokio::test]
    async fn test_factory_read_only_enforcer_denies_writes() {
        let factory = PermissionEnforcerFactoryImpl;
        let enforcer = factory.create_read_only().await.unwrap();

        assert!(
            enforcer
                .check("read_file", "test.txt", None)
                .await
                .is_allowed()
        );
        assert!(
            enforcer
                .check("write_file", "/tmp/test.txt", None)
                .await
                .is_denied()
        );
    }
}
