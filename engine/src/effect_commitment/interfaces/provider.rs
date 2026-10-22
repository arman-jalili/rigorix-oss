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
//! # Provider contract (Frozen)
//!
//! - **HTTP providers** receive the key in the `Idempotency-Key` request
//!   header (`EFFECT_IDEMPOTENCY_HEADER`).
//! - **Process providers** invoked through `run_command` receive it in
//!   `RIGORIX_EFFECT_IDEMPOTENCY_KEY` (`EFFECT_IDEMPOTENCY_ENV`).
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
pub const EFFECT_IDEMPOTENCY_HEADER: &str = "Idempotency-Key";

/// Environment variable carrying the key to a process provider.
pub const EFFECT_IDEMPOTENCY_ENV: &str = "RIGORIX_EFFECT_IDEMPOTENCY_KEY";
