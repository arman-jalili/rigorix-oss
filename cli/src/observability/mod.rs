//! Observability module — tracing, health checks, and event schemas.
//!
//! @canonical .pi/architecture/modules/observability.md
//! Implements: Contract Freeze — CLI Observability module (interfaces only)
//! Issue: issue-contract-freeze
//!
//! Provides TracingInitializer trait, tracing implementation, and
//! observability event schemas.
//!
//! # Architecture (Clean Architecture layers)
//!
//! ```text
//! observability/
//! ├── domain/           # ObservabilityCliError, ObservabilityEvent
//! │   ├── mod.rs
//! │   ├── error.rs      # ObservabilityCliError enum
//! │   └── event/        # ObservabilityEvent payload schemas
//! │       └── mod.rs
//! ├── application/      # Service traits, DTO schemas
//! │   ├── mod.rs
//! │   ├── service.rs    # TracingInitializer trait
//! │   └── dto/          # InitTracing, HealthCheck, Metrics DTOs
//! │       └── mod.rs
//! ├── infrastructure/   # Trait implementations, repository interfaces
//! │   ├── mod.rs
//! │   ├── observability.rs           # Re-exports TracingInitializer
//! │   ├── tracing.rs                 # Tracing init implementation
//! │   └── repository/                # ObservabilityCliRepository trait
//! │       └── mod.rs
//! └── interfaces/       # HTTP API contracts
//!     ├── mod.rs
//!     └── http/         # Endpoint definitions, request/response schemas
//!         └── mod.rs
//! ```

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
