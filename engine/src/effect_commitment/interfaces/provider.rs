//! The provider-facing side of the commitment identity.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#providerreceiptreader
//! Implements: ISSUE-1163 — transmit the intent key to the provider
//! Issue: #1165
//! ADR-019: L3 is only reachable if the provider was given the key
//!
//! The receipt reader can look a receipt up by the intent's idempotency key.
//! That lookup can only succeed if the provider received the key when the
//! effect was sent. Until #1165 the engine never sent it, so the second form
//! was unreachable by construction.
//!
//! # Provider contract
//!
//! Only one transmission exists today:
//!
//! - **Process providers** invoked through `run_command` receive the key in
//!   `RIGORIX_EFFECT_IDEMPOTENCY_KEY` (`EFFECT_IDEMPOTENCY_ENV`). This is
//!   implemented in `exec_run_command` and asserted by
//!   `run_command_carries_the_effect_key_to_the_provider`.
//!
//! The HTTP half is **reserved, not implemented** (#1174). The engine's tools
//! are process sinks; no execution path sends an HTTP request, so claiming that
//! HTTP providers "receive" the header would describe a contract with no
//! sender. `EFFECT_IDEMPOTENCY_HEADER` names the header an HTTP provider
//! adapter must attach when one lands; until then it has no production user.
//!
//! Common requirements for whichever channel carries it:
//!
//! - The value is the intent-derived key, stable across retries of the same
//!   intent; a changed payload derives a different key.
//! - The provider MUST dedupe on the key and MUST expose it on the receipt
//!   (the reader queries `GET /v1/receipts?idempotency_key=<key>`).
//! - The key is not a secret, but it MUST NOT be logged together with request
//!   or response bodies.
//!
//! A provider that stores the key but does not dedupe leaves L3 as unreachable
//! as it is today. The contract is the deliverable, not just the header.

/// HTTP header carrying the intent-derived idempotency key.
///
/// **Reserved** (#1174): no execution path sends HTTP today. An HTTP provider
/// adapter must attach this header; the constant is the contract's vocabulary,
/// not evidence that the transmission exists.
pub const EFFECT_IDEMPOTENCY_HEADER: &str = "Idempotency-Key";

/// Environment variable carrying the key to a process provider.
pub const EFFECT_IDEMPOTENCY_ENV: &str = "RIGORIX_EFFECT_IDEMPOTENCY_KEY";
