//! Event payload schemas for the CLI Template Generation module.
//!
//! @canonical .pi/architecture/modules/template-generation.md
//! Implements: Contract Freeze — TemplateGenerationCliEvent
//! Issue: issue-contract-freeze

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemplateGenerationCliEvent {
    GenerationRequested {
        intent: String,
        dry_run: bool,
    },
    GenerationCompleted {
        template_id: String,
        persisted: bool,
    },
    GenerationFailed {
        intent: String,
        error: String,
    },
    TemplatePersisted {
        template_id: String,
        path: String,
    },
    DryRunCompleted {
        content: String,
    },
}
