//! Infrastructure layer: building ports from operator configuration.
//!
//! @canonical .pi/architecture/modules/gate.md
//!
//! `GateSetup` reads `.rigorix/` and produces the two services the composition consults.
//! It is native-only — the whole module is gated by its parent — because the portable
//! slice takes its ports from the host, which is what lets a browser decide without a
//! filesystem.

pub mod setup;

pub use setup::GateSetup;
