//! Interfaces layer module for the Planning Pipeline.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#interfaces
//! Implements: Contract Freeze — interface module exports
//! Issue: issue-contract-freeze
//!
//! This module exposes API contracts including HTTP endpoint definitions
//! for the planning pipeline.

pub mod http;

pub use http::*;
