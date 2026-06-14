//! Implementation of the ExecutionEnforcerFactory.
//!
//! @canonical .pi/architecture/modules/enforcement.md#application
//! Implements: ISSUE-ENFORCEMENT-2 — ExecutionEnforcerFactory
//! Issue: #59
//!
//! Provides the concrete factory for constructing `ExecutionEnforcerImpl`
//! instances from configuration presets, custom budgets, and tool overrides.

use async_trait::async_trait;
use std::collections::HashMap;

use crate::enforcement::application::enforcer_impl::ExecutionEnforcerImpl;
use crate::enforcement::application::factory::ExecutionEnforcerFactory;
use crate::enforcement::application::service::ExecutionEnforcer;
use crate::enforcement::domain::{EnforcementConfig, EnforcementError};

/// Factory for creating `ExecutionEnforcerImpl` instances.
///
/// Constructs enforcers from configuration presets with optional
/// budget and tool policy overrides.
pub struct ExecutionEnforcerFactoryImpl;

#[async_trait]
impl ExecutionEnforcerFactory for ExecutionEnforcerFactoryImpl {
    async fn create_from_config(
        &self,
        execution_id: &str,
        config: EnforcementConfig,
    ) -> Result<Box<dyn ExecutionEnforcer>, EnforcementError> {
        // Validate the config before creating the enforcer
        let caps = crate::enforcement::domain::SafetyCaps::default();
        let errors = config.validate(&caps);
        if !errors.is_empty() {
            return Err(EnforcementError::InvalidConfiguration {
                detail: format!(
                    "Configuration validation failed with {} error(s): {}",
                    errors.len(),
                    errors
                        .iter()
                        .map(|e| format!("{}: {}", e.field, e.message))
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
            });
        }

        Ok(Box::new(ExecutionEnforcerImpl::new(execution_id, config)))
    }

    async fn create_default(
        &self,
        execution_id: &str,
    ) -> Result<Box<dyn ExecutionEnforcer>, EnforcementError> {
        let config = EnforcementConfig::standard();
        Ok(Box::new(ExecutionEnforcerImpl::new(execution_id, config)))
    }

    async fn create_with_custom_budgets(
        &self,
        execution_id: &str,
        config: EnforcementConfig,
        budget_overrides: HashMap<String, crate::enforcement::domain::ResourceBudget>,
    ) -> Result<Box<dyn ExecutionEnforcer>, EnforcementError> {
        let mut merged_config = config;
        for (name, budget) in budget_overrides {
            merged_config.budgets.insert(name, budget);
        }

        self.create_from_config(execution_id, merged_config).await
    }

    async fn create_with_tool_overrides(
        &self,
        execution_id: &str,
        config: EnforcementConfig,
        tool_overrides: HashMap<String, crate::enforcement::domain::ToolPolicy>,
    ) -> Result<Box<dyn ExecutionEnforcer>, EnforcementError> {
        let mut merged_config = config;
        for (tool, policy) in tool_overrides {
            merged_config.tool_policies.insert(tool, policy);
        }

        self.create_from_config(execution_id, merged_config).await
    }

    async fn create_child(
        &self,
        execution_id: &str,
        _parent_enforcer: &dyn ExecutionEnforcer,
        config: EnforcementConfig,
    ) -> Result<Box<dyn ExecutionEnforcer>, EnforcementError> {
        // Create a child enforcer with its own config.
        // In a full implementation, this would:
        // 1. Inherit parent's budgets as initial values
        // 2. Apply child-specific limits on top
        // 3. Track child budgets separately but report combined to parent
        //
        // For now, we create a fresh enforcer with the provided config.
        self.create_from_config(execution_id, config).await
    }
}
