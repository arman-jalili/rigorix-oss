//! Data Transfer Objects for the Enterprise module.

use crate::enterprise::domain::{EnterpriseConfig, PolicyBundle};
use crate::enforcement::domain::EnforcementConfig;

// ---------------------------------------------------------------------------
// Fetch Bundle DTOs
// ---------------------------------------------------------------------------

/// Input for fetching the policy bundle.
#[derive(Debug, Clone)]
pub struct FetchBundleInput {
    pub config: EnterpriseConfig,
}

/// Output from fetching the policy bundle.
#[derive(Debug, Clone)]
pub struct FetchBundleOutput {
    pub bundle: PolicyBundle,
}

// ---------------------------------------------------------------------------
// Merge Policies DTOs
// ---------------------------------------------------------------------------

/// Input for merging enterprise policies into local enforcement config.
#[derive(Debug, Clone)]
pub struct MergePoliciesInput {
    pub bundle: PolicyBundle,
    pub local_config: EnforcementConfig,
}

/// Output from merging policies.
#[derive(Debug, Clone)]
pub struct MergePoliciesOutput {
    pub merged_config: EnforcementConfig,
}
