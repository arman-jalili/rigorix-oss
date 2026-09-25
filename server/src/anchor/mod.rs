//! ADR-016 Phase C anchor surface for `rigorix-server`.
//!
//! @canonical .pi/architecture/decisions/ADR-016-audit-integrity-anchor.md
//! Issue: #899 (OSS-AUDIT-C)
//!
//! The server composes the engine's shared [`AnchorRuntime`] (the MCP host
//! installs it from `RIGORIX_ANCHOR_URL` / `RIGORIX_ANCHOR_PUBLIC_KEY`). This
//! module exposes the active mode + verified head for `rigorix.system.version`
//! — the host itself never holds the anchor's private key (ADR-016 Phase D).
//!
//! [`AnchorRuntime`]: rigorix_engine::audit::infrastructure::anchor::AnchorRuntime

use rigorix_engine::audit::domain::anchor::AnchorMode;
use rigorix_engine::audit::infrastructure::anchor::{AnchorStatus, runtime_status};

/// The active anchor status (initialises the shared runtime from env).
pub fn status() -> AnchorStatus {
    runtime_status()
}

/// The wire label for a mode (`local_unanchored` | `anchored`).
pub fn mode_label(mode: AnchorMode) -> &'static str {
    match mode {
        AnchorMode::LocalUnanchored => "local_unanchored",
        AnchorMode::Anchored => "anchored",
    }
}
