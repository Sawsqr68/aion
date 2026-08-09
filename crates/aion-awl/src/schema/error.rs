use thiserror::Error;

use crate::Span;

/// Failure to derive a JSON Schema from an AWL contract.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum SchemaError {
    /// The requested type is not declared in the document.
    #[error("no type named `{name}` is declared")]
    UnknownType {
        /// The requested type name.
        name: String,
        /// Span of the workflow header, anchoring the diagnostic.
        span: Span,
    },
    /// A schema import cannot resolve without the document's directory.
    #[error("cannot resolve imported schema `{path}` without the document's directory")]
    ImportUnresolved {
        /// Import path as written.
        path: String,
        /// Span of the import path literal.
        span: Span,
    },
    /// A schema import could not be read from disk.
    #[error("cannot read imported schema `{path}`: {detail}")]
    ImportUnreadable {
        /// Import path as written.
        path: String,
        /// Operating-system error detail.
        detail: String,
        /// Span of the import path literal.
        span: Span,
    },
    /// A schema door's JSON does not parse.
    #[error("schema for `{name}` is not valid JSON: {detail}")]
    InvalidJson {
        /// The declared type name.
        name: String,
        /// JSON parser detail.
        detail: String,
        /// Span of the declaration.
        span: Span,
    },
    /// The derived schema is well-formed JSON but is not a schema any
    /// validator can compile, so nothing could ever be checked against it.
    #[error(
        "the schema derived for `{name}` cannot be compiled into a validator, \
         so nothing would be checked against it: {detail}"
    )]
    UnusableSchema {
        /// The workflow or type the schema was derived for.
        name: String,
        /// The schema compiler's own reason.
        detail: String,
        /// Span of the declaration.
        span: Span,
    },
    /// A workflow-only contract was asked of a `worker` document, which
    /// declares no start contract, no outcomes and no queries.
    #[error(
        "`{name}` is a worker document: it implements the actions of task queue `{name}` \
         and declares no workflow, so it publishes no {contract}. Ask the `workflow` \
         document that calls this queue; a worker document's own contract is the schema \
         of each action, which `--type` derives."
    )]
    NotAWorkflow {
        /// The task queue the document serves.
        name: String,
        /// The workflow-only contract that was requested, named as an
        /// operator would name it ("start contract", "outcome envelope").
        contract: &'static str,
        /// Span of the document's opening declaration.
        span: Span,
    },
}

impl SchemaError {
    /// Span that anchors a compiler-style diagnostic.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::UnknownType { span, .. }
            | Self::ImportUnresolved { span, .. }
            | Self::ImportUnreadable { span, .. }
            | Self::InvalidJson { span, .. }
            | Self::UnusableSchema { span, .. }
            | Self::NotAWorkflow { span, .. } => *span,
        }
    }
}
