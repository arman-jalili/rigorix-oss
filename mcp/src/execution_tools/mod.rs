//! Execution Tools — Bridges MCP tool calls to rigorix-engine.
//!
//! @canonical .pi/architecture/modules/execution-tools.md
//! Implements: Contract Freeze — execution-tools module root
//!
//! The Execution Tools module is the primary value-add of the MCP Gateway.
//! AI assistants get deterministic execution through Rigorix via three MCP tools:
//!
//! - `rigorix_execute` — Execute a structured plan through rigorix-engine
//! - `rigorix_validate_plan` — Pre-flight validation against enforcement policies
//! - `rigorix_check_enforcement` — Check current budget and limit status
//!
//! # Module Structure
//!
//! This module follows Clean Architecture with bounded context (DDD):
//!
//! - `execution-tools/domain/` — EngineFacade trait (aggregate root), PlanTemplate,
//!   StepDefinition, execution result types, domain events, error types
//! - `execution-tools/application/` — Service traits (ExecuteHandler, ValidatePlanHandler,
//!   CheckEnforcementHandler), input/output DTOs, factory interfaces
//! - `execution-tools/infrastructure/` — Repository interfaces for execution persistence
//! - `execution-tools/interfaces/` — MCP tool handler contracts and schemas
//!
//! # Architecture References
//!
//! | Component | Path (per architecture) | Canonical Section |
//! |-----------|------------------------|-------------------|
//! | EngineFacade (aggregate root) | `src/execution-tools/domain/entity.rs` | `.pi/architecture/modules/execution-tools.md#enginefacade` |
//! | ExecuteHandler (domain service) | `src/execution-tools/application/service.rs` | `.pi/architecture/modules/execution-tools.md#executehandler` |
//! | ValidatePlanHandler (domain service) | `src/execution-tools/application/service.rs` | `.pi/architecture/modules/execution-tools.md#validateplanhandler` |
//! | CheckEnforcementHandler (domain service) | `src/execution-tools/application/service.rs` | `.pi/architecture/modules/execution-tools.md#checkenforcementhandler` |
//!
//! # Dependencies
//!
//! - **Depends on:** None (execution-tools is self-contained)
//! - **Used by:** MCP Server (via ToolRegistry registration), Audit Tools, Template Tools
//!
//! # Contract (Frozen)
//!
//! - All public interfaces are frozen — no additions without ADR approval
//! - Domain types are pure data with serde Serialize/Deserialize
//! - Service traits are async (async-trait) and return domain error types
//! - Repository interfaces abstract all persistence concerns
//! - MCP tool handler contracts are framework-agnostic

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;

#[cfg(test)]
pub mod tests;
