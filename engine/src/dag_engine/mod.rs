//! DAG Engine — Template-driven DAG construction, validation, and planning.
//!
//! @canonical .pi/architecture/modules/dag-engine.md
//! Implements: Contract Freeze — dag-engine public interface contracts
//! Issue: issue-contract-freeze
//!
//! The DAG Engine compiles templates into executable Directed Acyclic Graphs.
//! It handles two-phase graph construction (add nodes → seal), topological
//! sorting (Kahn's algorithm), cycle detection, O(1) ready queue, and per-node
//! execution policies with retry configuration.
//!
//! # Contract (Frozen)
//! - All public interfaces are defined in domain/ and application/
//! - DTOs in application/dto/ define input/output contracts
//! - HTTP contracts in interfaces/http/ define API surface
//! - Event payloads in domain/event/ define emitted events
//! - Repository interfaces in infrastructure/repository/
//!
//! No implementation code is permitted in this module — only contracts.
//! Implementation issues must satisfy these contracts.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
