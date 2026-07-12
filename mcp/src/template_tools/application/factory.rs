//! Factory interfaces for the Template Tools bounded context.
//!
//! @canonical .pi/architecture/modules/template-tools.md#factories
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

use crate::template_tools::domain::error::TemplateError;
use crate::template_tools::domain::value::PlanTemplate;

/// Factory for constructing PlanTemplate instances.
///
/// Encapsulates the details of PlanTemplate construction including
/// validation, default timestamps, and metadata normalization.
#[async_trait]
pub trait PlanTemplateFactory: Send + Sync {
    /// Create a PlanTemplate from raw JSON input.
    ///
    /// Validates the JSON structure and constructs a PlanTemplate with
    /// default timestamps (created_at = updated_at = now).
    ///
    /// # Errors
    /// - `TemplateError::DeserializationFailed` if JSON is malformed
    /// - `TemplateError::ValidationError` if template structure is invalid
    ///   (e.g., empty steps, invalid field values)
    async fn create_from_json(
        &self,
        json: serde_json::Value,
    ) -> Result<PlanTemplate, TemplateError>;

    /// Create a PlanTemplate with defaults for optional fields.
    ///
    /// Useful for test helpers and when constructing templates from
    /// known-valid data (e.g., programmatic creation).
    ///
    /// # Errors
    /// - `TemplateError::ValidationError` if template structure is invalid
    async fn create(
        &self,
        name: String,
        description: String,
        steps: Vec<crate::template_tools::domain::value::StepDefinition>,
    ) -> Result<PlanTemplate, TemplateError>;
}
