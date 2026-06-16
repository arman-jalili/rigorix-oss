//! Repository interfaces for the CLI Planning module.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md
//! Implements: Contract Freeze — PlanningRepository trait
//! Issue: issue-contract-freeze

use async_trait::async_trait;

use crate::planning::domain::PlanningCliError;

#[async_trait]
pub trait PlanningRepository: Send + Sync {
    async fn store_plan_result(
        &self,
        intent: &str,
        template_id: &str,
        confidence: f64,
    ) -> Result<(), PlanningCliError>;

    async fn get_recent_plans(
        &self,
        limit: usize,
    ) -> Result<Vec<(String, String, f64)>, PlanningCliError>;

    async fn clear(&self) -> Result<(), PlanningCliError>;
}
