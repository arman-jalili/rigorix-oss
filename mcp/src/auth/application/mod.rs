//! Application layer for the Auth bounded context.
//!
//! @canonical .pi/architecture/modules/auth.md#application
//! Implements: Contract Freeze — AuthService trait, DTOs, factory interfaces
//!
//! This module defines:
//! - Service traits (use cases): `AuthService` (login/poll/status/refresh/
//!   logout/attest)
//! - Input/Output DTOs with documented field names and types
//! - Factory interfaces for composing the service from its ports
//!
//! # Contract (Frozen)
//!
//! - All service methods are async (use async-trait)
//! - All public methods return domain error types (`AuthError`)
//! - DTOs document every field's name and type (auth.md API Endpoints table)
//! - Factory interfaces encapsulate complex construction
//! - Service trait signatures are frozen; the concrete `AuthServiceImpl`
//!   (application layer, ISSUE-AUTH-1) implements the behavior

pub mod dto;
pub mod factory;
pub mod service;
pub mod service_impl;

pub use dto::*;
pub use factory::*;
pub use service::*;
pub use service_impl::AuthServiceImpl;
