//! Why the gate could not answer — every variant a refusal at the caller.
//!
//! @canonical .pi/architecture/modules/gate.md


/// Why the gate could not answer.
///
/// Every variant is a refusal at the caller (fail closed). Note what is *absent*: a
/// "no gate configured" variant. Absence is not an error — it is `GateVerdict::Dispatch`
/// with no findings, because the operator's decision not to gate is a decision.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GateError {
    /// The request carried no execution id.
    ///
    /// Not defaulted: a gate that minted or defaulted one would make the digest
    /// non-reproducible or collide two runs in one record (ADR-014 §2).
    #[error("the gate request carries no execution id; pin one (ADR-014 §2)")]
    UnpinnedExecution,

    /// The precondition gate could not evaluate.
    #[error("the precondition gate could not evaluate: {detail}")]
    Precondition {
        /// Redacted detail.
        detail: String,
    },

    /// The composition was not ready on its first poll.
    ///
    /// The slice's gate path must resolve without awaiting I/O: a port that awaits real
    /// I/O here is a build defect the caller is told about, not a hang or a panic
    /// (ADR-012 forbids the panic). See `evaluate_gate_sync`.
    #[error(
        "the gate composition was not ready on its first poll — a port on this path awaits I/O"
    )]
    NotReady,

    /// The envelope could not be assembled or signed.
    #[error("the gate decision could not be recorded: {detail}")]
    Envelope {
        /// Redacted detail.
        detail: String,
    },

    /// The requirements could not evaluate.
    #[error("the sequence-policy requirements could not evaluate: {detail}")]
    Requirements {
        /// Redacted detail.
        detail: String,
    },
}
