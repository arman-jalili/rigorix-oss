//! Enterprise Proxy — Conditionally forwards `rigorix_enterprise_*` tool calls
//! to the Rigorix Enterprise API via HTTP JSON-RPC.
//!
//! @canonical .pi/architecture/modules/enterprise-proxy.md
//! Implements: Contract Freeze — enterprise-proxy module root
//!
//! This is a **conditional module** — when no enterprise configuration is
//! present, zero enterprise code is loaded, and no `rigorix_enterprise_*`
//! tools appear in the MCP tool list. The enterprise proxy dynamically
//! discovers available enterprise tools during initialization.
//!
//! # Module Structure
//!
//! This module follows Clean Architecture with bounded context (DDD):
//!
//! - `enterprise-proxy/domain/` — EnterpriseProxy trait (aggregate root),
//!   SchemaCache struct (domain service), value objects (ProxyConfig,
//!   EnterpriseMetadata, JsonRpcRequest/Response), domain events, error types
//! - `enterprise-proxy/application/` — Service traits (ProxyInitializationService,
//!   EnterpriseToolRouter, SchemaCacheService), input/output DTOs, factory interfaces
//! - `enterprise-proxy/infrastructure/` — Repository interfaces for schema cache
//!   persistence, HTTP client contracts
//! - `enterprise-proxy/interfaces/` — MCP tool handler contracts and schemas
//!
//! # Architecture References
//!
//! | Component | Path (per architecture) | Canonical Section |
//! |-----------|------------------------|-------------------|
//! | EnterpriseProxy (aggregate root) | `src/enterprise-proxy/domain/entity.rs` | `.pi/architecture/modules/enterprise-proxy.md#enterpriseproxy` |
//! | SchemaCache (domain service) | `src/enterprise-proxy/domain/entity.rs` | `.pi/architecture/modules/enterprise-proxy.md#schemacache` |
//! | ProxyInitializationService | `src/enterprise-proxy/application/service.rs` | `.pi/architecture/modules/enterprise-proxy.md#initialization` |
//! | EnterpriseToolRouter | `src/enterprise-proxy/application/service.rs` | `.pi/architecture/modules/enterprise-proxy.md#routing` |
//!
//! # Dependencies
//!
//! - **Depends on:** MCP Server (via ToolRegistry registration)
//! - **Used by:** None directly (standalone proxying module)

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
