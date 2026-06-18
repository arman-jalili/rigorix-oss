//! Code Graph — Dependency graph for code modules and their relationships.
//!
//! @canonical .pi/architecture/modules/code-graph.md
//! Implements: Contract Freeze — code-graph module root
//! Issue: issue-contract-freeze
//!
//! The CodeGraph module defines a dependency graph of code modules (nodes)
//! connected by relationships (edges). It provides construction, analysis,
//! persistence, and formatting services for code structure visualization
//! and dependency auditing.
//!
//! # Architecture
//! - `domain/` — Core entities: CodeGraph, ModuleNode, ModuleEdge
//! - `application/` — Service traits, DTOs, factory interfaces
//! - `infrastructure/` — Repository interfaces for persistence
//! - `interfaces/` — API contracts (HTTP)
//!
//! # Contract (Frozen)
//! - All interfaces defined in domain/ and application/
//! - DTO schemas in application/dto/
//! - API contracts in interfaces/http/
//! - Repository interfaces in infrastructure/repository/
//! - No implementation — only contracts and interfaces

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;

#[cfg(test)]
pub(crate) mod tests;
