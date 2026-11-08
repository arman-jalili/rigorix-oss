//! `rigorix-gate-wasi` — the gate as an artifact a non-JS host can load.
//!
//! @canonical .pi/architecture/decisions/ADR-022-binding-boundary.md
//! @canonical .pi/architecture/decisions/ADR-013-who-verifies.md
//! @canonical .pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md
//! Implements: the three-export ABI — `rigx_alloc`, `rigx_free`, `rigx_call`
//! Issue: #1465 (WASI-1)
//!
//! # Why this crate exists and `wasm/` did not grow a second entry
//!
//! The browser crate's four operations are the contract. A WASI host cannot call them
//! through `#[wasm_bindgen]`: that attribute layer *is* a JS import set, and the whole claim
//! of this artifact is that its import section is empty — "this module cannot read a file,
//! spawn a process, or open a socket" stated in a form a reviewer can check rather than in a
//! sentence a questionnaire has to believe (ADR-014, ADR-022).
//!
//! So the four operations stay one implementation, in `rigorix-gate-wasm`, which compiles
//! for `wasm32-wasip1` because its binding layer is gated on `target_os = "unknown"`. What
//! this crate adds is a **boundary**: three exports, one JSON entry, no state a host can
//! reach into.
//!
//! ```text
//! rigx_alloc(len) -> ptr          the host asks for a request buffer
//! rigx_call(ptr, len) -> ptr      ONE entry; the operation is named inside the JSON
//! rigx_free(ptr, len) -> ()       the host returns a buffer, or frees a response
//! ```
//!
//! A response is a buffer whose first four bytes are the little-endian length. An ABI with
//! one entry cannot grow a second dialect: `{"op":"contract_version"}` and
//! `{"op":"attempt_run",…}` are the same call with different JSON.
//!
//! # A trap is not a refusal (ADR-012)
//!
//! Malformed JSON, an unknown `op`, a truncated request and a buffer that was never
//! allocated all return `{"kind":"request_invalid",…}` and leave the instance running.
//! `panic = "abort"`: there is no unwinder on `wasm32-wasip1`, so a panic is a trap the host
//! sees, and the only way to keep the promise is to not panic — hence the deny-lints below
//! and a dispatch that has no fallible step it does not map.
//!
//! # Unsafe is three functions wide
//!
//! `rigx_alloc`, `rigx_free` and `rigx_call` are the crate's entire `unsafe` surface, and
//! they are `unsafe` for the reason any allocator boundary is: they turn a host-supplied
//! integer into a slice. Everything they build on is safe and tested natively.

#![deny(unsafe_code)]
// ADR-012 tier 2, and the same list the browser crate denies for the same reason: on a
// target with no unwinder a panic is a trap, so these are build failures rather than
// handled cases. `not(test)` because the lint attributes of a crate root do not reach the
// integration test crates, and a test legitimately asserts on a `Result`.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::todo,
        clippy::unimplemented,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::exit
    )
)]

pub mod abi;
pub mod memory;

pub use abi::{MAX_REQUEST_BYTES, dispatch};
pub use memory::{rigx_alloc, rigx_call, rigx_free};
