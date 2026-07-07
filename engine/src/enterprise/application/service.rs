//! EnterpriseService trait — orchestrates policy fetch, verify, and merge.

use async_trait::async_trait;

use crate::enforcement::domain::EnforcementConfig;
use crate::enterprise::domain::{EnterpriseConfig, EnterpriseError};

use super::dto::{FetchBundleOutput, MergePoliciesOutput};

/// Service for enterprise policy bundle operations.
///
/// Implementations handle:
/// 1. Fetching policy bundles from the enterprise API
/// 2. Verifying HMAC signatures
/// 3. Merging enterprise policies into EnforcementConfig
#[async_trait]
pub trait EnterpriseService: Send + Sync {
    /// Fetch and verify the policy bundle for the given enterprise config.
    ///
    /// Returns the verified bundle, or an error if fetch or verification fails.
    async fn fetch_policy_bundle(
        &self,
        config: &EnterpriseConfig,
    ) -> Result<FetchBundleOutput, EnterpriseError>;

    /// Merge enterprise policies from a bundle into the local enforcement config.
    ///
    /// Enterprise policies always win in conflicts.
    async fn merge_policies(
        &self,
        bundle: &crate::enterprise::domain::PolicyBundle,
        local_config: &EnforcementConfig,
    ) -> Result<MergePoliciesOutput, EnterpriseError>;

    /// Get the merged EnforcementConfig, fetching from cache or enterprise API.
    ///
    /// Uses in-memory cache with TTL to avoid redundant fetches.
    async fn get_enforcement_config(
        &self,
        enterprise_config: &EnterpriseConfig,
        local_config: &EnforcementConfig,
    ) -> Result<EnforcementConfig, EnterpriseError>;
}
