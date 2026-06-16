//! Gate state tracking for the Risk Gating bounded context.
//!
//! @canonical .pi/architecture/modules/risk-gating.md
//! Implements: ISSUE-RISK-GATING-1 — Gate state management
//! Issue: #90
//!
//! Tracks pending gates that require user resolution (Medium → confirmation,
//! High → dry-run approval). Maintains a per-execution registry of unresolved
//! gates and provides resolution lookup.
//!
//! # Thread Safety
//! - Gate state is protected by `RwLock` for concurrent read/write
//! - All methods are safe to call from multiple tasks

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

use chrono::Utc;

use crate::risk_gating::application::dto::PendingGate;
use crate::risk_gating::domain::risk_level::{GatingAction, RiskLevel};

/// Thread-safe registry for tracking pending gates across executions.
pub struct GateStateRegistry {
    /// Auto-incrementing gate ID counter.
    next_gate_id: AtomicU64,

    /// Pending gates keyed by (execution_id, gate_id).
    pending: RwLock<HashMap<String, HashMap<String, PendingGate>>>,
}

impl GateStateRegistry {
    /// Create a new empty gate state registry.
    pub fn new() -> Self {
        Self {
            next_gate_id: AtomicU64::new(1),
            pending: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new pending gate and return its gate ID.
    pub fn register_gate(
        &self,
        execution_id: &str,
        node_id: &str,
        tool: &str,
        risk_level: RiskLevel,
        action: GatingAction,
    ) -> String {
        let gate_id = format!(
            "gate-{}-{}",
            execution_id,
            self.next_gate_id.fetch_add(1, Ordering::SeqCst)
        );

        let gate = PendingGate {
            gate_id: gate_id.clone(),
            execution_id: execution_id.to_string(),
            node_id: node_id.to_string(),
            tool: tool.to_string(),
            risk_level,
            action,
            created_at: Utc::now().to_rfc3339(),
            resolved: false,
        };

        let mut pending = self
            .pending
            .write()
            .expect("GateStateRegistry lock poisoned");
        pending
            .entry(execution_id.to_string())
            .or_default()
            .insert(gate_id.clone(), gate);

        gate_id
    }

    /// Resolve a pending gate. Returns the gate details if found.
    pub fn resolve_gate(&self, execution_id: &str, gate_id: &str) -> Option<PendingGate> {
        let mut pending = self
            .pending
            .write()
            .expect("GateStateRegistry lock poisoned");

        if let Some(exec_gates) = pending.get_mut(execution_id)
            && let Some(gate) = exec_gates.get_mut(gate_id) {
                gate.resolved = true;
                return Some(gate.clone());
            }
        None
    }

    /// Get all pending (unresolved) gates for an execution.
    pub fn pending_gates(&self, execution_id: &str) -> Vec<PendingGate> {
        let pending = self
            .pending
            .read()
            .expect("GateStateRegistry lock poisoned");

        if let Some(exec_gates) = pending.get(execution_id) {
            exec_gates
                .values()
                .filter(|g| !g.resolved)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Check if a gate exists and is still pending.
    pub fn is_gate_pending(&self, execution_id: &str, gate_id: &str) -> bool {
        let pending = self
            .pending
            .read()
            .expect("GateStateRegistry lock poisoned");

        pending
            .get(execution_id)
            .and_then(|gates| gates.get(gate_id))
            .is_some_and(|g| !g.resolved)
    }

    /// Clean up all gates for a completed execution.
    pub fn cleanup_execution(&self, execution_id: &str) {
        let mut pending = self
            .pending
            .write()
            .expect("GateStateRegistry lock poisoned");
        pending.remove(execution_id);
    }

    /// Get a gate by its ID across all executions.
    pub fn get_gate(&self, gate_id: &str) -> Option<PendingGate> {
        let pending = self
            .pending
            .read()
            .expect("GateStateRegistry lock poisoned");

        for exec_gates in pending.values() {
            if let Some(gate) = exec_gates.get(gate_id) {
                return Some(gate.clone());
            }
        }
        None
    }
}

impl Default for GateStateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_resolve_gate() {
        let registry = GateStateRegistry::new();
        let gate_id = registry.register_gate(
            "exec-1",
            "node-1",
            "file_write",
            RiskLevel::Medium,
            GatingAction::RequireConfirmation,
        );

        assert!(gate_id.starts_with("gate-exec-1-"));
        assert!(registry.is_gate_pending("exec-1", &gate_id));

        let resolved = registry.resolve_gate("exec-1", &gate_id);
        assert!(resolved.is_some());
        assert!(resolved.unwrap().resolved);

        assert!(!registry.is_gate_pending("exec-1", &gate_id));
    }

    #[test]
    fn test_pending_gates_empty_for_unknown_execution() {
        let registry = GateStateRegistry::new();
        let gates = registry.pending_gates("nonexistent");
        assert!(gates.is_empty());
    }

    #[test]
    fn test_cleanup_execution() {
        let registry = GateStateRegistry::new();
        registry.register_gate(
            "exec-1",
            "node-1",
            "file_write",
            RiskLevel::Medium,
            GatingAction::RequireConfirmation,
        );

        registry.cleanup_execution("exec-1");
        let gates = registry.pending_gates("exec-1");
        assert!(gates.is_empty());
    }

    #[test]
    fn test_get_gate_across_executions() {
        let registry = GateStateRegistry::new();
        let gate_id = registry.register_gate(
            "exec-1",
            "node-1",
            "bash",
            RiskLevel::High,
            GatingAction::DryRun,
        );

        let gate = registry.get_gate(&gate_id);
        assert!(gate.is_some());
        assert_eq!(gate.unwrap().tool, "bash");
    }

    #[test]
    fn test_resolve_nonexistent_gate() {
        let registry = GateStateRegistry::new();
        let resolved = registry.resolve_gate("exec-1", "nonexistent");
        assert!(resolved.is_none());
    }
}
