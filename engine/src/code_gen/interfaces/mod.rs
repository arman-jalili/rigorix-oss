//! Interface adapters for the Code Generation Pipeline bounded context.
//!
//! @canonical .pi/architecture/modules/code-generation.md
//! Implements: Contract Freeze — HTTP API endpoint contracts
//! Issue: #424
//!
//! This module defines API contracts (HTTP, CLI, etc.) that external
//! actors use to interact with the code generation pipeline.

pub mod http;
