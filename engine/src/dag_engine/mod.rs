//! DAG Engine — Template-driven DAG construction, validation, and planning.
//!
//! @canonical .pi/architecture/modules/dag-engine.md
//! Implements: TaskGraph — TaskGraph, DagGraphService, DagPlanningService
//! Issue: issue-taskgraph
//!
//! The DAG Engine compiles templates into executable Directed Acyclic Graphs.
//! It handles two-phase graph construction (add nodes → seal), topological
//! sorting (Kahn's algorithm), cycle detection, O(1) ready queue, and per-node
//! execution policies with retry configuration.
//!
//! # Design
//! - `domain/`: Core entities (TaskGraph, TaskNode, ExecutionPolicy, PlanDiff)
//! - `application/`: Service interfaces, implementations, DTOs, and factories
//! - `infrastructure/`: Repository interfaces for persistence
//! - `interfaces/`: HTTP API contracts
//!
//! Contracts defined in issue-contract-freeze are frozen.
//! Implementation satisfies those contracts.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;

#[cfg(test)]
pub(crate) mod tests;
