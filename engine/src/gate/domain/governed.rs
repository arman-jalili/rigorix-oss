//! `Governed` — the witness that an orchestrator was built out of the assembly (#1626, ADR-025 D2).
//!
//! @canonical .pi/architecture/decisions/ADR-025-one-governed-assembly.md
//! Issue: #1626
//!
//! # What a witness is for
//!
//! "This engine consults ADR-017's gate at dispatch" was a property of a *source file*: the constructor
//! accepted a caller-built execution service, and whatever gates that service carried were the caller's
//! business. A type decides what can be written; a source guard decides what was written. This module is
//! the type, and [`crate::orchestrator::application::orchestrator_impl::OrchestratorServiceImpl`] now
//! requires it.
//!
//! # What it contains
//!
//! Nothing. Its whole content is "some code that owns the seam order said yes", which is why it is not
//! `Copy`, not `Default`, not constructible from a field, and not serialisable — a witness that can be
//! produced by writing a literal is a comment.
//!
//! # The two producers
//!
//! - [`Governed::witness`] — `pub(crate)`, called by the assembly and by nothing else. One producer is
//!   what makes this a witness rather than a flag.
//! - [`crate::orchestrator::application::orchestrator_impl::OrchestratorServiceImpl::ungoverned`] — the
//!   **named** alternative for a caller that legitimately needs an engine with no gate: unit tests, the
//!   portable slice, a deliberately ungoverned probe. The name is the documentation, and
//!   `engine/tests/one_assembly.rs` counts its call sites so the list cannot grow quietly.
//!
//! # Why a guard as well
//!
//! A type cannot reach `cfg`-gated code, macro-generated code, or a host that adds a dependency and
//! writes its own import. That is ADR-025 D2's second half, and it is why both instruments exist.

/// Evidence that an orchestrator was constructed by the governed assembly.
///
/// The field is private and the only constructor in the crate is [`Governed::witness`], so the type
/// cannot be named into existence elsewhere: `Governed { .. }` does not compile outside this module, and
/// neither does `Default::default()`.
///
/// # The property, tested by the compiler
///
/// A doctest is compiled as an external crate, which makes it the one instrument that can test a
/// privacy property. This block is the claim — an ungated orchestrator cannot be built by writing a
/// literal:
///
/// ```compile_fail
/// use rigorix_engine::gate::domain::Governed;
///
/// // `_private` is private, so there is no literal to write. This is what makes the type a witness
/// // and not a flag; if this block ever compiles, ADR-025 D2 has been undone.
/// let ungated = Governed { _private: () };
/// ```
///
/// And this block is the *attribution*: naming the type and passing it across a function boundary are
/// both allowed, so the failure above is the private field and not a wrong path or a missing
/// re-export. Without it, `compile_fail` would pass for any reason at all, including a typo.
///
/// ```
/// use rigorix_engine::gate::domain::Governed;
///
/// fn accepts_the_witness(_governed: Governed) {}
/// ```
#[derive(Debug)]
pub struct Governed {
    /// Private on purpose. Its value is irrelevant; its *existence* is the claim, and a unit field
    /// would let a caller write `Governed(())`.
    _private: (),
}

impl Governed {
    /// The assembly's assertion that it attached the seams.
    ///
    /// `pub(crate)` and not `pub`: exactly one function in this crate may say this, and it is
    /// `gate::infrastructure::assembly::assemble`. A host outside the crate obtains a governed engine
    /// by *calling* the assembly, which returns the orchestrator the witness produced — so the witness
    /// is never a value a host holds, only one the engine's own code passes along.
    #[must_use]
    pub(crate) fn witness() -> Self {
        Self { _private: () }
    }
}
