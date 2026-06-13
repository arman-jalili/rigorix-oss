//! Interface adapters for the Cancellation bounded context.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — HTTP API endpoint contracts
//! Issue: issue-contract-freeze
//!
//! This module defines API contracts (HTTP, CLI, etc.) that external
//! actors use to interact with the cancellation system.
//!
//! # Cancellation API Context
//!
//! Cancellation is primarily triggered by:
//! 1. OS signals (SIGINT, SIGTERM) — handled by the orchestrator
//! 2. TUI user commands — passed through the orchestrator
//! 3. Internal enforcement limits — coordinated by the orchestrator
//!
//! The HTTP API provides an additional remote cancellation surface
//! for programmatic control (e.g., CI/CD pipelines, web dashboards).

pub mod http;
