//! ExecutionIntent — canonical representation of what a step will dispatch.
//!
//! @canonical .pi/architecture/modules/approval.md#executionintent
//! Implements: Contract Freeze — ExecutionIntent value object
//! Issue: #786 (approval epic — contract freeze); behavior in
//!   ISSUE-EXECUTIONINTENT (#787)
//!
//! The byte-level source of truth for both display and hashing. An
//! `ExecutionIntent` carries the tool, the resolved intent payload exactly as
//! stored on the sealed `TaskNode`, and the declared effect scope — the three
//! things bound by the approval digest (R1).
//!
//! # Contract (Frozen)
//! - `from_node` builds the canonical intent from a sealed `TaskNode` — the
//!   exact bytes that will dispatch
//! - `canonical_bytes` serializes `{ tool, intent, declared_scope }` as
//!   **sorted-key JSON** (recursively sorted keys, stable field order) so
//!   identical payloads always hash identically
//! - `render` is the human-readable form and MUST derive from the same
//!   canonical serialization as `canonical_bytes` — there is exactly one
//!   renderer (shown = dispatched = hashed)
//! - Dynamic runtime context knowable only at dispatch time is excluded from
//!   the hash and recorded in `decision_context` as evidence instead
//!
//! **Implementation note:** method bodies are `todo!()` stubs; the canonical
//! serialization logic lands in ISSUE-EXECUTIONINTENT.

use serde::{Deserialize, Serialize};

use crate::dag_engine::domain::TaskNode;

/// The exact `(tool, intent payload, declared scope)` a step will dispatch.
///
/// For `llm_generate` nodes (input-anchored binding class) `intent` carries
/// the **assembled prompt** — resolved `prompt_template` plus the filled
/// `LlmStepContext`. For deterministic tool steps (payload-anchored class)
/// it carries the resolved post-template payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionIntent {
    /// Tool/action string, e.g. "run_command", "file_write", "edit_file", "llm_generate".
    pub tool: String,
    /// The resolved intent payload (post-template-resolution) exactly as
    /// stored on the `TaskNode`.
    pub intent: serde_json::Value,
    /// Optional declared effect scope — files this step intends to touch.
    pub declared_scope: Vec<String>,
}

impl ExecutionIntent {
    /// Build the canonical intent from a sealed `TaskNode` — the exact bytes
    /// that will dispatch.
    ///
    /// # States
    /// - **Populated:** tool + intent from a sealed graph node
    /// - **Error:** node missing tool or intent
    ///
    /// # Implementation
    /// TODO: implemented in ISSUE-EXECUTIONINTENT (#787).
    pub fn from_node(_node: &TaskNode) -> Self {
        todo!("ISSUE-EXECUTIONINTENT (#787): derive canonical intent from sealed TaskNode")
    }

    /// Canonical serialization for hashing — deterministic field order
    /// (recursively sorted-key JSON over `{ tool, intent, declared_scope }`).
    ///
    /// # Implementation
    /// TODO: implemented in ISSUE-EXECUTIONINTENT (#787).
    pub fn canonical_bytes(&self) -> Vec<u8> {
        todo!("ISSUE-EXECUTIONINTENT (#787): sorted-key canonical serialization")
    }

    /// Human-readable render — the SAME renderer used for display and hashing.
    ///
    /// # Invariant
    /// `render()` and `canonical_bytes()` derive from the same canonical
    /// serialization — what the human sees is what is hashed.
    ///
    /// # Implementation
    /// TODO: implemented in ISSUE-EXECUTIONINTENT (#787).
    pub fn render(&self) -> String {
        todo!("ISSUE-EXECUTIONINTENT (#787): single canonical renderer")
    }
}
