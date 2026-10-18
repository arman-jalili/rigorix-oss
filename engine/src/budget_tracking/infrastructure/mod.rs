//! Infrastructure layer for the Budget Tracking bounded context.
//!
//! @canonical .pi/architecture/modules/budget-tracking.md
//! Implements: ISSUE-BUDGET-REPOSITORY-PORT-UNWIRED — no persistence port
//! Issue: #1016
//!
//! # No persistence port
//!
//! Budgets are **per-execution and ephemeral**: `LlmBudgetImpl` owns its
//! counters and nothing in the OSS engine persists them. Durable or
//! cross-execution accounting belongs to the capacity ledger
//! (`crate::capacity_ledger`), whose HTTP adapter delegates to the enterprise
//! ledger service.
//!
//! The former `LlmBudgetRepository` port and its in-memory adapter had no
//! production caller — a reader could reasonably conclude that budgets were
//! durable. Both were removed rather than left as a dead seam (#1016).
