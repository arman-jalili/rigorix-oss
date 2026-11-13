//! The receipt module's infrastructure layer — deliberately empty, and saying so.
//!
//! @canonical .pi/architecture/modules/receipt.md
//! Implements: the layer declaration the conformance check requires (`import_boundaries`)
//!
//! An attestation is persisted **beside the audit envelope**, in the same repository, as a new
//! record: the envelope already owns the durable, append-only store and the signing key, and a
//! second store would be a second place for the two to disagree. So this module has no adapter of
//! its own yet, and the port it will use is `audit`'s.
//!
//! Writing an adapter here now would be the wrong shape: the location rule (ADR-024 §6) is a
//! *property of the store*, and storing attestations in a place the agent can write is the failure
//! the rule exists to prevent. When the record lands, it lands there.
