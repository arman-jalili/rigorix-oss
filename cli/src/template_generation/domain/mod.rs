//! Template generation domain types.
//!
//! @canonical .pi/architecture/modules/template-generation.md
//! Implements: Contract Freeze — GenerationCliError, TemplateGenerationCliEvent
//! Issue: issue-contract-freeze

pub mod error;
pub mod event;

pub use error::GenerationCliError;
pub use event::TemplateGenerationCliEvent;
