//! Audit Tools — Bridges MCP tool calls to rigorix-engine audit subsystem.
//!
//! @canonical .pi/architecture/modules/audit-tools.md
//! Implements: Contract Freeze — audit-tools module root
//!
//! The Audit Tools module provides read-only access to execution audit records
//! via three MCP tools:
//!
//! - `rigorix_read_audit` — Read an audit record by execution ID
//! - `rigorix_list_audits` — List recent audit records with filtering
//! - `rigorix_audit_summary` — Generate aggregate audit statistics
//!
//! # Module Structure
//!
//! This module follows Clean Architecture with bounded context (DDD):
//!
//! - `audit_tools/domain/` — AuditQueryService trait (aggregate root), AuditFilter,
//!   AuditSummary value objects, domain events, error types
//! - `audit_tools/application/` — Service traits (ReadAuditHandler, ListAuditsHandler,
//!   AuditSummaryHandler), input/output DTOs, factory interfaces
//! - `audit_tools/infrastructure/` — Repository interfaces for audit persistence
//! - `audit_tools/interfaces/` — MCP tool handler contracts and schemas
//!
//! # Architecture References
//!
//! | Component | Path (per architecture) | Canonical Section |
//! |-----------|------------------------|-------------------|
//! | AuditQueryService (aggregate root) | `src/audit_tools/domain/entity.rs` | `.pi/architecture/modules/audit-tools.md#auditqueryservice` |
//! | ReadAuditHandler (domain service) | `src/audit_tools/application/service.rs` | `.pi/architecture/modules/audit-tools.md#readaudithandler` |
//! | ListAuditsHandler (domain service) | `src/audit_tools/application/service.rs` | `.pi/architecture/modules/audit-tools.md#listauditshandler` |
//! | AuditSummaryHandler (domain service) | `src/audit_tools/application/service.rs` | `.pi/architecture/modules/audit-tools.md#auditsummaryhandler` |
//! | AuditFormatter (domain service) | `src/audit_tools/domain/entity.rs` | `.pi/architecture/modules/audit-tools.md#auditformatter` |
//!
//! # Dependencies
//!
//! - **Depends on:** MCP Server (via ToolRegistry registration), Execution Tools (shares EngineFacade)
//! - **Used by:** None directly (leaf handler)
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
