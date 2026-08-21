//! Shared construction of workflow-visible NIF result terms.

use beamr::atom::Atom;
use beamr::native::ProcessContext;
use beamr::term::Term;

/// Why a NIF is not producing its ordinary value term.
pub(super) enum NifRefusal {
    /// Report to the workflow as `{error, <<message>>}`; the term is built.
    Reported(Term),
    /// The result term could not be built. Leave by raising `reason`.
    Unbuildable(Term),
}

impl NifRefusal {
    pub(super) fn reported(term: Result<Term, Term>) -> Self {
        match term {
            Ok(term) => Self::Reported(term),
            Err(reason) => Self::Unbuildable(reason),
        }
    }

    pub(super) fn into_nif_result(self) -> Result<Term, Term> {
        match self {
            Self::Reported(term) => Ok(term),
            Self::Unbuildable(reason) => Err(reason),
        }
    }
}

/// Build `{tag, <<bytes>>}` on the calling process heap.
pub(super) fn tagged_result_term(
    ctx: &mut ProcessContext,
    tag: Atom,
    bytes: &[u8],
) -> Result<Term, Term> {
    let value = ctx.alloc_binary(bytes)?;
    ctx.alloc_tuple(&[Term::atom(tag), value])
}

pub(super) fn ok_result_term(ctx: &mut ProcessContext, bytes: &[u8]) -> Result<Term, Term> {
    tagged_result_term(ctx, Atom::OK, bytes)
}

pub(super) fn error_result_term(ctx: &mut ProcessContext, message: &str) -> Result<Term, Term> {
    tagged_result_term(ctx, Atom::ERROR, message.as_bytes())
}

/// Build a workflow refusal for a deliberate raise, or use the allocation reason.
pub(super) fn raise_reason(ctx: &mut ProcessContext, message: &str) -> Term {
    match error_result_term(ctx, message) {
        Ok(term) | Err(term) => term,
    }
}
