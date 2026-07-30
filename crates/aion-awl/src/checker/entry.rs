//! Public checking entry points, orchestrating the declaration pass, the
//! graph pass, and the flow walk.

use std::path::Path;

use crate::ast::Document;
use crate::semantic::SemanticAnalysis;

use super::context::{Ctx, Flow};
use super::error::CheckError;
use super::{consts, decls, durations, graph, queries, subflows, walk};

/// Typecheck a parsed document with no document directory: schema imports
/// (`type X = schema("file")`) cannot resolve and are reported as errors.
/// Prefer [`check_in`] when the document's directory is known.
#[must_use]
pub fn check(document: &Document) -> Vec<CheckError> {
    run(document, None).0
}

/// Typecheck a parsed document, resolving schema imports relative to `root`
/// (the directory containing the `.awl` file).
#[must_use]
pub fn check_in(document: &Document, root: &Path) -> Vec<CheckError> {
    run(document, Some(root)).0
}

pub(crate) fn analyze(document: &Document, root: Option<&Path>) -> SemanticAnalysis {
    let (diagnostics, builder) = run(document, root);
    SemanticAnalysis::from_parts(diagnostics, builder)
}

fn run(document: &Document, root: Option<&Path>) -> (Vec<CheckError>, crate::semantic::Builder) {
    let mut ctx = Ctx::new(document, root);
    if let Some(timeout) = &document.timeout
        && (timeout.negative || timeout.duration.magnitude == 0)
    {
        ctx.error(
            timeout.span,
            "workflow `timeout` must be a positive duration",
        );
    }
    if let Some(timeout) = &document.timeout
        && timeout.duration.checked_duration().is_none()
    {
        ctx.error(timeout.span, "workflow `timeout` is too large");
    }
    durations::run(&mut ctx);
    decls::run(&mut ctx);
    consts::run(&mut ctx);
    queries::check_declared_answers(&mut ctx);
    subflows::run(&mut ctx);
    // Every flow — the workflow's own steps, then each subflow's — gets the
    // full graph pass and flow walk over its own inputs and outcomes.
    let workflow_flow = Flow::workflow(&ctx);
    check_flow(&mut ctx, &workflow_flow);
    for subflow in &document.subflows {
        let flow = Flow::subflow(&ctx, subflow);
        check_flow(&mut ctx, &flow);
    }
    let mut errors = ctx.errors;
    // The contract this document DERIVES has to be enforceable, not merely
    // well-typed. A derived schema no validator can compile does not fail at
    // the server — admission logs a warning and lets the payload through
    // unchecked — so the only place an author can meet it is here. Run it
    // last, and only over an otherwise-clean document: derivation reports one
    // failure at a time, and it would otherwise crowd out the specific type
    // errors that are far more likely to be the real cause.
    if errors.is_empty() {
        errors.extend(contract_derivation_errors(document, root));
    }
    errors.sort_by(|a, b| {
        a.span
            .start
            .cmp(&b.span.start)
            .then_with(|| a.message.cmp(&b.message))
    });
    errors.dedup();
    (errors, ctx.semantic)
}

/// Derives every schema this document publishes as its contract and reports
/// any that cannot be compiled into a validator.
///
/// Without a document directory a schema IMPORT cannot be read, so import
/// failures here would be an artefact of how the checker was called rather
/// than anything about the document. Those are left to the caller that has the
/// directory; a schema that is unusable is reported either way.
fn contract_derivation_errors(document: &Document, root: Option<&Path>) -> Vec<CheckError> {
    let derived = match root {
        Some(root) => vec![
            crate::schema_for_workflow_in(document, root),
            crate::schema_for_outcomes_in(document, root),
            crate::schema_for_queries_in(document, root),
        ],
        None => vec![
            crate::schema_for_workflow(document),
            crate::schema_for_outcomes(document),
            crate::schema_for_queries(document),
        ],
    };
    derived
        .into_iter()
        .filter_map(|result| match result {
            Err(error @ crate::SchemaError::UnusableSchema { .. }) => Some(CheckError {
                span: error.span(),
                message: error.to_string(),
            }),
            Ok(_) | Err(_) => None,
        })
        .collect()
}

/// Run the graph pass and (when the `after` edges are acyclic) the flow
/// walk over one flow.
fn check_flow<'a>(ctx: &mut Ctx<'a>, flow: &Flow<'a>) {
    let step_graph = graph::build(ctx, flow);
    if !step_graph.after_cycle {
        walk::run(ctx, flow, &step_graph);
    }
}
