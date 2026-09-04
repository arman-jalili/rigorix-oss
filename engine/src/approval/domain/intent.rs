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
//! Implemented in ISSUE-EXECUTIONINTENT (#787).

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    /// The node's `intent` field is the resolved post-template payload. When
    /// it holds a JSON payload it is parsed into a structured value; otherwise
    /// (the common plain-description case) it is wrapped losslessly as a JSON
    /// string — deterministic per node either way.
    ///
    /// # States
    /// - **Populated:** tool + intent from a sealed graph node
    /// - **Error:** node missing tool or intent
    pub fn from_node(node: &TaskNode) -> Self {
        let parsed = serde_json::from_str::<Value>(&node.intent).ok();
        let declared_scope = parsed
            .as_ref()
            .and_then(|v| v.get("declared_scope"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str().map(|x| x.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let intent = parsed.unwrap_or_else(|| Value::String(node.intent.clone()));
        Self {
            tool: node.tool.clone(),
            intent,
            // The declared effect scope may be declared in the node's resolved
            // payload (`declared_scope: [..]`) — e.g. set by the planner or
            // the approve surface. A bare sealed node without one declares no
            // scope (R5 then has nothing to verify against).
            declared_scope,
        }
    }

    /// Canonical serialization for hashing — deterministic field order.
    ///
    /// Serializes `{ tool, intent, declared_scope }` as **sorted-key JSON**: a
    /// compact, whitespace-free encoding whose object keys are recursively
    /// sorted, so identical payloads always produce identical bytes and any
    /// byte change alters the digest. `intent` values that parse as JSON are
    /// canonicalized recursively (key order is normalized).
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut map = serde_json::Map::new();
        map.insert("tool".to_string(), Value::String(self.tool.clone()));
        map.insert("intent".to_string(), sorted_value(&self.intent));
        map.insert(
            "declared_scope".to_string(),
            Value::Array(
                self.declared_scope
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect(),
            ),
        );
        // Serialize the outer map too — serde_json emits BTreeMap keys in
        // sorted order, giving a stable, deterministic byte stream.
        serde_json::to_vec(&Value::Object(map))
            .expect("canonical serialization is always valid JSON")
    }

    /// Human-readable render — the SAME renderer used for display and hashing.
    ///
    /// # Invariant
    /// `render()` and `canonical_bytes()` derive from the same canonical
    /// serialization — what the human sees is what is hashed. This method is
    /// deterministic: equal intents render identically.
    pub fn render(&self) -> String {
        let bytes = self.canonical_bytes();
        let canonical = String::from_utf8_lossy(&bytes);
        format!("{} {}", self.tool, canonical)
    }
}

/// Recursively normalize a JSON value for canonical serialization.
///
/// Object keys are sorted (byte-wise) at every level; arrays preserve order
/// (a `declared_scope` / args list is an ordered sequence). Primitive values
/// and the number formatting produced by serde_json are already canonical for
/// a given input.
fn sorted_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map
                .iter()
                .map(|(k, v)| (k.clone(), sorted_value(v)))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(entries.into_iter().collect())
        }
        Value::Array(items) => Value::Array(items.iter().map(sorted_value).collect()),
        other => other.clone(),
    }
}
