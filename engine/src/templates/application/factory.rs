//! Factory interfaces for constructing Template System domain objects.
//!
//! @canonical .pi/architecture/modules/template-system.md
//! Implements: Contract Freeze — TemplateFactory and ParameterFactory traits
//! Issue: #101
//!
//! Factories encapsulate the construction of complex domain objects,
//! allowing implementations to inject dependencies and apply defaults
//! without exposing construction logic to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured domain object
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::templates::domain::{Template, TemplateError};

use super::dto::{GenerateOutput, TemplateSystemConfig, TemplateSummary};

// ---------------------------------------------------------------------------
// TemplateFactory
// ---------------------------------------------------------------------------

/// Factory for constructing `Template` aggregates.
///
/// Implementations handle constructing Template objects from partial data,
/// applying defaults for unset fields, and validating the result.
#[async_trait]
pub trait TemplateFactory: Send + Sync {
    /// Construct a `Template` from raw field values with defaults applied.
    ///
    /// Applies default values for optional fields (empty vecs, version "0.1.0").
    /// Returns error on structural validation failure.
    async fn build_template(
        &self,
        id: &str,
        name: &str,
        description: &str,
    ) -> Result<Template, TemplateError>;

    /// Create a `TemplateSummary` from a full `Template`.
    ///
    /// Extracts the summary-relevant fields and computes counts.
    fn summarize(&self, template: &Template) -> TemplateSummary;

    /// Create a default `TemplateSystemConfig`.
    fn default_config(&self) -> TemplateSystemConfig;
}

// ---------------------------------------------------------------------------
// GraphFactory
// ---------------------------------------------------------------------------

/// Factory for constructing generated graph outputs.
///
/// Implementations handle parameter substitution in template node actions,
/// building node/edge lists, and producing the final `GenerateOutput`.
///
/// @todo When the DAG Engine module exists, this should produce TaskGraph
///   instances directly instead of GenerateOutput DTOs.
#[async_trait]
pub trait GraphFactory: Send + Sync {
    /// Perform `{{ param }}` substitution on a template's nodes.
    ///
    /// Substitutes all occurrences of `{{ param_name }}` in node action
    /// fields with the corresponding parameter values.
    async fn substitute_params(
        &self,
        template: &Template,
        params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<GenerateOutput, TemplateError>;

    /// Detect cycles in node dependencies.
    ///
    /// Uses Kahn's algorithm for cycle detection. Returns the node IDs
    /// involved in any detected cycle, or empty vec if no cycle exists.
    async fn detect_cycles(
        &self,
        nodes: &[crate::templates::domain::TemplateNode],
    ) -> Result<Vec<String>, TemplateError>;

    /// Compute the topological order of nodes.
    ///
    /// Returns ordered node IDs or error if cycle detected.
    async fn topological_sort(
        &self,
        nodes: &[crate::templates::domain::TemplateNode],
    ) -> Result<Vec<String>, TemplateError>;
}
