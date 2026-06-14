//! Tool System — Execution primitives for the task graph.
//!
//! @canonical .pi/architecture/modules/tool-system.md
//! Implements: Contract Freeze — module root for Tool trait and ToolRegistry
//! Issue: #124
//!
//! # Module Structure
//!
//! This module follows Clean Architecture with bounded contexts (DDD):
//!
//! - `domain/` — `Tool` trait, `ToolError`, `ToolEvent`, risk level mapping
//! - `application/` — Service traits (`ToolRegistryService`), DTOs,
//!   factory interfaces
//! - `infrastructure/` — Repository interfaces for tool storage
//! - `interfaces/` — HTTP API contracts for tool execution
//!
//! # Architecture References
//!
//! | Component | File (per architecture) | Canonical Section |
//! |-----------|------------------------|-------------------|
//! | Tool (trait) | `rigorix/src/tools/tool_trait.rs` | `.pi/architecture/modules/tool-system.md#trait` |
//! | ToolRegistry | `rigorix/src/tools/mod.rs` | `.pi/architecture/modules/tool-system.md#registry` |
//! | ToolInput | `rigorix/src/tools/mod.rs` | `.pi/architecture/modules/tool-system.md#input` |
//! | ToolResult | `rigorix/src/tools/mod.rs` | `.pi/architecture/modules/tool-system.md#result` |
//!
//! # Dependencies
//!
//! - **Depends on:** Risk Gating (RiskLevel, RiskConfig for gating decisions),
//!   Configuration (repo root path, path allowlists)
//! - **Used by:** Execution Engine (resolves tool via registry),
//!   Orchestrator (registers tools during build)

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
