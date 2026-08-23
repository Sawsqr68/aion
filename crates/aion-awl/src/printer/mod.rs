//! The canonical printer: one shape per document, lossless comments and
//! doc lines, `parse ∘ print = id` and `print ∘ parse ∘ print = print`.

mod assembly;
mod document;
mod exprs;
mod harness;
mod steps;

pub use document::print;
pub use exprs::expr_text;

// The command-declaration family renders its own clauses but must escape a
// string, a duration and a type exactly as the rest of the language does.
// Crate-visible rather than re-implemented: a second escaper is a second
// answer to "what is this document's canonical form".
pub(crate) use exprs::{duration_text, string_literal, type_ref_text};
