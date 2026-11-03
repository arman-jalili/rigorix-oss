//! The **Portable Session** context, as one module — the crate's public façade.
//!
//! @canonical .pi/architecture/modules/portable-session.md
//! @canonical .pi/architecture/decisions/ADR-001-architecture-pattern.md
//! Implements: Contract Freeze — context façade
//! Issue: #1294 (contract freeze)
//!
//! # Why this file exists
//!
//! `rigorix-gate-wasm` owns exactly one bounded context, so the context and the crate are
//! coextensive: the layers live at the crate root (`domain/`, `application/`,
//! `infrastructure/`, `interfaces/`) instead of under a per-context directory the way
//! `rigorix-engine` does it with 36 contexts.
//!
//! That leaves the architecture doc (`portable-session.md`) with no path of its own. This
//! module closes the gap: it is the single name a reader (or the canonical-reference
//! validator) can follow from the doc to the code, and it is the only place the layers
//! are presented as one thing.
//!
//! # Contract (Frozen)
//! - Re-exports only: no logic, no new types, no second entry point
//! - Hosts should keep using the layer paths (`domain::…`, `application::…`) for
//!   precision; this façade exists for orientation, not for a second API

pub use crate::application;
pub use crate::domain;
pub use crate::infrastructure;
pub use crate::interfaces;
