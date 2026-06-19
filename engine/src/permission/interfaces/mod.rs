//! Interface adapters for the Permission Enforcer bounded context.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md
//! Implements: Contract Freeze — HTTP API endpoint contracts
//! Issue: issue-contract-freeze
//!
//! This module defines API contracts (HTTP, CLI, etc.) that external
//! actors use to interact with the permission enforcement system.
//!
//! # Permission API Context
//!
//! Permission is primarily triggered by:
//! 1. Tool call evaluation — every tool invocation passes through the enforcer
//! 2. File write boundary checks — workspace path validation
//! 3. Bash command classification — intent-aware gating
//!
//! The HTTP API provides external monitoring and override capabilities
//! (e.g., querying permission status, setting mode, evaluating tools).

pub mod http;
