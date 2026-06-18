//! Plan comparison and impact classification for audit trails.
//!
//! @canonical .pi/architecture/modules/dag-engine.md#plan
//! Implements: Contract Freeze — PlanDiff, ImpactLevel domain entities
//! Issue: issue-contract-freeze
//!
//! Defines structured comparison between two execution plans for audit:
//! - `PlanDiff`: A structured diff between two plans (before/after)
//! - `ImpactLevel`: Classification of how impactful a plan change is
//!
//! # Contract (Frozen)
//! - PlanDiff captures added, removed, modified, and unchanged nodes
//! - ImpactLevel is an ordinal classification (None → Low → Medium → High → Breaking)
//! - Comparison is structural (node identity by UUID), not textual

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::graph::TaskNode;

// ---------------------------------------------------------------------------
// ImpactLevel — Impact Classification for Plan Changes
// ---------------------------------------------------------------------------

/// Classification of how impactful a plan change is.
///
/// Used in audit trails and approval workflows to determine review
/// requirements before executing a modified plan.
///
/// # Contract (Frozen)
/// - `None`: No changes detected
/// - `Low`: Cosmetic or non-functional changes (e.g., intent text, reordering)
/// - `Medium`: Behavioural changes within the same scope (e.g., new tool binding)
/// - `High`: Structural changes (e.g., added/removed nodes, changed dependencies)
/// - `Breaking`: Changes that invalidate previous results or require re-validation
///
/// Ordinal ordering: None < Low < Medium < High < Breaking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ImpactLevel {
    /// No changes detected.
    None,
    /// Cosmetic or non-functional changes.
    Low,
    /// Behavioural changes within the same scope.
    Medium,
    /// Structural changes (added/removed nodes, changed dependencies).
    High,
    /// Changes that invalidate previous results or require re-validation.
    Breaking,
}

impl ImpactLevel {
    /// Returns the canonical snake_case name of this impact level.
    pub fn as_str(&self) -> &'static str {
        match self {
            ImpactLevel::None => "none",
            ImpactLevel::Low => "low",
            ImpactLevel::Medium => "medium",
            ImpactLevel::High => "high",
            ImpactLevel::Breaking => "breaking",
        }
    }

    /// Returns the maximum of two impact levels.
    pub fn max(self, other: ImpactLevel) -> ImpactLevel {
        if self > other { self } else { other }
    }
}

// ---------------------------------------------------------------------------
// PlanDiff — Structured Plan Comparison
// ---------------------------------------------------------------------------

/// Structured comparison between two execution plans (before/after).
///
/// Captures which nodes were added, removed, modified, or unchanged
/// between two versions of a plan. Used for audit trails, approval
/// workflows, and impact analysis.
///
/// # Contract (Frozen)
/// - Comparison is by node UUID (structural identity)
/// - Each section contains the full TaskNode for context
/// - ImpactLevel is computed from the structural changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanDiff {
    /// Nodes present in the new plan but not in the old plan.
    pub added: Vec<TaskNode>,

    /// Nodes present in the old plan but not in the new plan.
    pub removed: Vec<TaskNode>,

    /// Nodes present in both plans but with different properties.
    pub modified: Vec<NodeDiff>,

    /// Nodes present in both plans with identical properties.
    pub unchanged: Vec<TaskNode>,

    /// Computed impact level based on the changes.
    pub impact_level: ImpactLevel,
}

impl PlanDiff {
    /// Compute a PlanDiff between two lists of nodes (old vs new).
    ///
    /// Nodes are matched by UUID. A node is:
    /// - **Added** if its UUID exists in `new_nodes` but not in `old_nodes`
    /// - **Removed** if its UUID exists in `old_nodes` but not in `new_nodes`
    /// - **Modified** if its UUID exists in both but any field differs
    /// - **Unchanged** if its UUID exists in both and all fields match
    ///
    /// The impact level is auto-computed:
    /// - `Breaking` if nodes were added, removed, or dependencies changed
    /// - `High` if tool/policy/validation changed
    /// - `Medium` if only intent changed
    /// - `None` if completely identical
    pub fn compute(old_nodes: &[TaskNode], new_nodes: &[TaskNode]) -> Self {
        let old_by_id: std::collections::HashMap<Uuid, &TaskNode> =
            old_nodes.iter().map(|n| (n.id, n)).collect();
        let new_by_id: std::collections::HashMap<Uuid, &TaskNode> =
            new_nodes.iter().map(|n| (n.id, n)).collect();

        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut modified = Vec::new();
        let mut unchanged = Vec::new();

        for new_node in new_nodes {
            match old_by_id.get(&new_node.id) {
                None => added.push(new_node.clone()),
                Some(old_node) => {
                    if old_node.tool != new_node.tool
                        || old_node.dependencies != new_node.dependencies
                        || old_node.policy != new_node.policy
                        || old_node.name != new_node.name
                        || old_node.intent != new_node.intent
                    {
                        modified.push(NodeDiff {
                            node_id: new_node.id,
                            name: new_node.name.clone(),
                            tool: new_node.tool.clone(),
                            old_tool: old_node.tool.clone(),
                            old_dependencies: old_node.dependencies.clone(),
                            new_dependencies: new_node.dependencies.clone(),
                            old_intent: old_node.intent.clone(),
                            new_intent: new_node.intent.clone(),
                        });
                    } else {
                        unchanged.push(new_node.clone());
                    }
                }
            }
        }

        for old_node in old_nodes {
            if !new_by_id.contains_key(&old_node.id) {
                removed.push(old_node.clone());
            }
        }

        let impact_level =
            Self::compute_impact_level(!added.is_empty(), !removed.is_empty(), &modified);

        Self {
            added,
            removed,
            modified,
            unchanged,
            impact_level,
        }
    }

    fn compute_impact_level(
        has_added: bool,
        has_removed: bool,
        modified: &[NodeDiff],
    ) -> ImpactLevel {
        if has_added || has_removed {
            return ImpactLevel::Breaking;
        }

        let has_structure_changed = modified
            .iter()
            .any(|d| d.old_dependencies != d.new_dependencies);
        if has_structure_changed {
            return ImpactLevel::Breaking;
        }

        let has_tool_changed = modified.iter().any(|d| d.old_tool != d.tool);
        if has_tool_changed {
            return ImpactLevel::High;
        }

        if modified.is_empty() {
            return ImpactLevel::None;
        }

        ImpactLevel::Medium
    }
}

/// Diff of a single node between two plan versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDiff {
    /// The node ID that was modified.
    pub node_id: Uuid,

    /// Current name of the node.
    pub name: String,

    /// Current tool binding.
    pub tool: String,

    /// Previous tool binding (for change tracking).
    pub old_tool: String,

    /// Previous dependency list.
    pub old_dependencies: Vec<Uuid>,

    /// Current dependency list.
    pub new_dependencies: Vec<Uuid>,

    /// Previous intent description.
    pub old_intent: String,

    /// Current intent description.
    pub new_intent: String,
}
