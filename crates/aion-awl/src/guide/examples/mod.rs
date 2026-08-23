//! The example corpus: every glossary entry's complete demonstration
//! documents, one file per vocabulary cluster.
//!
//! Each constant is a slice of whole `.awl` documents for one word. A word
//! with several genuinely distinct forms or contexts carries one document per
//! form; a word with exactly one honest form carries one document. All of
//! them pass the crate's real parse and check pipeline — the guide tests
//! refuse any that do not. The strings sit at column zero so the AWL
//! indentation inside them is the document's own.

mod decl_commands;
mod decl_document;
mod decl_recipes;
mod decl_workers;
mod flow_graph;
mod flow_iteration;
mod flow_routing;
mod values;

pub(crate) use decl_commands::{ADMITTING_COMMAND, REFUSING_COMMAND};
pub(crate) use decl_document::{
    CONST, FALSE, INPUT, JSON, OF, SCHEMA, SIGNAL, TRUE, TYPE, WORKFLOW,
};
pub(crate) use decl_recipes::{ASSEMBLY_SOURCES, RECIPE_PARAMETERS, RECIPE_STEPS};
pub(crate) use decl_workers::{
    ACTION, ADVISORY, AGENT, BACKOFF, CHILD, EVERY, FALLBACK, NODE, RETRY, RUN, SPAWN, SUBFLOW,
    TIMEOUT, WORKER,
};
pub(crate) use flow_graph::{
    AFTER, COLLECT, DISTRIBUTE, FORK, IN, JOIN, SEQUENCE, SEQUENTIAL, STEP,
};
pub(crate) use flow_iteration::{COUNTING, LOOP, MAX, ON, SLEEP, UNTIL, VISITS, WAIT};
pub(crate) use flow_routing::{
    ANSWER, FAILURE, OTHERWISE, OUTCOME, QUERY, ROUTE, SEND, SUCCESS, TO, WHEN,
};
pub(crate) use values::{
    ABSENT, ALL, AND, ANY, COUNT, EMPTY, FILTER, IS, MAP, NOT, OR, PRESENT, SORT,
};
