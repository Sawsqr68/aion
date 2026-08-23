//! The rev-2 parser: token stream in, canonical workflow model out, with
//! compiler-quality diagnostics on source-correct spans.

mod action_body;
mod args;
mod declarations;
mod document;
mod error;
mod exprs;
mod flow;
mod harness;
mod hints;
mod query;
mod send;
mod statements;
mod steps;
mod stream;
mod types;
mod workers;

pub use document::parse;
pub use error::ParseError;

// The command-declaration family (`crate::command`) is a sibling module with
// its own parser over the same token stream, so the stream, the shared
// diagnostic vocabulary, and the type-reference reader are crate-visible.
// Promoted rather than duplicated: a second `Stream` or a second
// `parse_type_ref` would be a second account of the grammar.
pub(crate) use stream::{Stream, describe};
pub(crate) use types::{parse_type_ref, type_ref_span};
