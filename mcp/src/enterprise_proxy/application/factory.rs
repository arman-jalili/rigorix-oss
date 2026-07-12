//! Factory interfaces for the Enterprise Proxy bounded context.
//!
//! @canonical .pi/architecture/modules/enterprise-proxy.md#factories
//! Implements: Contract Freeze — factory interfaces
//!
//! Factory interfaces encapsulate the construction of complex domain objects.
//! They provide creation methods that handle validation, default values,
//! and cross-field invariants.
//!
//! # Contract (Frozen)
//!
//! - Factory methods validate inputs before constructing
//! - All factory methods return Result for fallible construction
//! - Factory interfaces are async for I/O-bound construction

use async_trait::async_trait;

use crate::enterprise_proxy::domain::error::ProxyError;
use crate::enterprise_proxy::domain::value::{EnterpriseMetadata, ProxyConfig};

/// Factory for constructing ProxyConfig instances.
///
/// Encapsulates the details of ProxyConfig construction including
/// URL validation, API key storage, and default value application.
#[async_trait]
pub trait ProxyConfigFactory: Send + Sync {
    /// Create a ProxyConfig from raw configuration values.
    ///
    /// Validates the API URL (must be HTTPS), ensures API key is not empty,
    /// and applies default values for optional fields.
    ///
    /// # Errors
    /// - `ProxyError::Configuration` if URL is invalid or key is empty
    async fn create(
        &self,
        api_url: String,
        api_key: String,
        timeout_secs: Option<u64>,
        tls_verify: Option<bool>,
        max_retries: Option<u32>,
        schema_ttl_secs: Option<u64>,
    ) -> Result<ProxyConfig, ProxyError>;
}

/// Factory for constructing EnterpriseMetadata from raw API responses.
///
/// Handles deserialization validation and normalizes the metadata
/// structure for internal use.
#[async_trait]
pub trait EnterpriseMetadataFactory: Send + Sync {
    /// Create EnterpriseMetadata from a raw JSON response.
    ///
    /// Validates required fields (version, tools, server_name) and
    /// provides defaults for optional fields (capabilities).
    ///
    /// # Errors
    /// - `ProxyError::Deserialization` if the JSON structure is invalid
    async fn create_from_json(
        &self,
        json: serde_json::Value,
    ) -> Result<EnterpriseMetadata, ProxyError>;
}
