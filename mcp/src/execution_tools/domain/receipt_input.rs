//! A receipt a caller sent through the MCP surface (#1630, ADR-025 D6).
//!
//! The tool input is JSON, and the crate's receipt type refuses unknown members
//! (`#[serde(deny_unknown_fields)]`) — so the boundary has to decide *where* a malformed document is
//! refused: here, with the crate's own `ReceiptRefusal`, before anything is dispatched.
//!
//! Deliberately not `Receipt` itself. A `serde_json::Value` that has not been through
//! `Receipt::from_document` is not a declaration, and a type that claims to be one invites a caller to
//! construct it. This wrapper says "bytes a client sent", and the only way out is the crate's door.

/// A receipt as it arrived on the wire.
#[derive(Debug, Clone)]
pub struct ReceiptInput {
    document: serde_json::Value,
}

impl ReceiptInput {
    /// Wrap the document the caller sent.
    #[must_use]
    pub fn new(document: serde_json::Value) -> Self {
        Self { document }
    }

    /// The document, for the crate's own parser.
    #[must_use]
    pub fn document(&self) -> &serde_json::Value {
        &self.document
    }

    /// Parse and validate through `Receipt::from_document` — the same door `run_receipt` uses, so a
    /// caller of either surface is refused by the same vocabulary with the same operands.
    ///
    /// # Errors
    /// The crate's refusals, verbatim. Nothing was dispatched and nothing will be: this is called
    /// before the enforcement pre-check, let alone before the graph.
    pub fn verify(
        &self,
    ) -> Result<
        (
            rigorix_engine::receipt::Receipt,
            Vec<rigorix_engine::receipt::ReceiptRefusal>,
        ),
        Vec<rigorix_engine::receipt::ReceiptRefusal>,
    > {
        match rigorix_engine::receipt::Receipt::from_document(&self.document.to_string()) {
            Ok(receipt) => Ok((receipt, Vec::new())),
            Err(refusals) => Err(refusals),
        }
    }
}

/// Which of the three "no digest" states a response is in (#1630).
///
/// The three are different facts and a caller acts on them differently: no declaration was made
/// (nothing to do), the declaration was accepted but the observation was not sealed (look at the
/// signing key and the state directory), or the store refused (fix the store). Collapsing them into
/// `None` is the state this exists to remove.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptState {
    /// The caller did not send a receipt. The run declared nothing.
    NotDeclared,
    /// A receipt was accepted, and the sealed observation was read back and reported.
    Sealed,
    /// A receipt was accepted, and the engine produced no attestation digest: either no signing key is
    /// wired or the store refused. The engine logs which; this value does not guess.
    NotSealed,
}

impl ReceiptState {
    /// The wire spelling, which a client branches on.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotDeclared => "not_declared",
            Self::Sealed => "sealed",
            Self::NotSealed => "not_sealed",
        }
    }

    /// What the engine produced says which state this is.
    ///
    /// `receipt_digest` is only `Some` when a declaration was admitted, so it is the first question —
    /// and the second is whether the observation was sealed.
    #[must_use]
    pub fn classify(receipt_digest: Option<&str>, attestation_digest: Option<&str>) -> Self {
        match (receipt_digest, attestation_digest) {
            (None, _) => Self::NotDeclared,
            (Some(_), Some(_)) => Self::Sealed,
            (Some(_), None) => Self::NotSealed,
        }
    }
}
