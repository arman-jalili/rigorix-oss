//! dag_engine tests (split by concern, #917).

use super::*;

#[test]
fn test_impact_level_ordering() {
    assert!(ImpactLevel::None < ImpactLevel::Low);
    assert!(ImpactLevel::Low < ImpactLevel::Medium);
    assert!(ImpactLevel::Medium < ImpactLevel::High);
    assert!(ImpactLevel::High < ImpactLevel::Breaking);
}

#[test]
fn test_impact_level_max() {
    assert_eq!(
        ImpactLevel::None.max(ImpactLevel::Breaking),
        ImpactLevel::Breaking
    );
    assert_eq!(ImpactLevel::High.max(ImpactLevel::Low), ImpactLevel::High);
}

#[test]
fn test_impact_level_as_str() {
    assert_eq!(ImpactLevel::None.as_str(), "none");
    assert_eq!(ImpactLevel::Low.as_str(), "low");
    assert_eq!(ImpactLevel::Medium.as_str(), "medium");
    assert_eq!(ImpactLevel::High.as_str(), "high");
    assert_eq!(ImpactLevel::Breaking.as_str(), "breaking");
}

// ---------------------------------------------------------------------------
// PlanDiff
// ---------------------------------------------------------------------------

#[test]
fn test_plan_diff_identical_plans() {
    let id = Uuid::new_v4();
    let node = make_node(id, "build", vec![]);
    let old_nodes = [node.clone()];
    let new_nodes = [node];
    let diff = PlanDiff::compute(&old_nodes, &new_nodes);
    assert_eq!(diff.added.len(), 0);
    assert_eq!(diff.removed.len(), 0);
    assert_eq!(diff.modified.len(), 0);
    assert_eq!(diff.unchanged.len(), 1);
    assert_eq!(diff.impact_level, ImpactLevel::None);
}

#[test]
fn test_plan_diff_added_node() {
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let old = vec![make_node(a, "build", vec![])];
    let new = vec![make_node(a, "build", vec![]), make_node(b, "test", vec![a])];
    let diff = PlanDiff::compute(&old, &new);
    assert_eq!(diff.added.len(), 1);
    assert_eq!(diff.removed.len(), 0);
    assert_eq!(diff.impact_level, ImpactLevel::Breaking);
}

#[test]
fn test_plan_diff_removed_node() {
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let old = vec![make_node(a, "build", vec![]), make_node(b, "test", vec![a])];
    let new = vec![make_node(a, "build", vec![])];
    let diff = PlanDiff::compute(&old, &new);
    assert_eq!(diff.added.len(), 0);
    assert_eq!(diff.removed.len(), 1);
    assert_eq!(diff.impact_level, ImpactLevel::Breaking);
}

#[test]
fn test_plan_diff_modified_tool() {
    let id = Uuid::new_v4();
    let old = vec![make_node_with_tool(id, "build", "cargo build", vec![])];
    let new = vec![make_node_with_tool(id, "build", "make", vec![])];
    let diff = PlanDiff::compute(&old, &new);
    assert_eq!(diff.modified.len(), 1);
    assert_eq!(diff.impact_level, ImpactLevel::High);
}

#[test]
fn test_plan_diff_modified_dependencies() {
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    let old = vec![make_node_with_tool(a, "A", "tool", vec![b])];
    let new = vec![make_node_with_tool(a, "A", "tool", vec![c])];
    let diff = PlanDiff::compute(&old, &new);
    assert_eq!(diff.modified.len(), 1);
    assert_eq!(diff.impact_level, ImpactLevel::Breaking);
}

// ---------------------------------------------------------------------------
// DagGraphService Integration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_planning_service_impact_breaking() {
    let service = DagPlanningServiceImpl::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let old = vec![make_node(a, "build", vec![])];
    let new = vec![make_node(a, "build", vec![]), make_node(b, "test", vec![a])];

    let result = service.compute_impact(old, new).await.unwrap();
    assert_eq!(result.impact_level, ImpactLevel::Breaking);
    assert!(result.summary.contains("Breaking"));
}

// ---------------------------------------------------------------------------
// Service — Additional Tests
// ---------------------------------------------------------------------------
