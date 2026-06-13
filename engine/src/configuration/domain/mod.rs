//! Domain entities and interfaces for the Configuration bounded context.
//!
//! @canonical .pi/architecture/modules/configuration.md#domain
//! Implements: Contract Freeze — domain entities Config, Secret, ConfigurationError, ConfigurationEvent
//! Issue: #2
//!
//! This module defines the core domain types — `Config`, `Secret`, and all
//! sub-configuration structs. These are pure domain objects with no framework
//! dependencies. They serve as the frozen contract that all implementation
//! must satisfy.
//!
//! # Contract Freeze
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces

pub mod config;
pub mod error;
pub mod event;
pub mod secret;

pub use config::*;
pub use error::ConfigurationError;
pub use secret::Secret;
