//! Implementation of `HookRunnerFactory`.
//!
//! @canonical .pi/architecture/modules/hooks.md
//! Implements: HookRunnerFactory trait — constructs HookRunnerImpl instances
//! Issue: #414, #415
//!
//! Provides factory methods for creating `HookRunnerService` instances with
//! default or explicit configuration.

use crate::hooks::domain::config::HookConfig;
use crate::hooks::domain::error::HookError;

use super::factory::HookRunnerFactory;
use super::hook_runner_impl::HookRunnerImpl;
use super::service::HookRunnerService;

/// Factory for constructing `HookRunnerService` instances.
///
/// Creates `HookRunnerImpl` instances with the provided configuration.
pub struct HookRunnerFactoryImpl;

impl HookRunnerFactory for HookRunnerFactoryImpl {
    fn create(&self, config: HookConfig) -> Result<Box<dyn HookRunnerService>, HookError> {
        Ok(Box::new(HookRunnerImpl::new(config)))
    }

    fn create_default(&self) -> Result<Box<dyn HookRunnerService>, HookError> {
        Ok(Box::new(HookRunnerImpl::new(HookConfig::default())))
    }

    fn create_with_pre_hooks(
        &self,
        pre_tool_use_commands: Vec<String>,
    ) -> Result<Box<dyn HookRunnerService>, HookError> {
        let config = HookConfig {
            pre_tool_use: pre_tool_use_commands,
            ..Default::default()
        };
        Ok(Box::new(HookRunnerImpl::new(config)))
    }

    fn create_with_timeout(
        &self,
        config: HookConfig,
        timeout_secs: u64,
    ) -> Result<Box<dyn HookRunnerService>, HookError> {
        let mut config = config;
        config.timeout_secs = timeout_secs;
        Ok(Box::new(HookRunnerImpl::new(config)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_default() {
        let factory = HookRunnerFactoryImpl;
        let runner = factory.create_default().unwrap();
        let status = runner.status();
        assert_eq!(status.total_hook_count, 0);
        assert!(!status.is_running);
    }

    #[test]
    fn test_create_with_config() {
        let factory = HookRunnerFactoryImpl;
        let config = HookConfig {
            pre_tool_use: vec!["hook-a".into()],
            ..Default::default()
        };
        let runner = factory.create(config).unwrap();
        let status = runner.status();
        assert_eq!(status.pre_tool_use_count, 1);
    }

    #[test]
    fn test_create_with_pre_hooks() {
        let factory = HookRunnerFactoryImpl;
        let runner = factory
            .create_with_pre_hooks(vec!["pre-hook".into(), "pre-hook2".into()])
            .unwrap();
        let status = runner.status();
        assert_eq!(status.pre_tool_use_count, 2);
        assert_eq!(status.post_tool_use_count, 0);
    }

    #[test]
    fn test_create_with_timeout() {
        let factory = HookRunnerFactoryImpl;
        let config = HookConfig {
            pre_tool_use: vec!["hook".into()],
            ..Default::default()
        };
        let runner = factory.create_with_timeout(config, 60).unwrap();
        let status = runner.status();
        assert_eq!(status.timeout_secs, 60);
    }

    #[test]
    fn test_runner_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<HookRunnerFactoryImpl>();
    }
}
