//! Event payload schemas for the Template Generation bounded context.
//!
//! @canonical .pi/architecture/modules/template-generation.md#events
//! Issue: issue-contract-freeze
//!
//! Events emitted during the template generation lifecycle.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Events emitted by the Template Generation module.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TemplateGenerationEvent {
    /// A template generation was started.
    GenerationStarted {
        session_id: Uuid,
        intent: String,
        timestamp: DateTime<Utc>,
    },
    /// A template was successfully generated.
    GenerationCompleted {
        session_id: Uuid,
        template_id: String,
        llm_calls: u32,
        timestamp: DateTime<Utc>,
    },
    /// Template generation failed.
    GenerationFailed {
        session_id: Uuid,
        error: String,
        attempts: u8,
        timestamp: DateTime<Utc>,
    },
    /// Phase 3 symbol validation result.
    SymbolValidationCompleted {
        session_id: Uuid,
        template_id: String,
        passed: bool,
        invalid_references: u32,
        timestamp: DateTime<Utc>,
    },
}
