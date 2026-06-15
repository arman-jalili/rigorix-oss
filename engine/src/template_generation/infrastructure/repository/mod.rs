//! Repository interfaces for the Template Generation bounded context.
//!
//! @canonical .pi/architecture/modules/template-generation.md#repositories
//! Issue: issue-contract-freeze
//!
//! The GeneratedTemplateRepository is defined in the planning module
//! (src/planning/infrastructure/repository/) since generated templates
//! are consumed by the planning pipeline.

pub use crate::planning::infrastructure::repository::GeneratedTemplateRepository;
