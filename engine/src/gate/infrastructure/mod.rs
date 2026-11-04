//! Infrastructure layer: building ports from operator configuration.
//!
//! @canonical .pi/architecture/modules/gate.md
//!
//! `GateSetup` reads `.rigorix/` and produces the two services the composition consults.
//! It is native-only: the portable slice takes its ports from the host instead, which is
//! what lets a browser decide without a filesystem.

#[cfg(not(feature = "portable"))]
pub mod setup;

#[cfg(not(feature = "portable"))]
pub use setup::GateSetup;
