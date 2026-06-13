//! Interface adapters for the Audit bounded context.
//!
//! @canonical .pi/architecture/modules/audit.md
//! Implements: Contract Freeze — HTTP API endpoint contracts
//! Issue: #13
//!
//! This module defines API contracts (HTTP, CLI, etc.) that external
//! actors use to interact with the audit system.

pub mod http;
