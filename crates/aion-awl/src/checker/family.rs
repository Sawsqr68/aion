//! The family rules: which declarations belong to a `workflow` document and
//! which to a `worker` one.
//!
//! Both families parse through one grammar, so a `step` in a worker document
//! and a `run json` body in a workflow document are well-formed parses. They
//! are refused HERE, where the diagnostic can say which family the file is
//! in, why the declaration cannot live in it, and where it does belong —
//! which a parse error at the first stray token could not.
//!
//! The split is not bureaucratic. A workflow document's `worker` block is a
//! typed REQUIREMENT on a queue; a worker document is an IMPLEMENTATION of
//! one. That is why every action here must carry a body, and why the delivery
//! keys (`timeout`, `retry`, `node`, `advisory`, `agent`) are refused: they
//! configure how a CALLER dispatches an action, and the caller is the
//! workflow document. A rule stated in two files with nothing forcing them
//! equal has already drifted.

use crate::Span;
use crate::ast::{ActionBody, ConfigLine, DocumentFamily, RunCapture};

use super::context::Ctx;

/// Run the family pass over the document in `ctx`.
pub(super) fn run(ctx: &mut Ctx<'_>) {
    match ctx.doc.family {
        DocumentFamily::Workflow => check_workflow_document(ctx),
        DocumentFamily::Worker => check_worker_document(ctx),
    }
}

/// A workflow document's declared bodies are run BY THE SERVER, whose
/// declared-command dispatcher hands back the command's outcome record and
/// nothing else. A capture selector would therefore promise a result the
/// dispatch cannot produce.
fn check_workflow_document(ctx: &mut Ctx<'_>) {
    let mut refusals = Vec::new();
    for worker in &ctx.doc.workers {
        for action in &worker.actions {
            let Some(ActionBody::Run {
                capture,
                capture_span,
                span,
                ..
            }) = &action.body
            else {
                continue;
            };
            let Some(word) = capture.word() else {
                continue;
            };
            refusals.push((
                capture_span.unwrap_or(*span),
                format!(
                    "action `{name}` declares a `run {word}` body, and a workflow document's \
                     declared bodies are executed by the server, which returns the command's \
                     outcome record. Write the bare `run \"…\"` form and take `exit_code`, \
                     `stdout` and `stderr` from it, or move this action into a `worker` \
                     document, which serves the queue itself and can capture the command's \
                     output as the action's result.",
                    name = action.name
                ),
            ));
        }
    }
    for (span, message) in refusals {
        ctx.error(span, message);
    }
}

/// A worker document serves one queue: the actions its opening declaration
/// names, each with the body that runs it, and nothing else.
fn check_worker_document(ctx: &mut Ctx<'_>) {
    let mut refusals = Vec::new();
    let queue = ctx.doc.name.clone();
    for step in &ctx.doc.steps {
        refusals.push((
            step.span,
            format!(
                "a worker document implements a queue's actions and runs no flow, so step \
                 `{name}` cannot live in it; move it into the `workflow` document that calls \
                 queue `{queue}`.",
                name = step.name
            ),
        ));
    }
    for child in &ctx.doc.children {
        refusals.push((
            child.span,
            format!(
                "`child {name}` declares a workflow this flow delegates to, and a worker \
                 document runs no flow; move the declaration into the `workflow` document \
                 that spawns it.",
                name = child.name
            ),
        ));
    }
    for subflow in &ctx.doc.subflows {
        refusals.push((
            subflow.span,
            format!(
                "`subflow {name}` is a flow container, and a worker document runs no flow; \
                 move it into the `workflow` document whose steps use it.",
                name = subflow.name
            ),
        ));
    }
    for decl in &ctx.doc.consts {
        refusals.push((
            decl.span,
            format!(
                "`const {name}` is a compile-time value for a workflow's expressions, and a \
                 worker document has no expressions; write the value into the command that \
                 needs it, or move the const into the `workflow` document that uses it.",
                name = decl.name
            ),
        ));
    }
    for worker in ctx.doc.workers.iter().skip(1) {
        refusals.push((
            worker.span,
            format!(
                "a worker document serves exactly one queue — the one its opening `worker` \
                 declaration names (`{queue}`) — so the `worker {name}` block here has no \
                 meaning; give queue `{name}` its own worker document.",
                name = worker.name
            ),
        ));
    }
    if let Some(worker) = ctx.doc.workers.first() {
        for action in &worker.actions {
            if action.body.is_none() {
                refusals.push((
                    action.span,
                    format!(
                        "action `{name}` declares no body, so this worker document says \
                         nothing about how queue `{queue}` serves it. Give it a `run` body, \
                         or move the declaration into the `worker` block of the workflow \
                         document that calls it, where a bodyless action is a requirement on \
                         an out-of-band worker.",
                        name = action.name
                    ),
                ));
            }
            if let Some(config) = &action.config {
                refusals.extend(delivery_key_refusals(&action.name, config));
            }
        }
    }
    for (span, message) in refusals {
        ctx.error(span, message);
    }
}

/// One refusal per delivery key the action's config line carries.
///
/// Each key configures how a CALLER dispatches the action — the engine reads
/// them off the calling workflow's deployed contract — so restating one here
/// would put the same fact in two documents with nothing holding them equal.
fn delivery_key_refusals(action: &str, config: &ConfigLine) -> Vec<(Span, String)> {
    let mut refusals = Vec::new();
    let mut refuse = |span: Span, key: &str, what: &str| {
        refusals.push((
            span,
            format!(
                "`{key}` on action `{action}` {what}, so it belongs to the action's \
                 declaration in the `workflow` document that calls it, not to the \
                 implementation here; remove it from this worker document."
            ),
        ));
    };
    if let Some(span) = config.advisory {
        refuse(
            span,
            "advisory",
            "classes the action as a side channel of the calling step",
        );
    }
    if let Some(span) = config.agent {
        refuse(
            span,
            "agent",
            "tells a dispatch to hand the call to an agent harness",
        );
    }
    if let Some(node) = &config.node {
        refuse(node.span, "node", "routes the dispatch to a node");
    }
    if let Some(timeout) = &config.timeout {
        refuse(
            timeout.span,
            "timeout",
            "bounds the dispatch the engine schedules",
        );
    }
    if let Some(retry) = &config.retry {
        refuse(
            retry_span(retry),
            "retry",
            "is the policy the engine's retry loop consumes",
        );
    }
    refusals
}

/// The span covering a retry clause.
const fn retry_span(retry: &crate::ast::RetrySpec) -> Span {
    match retry {
        crate::ast::RetrySpec::Every { span, .. } | crate::ast::RetrySpec::Backoff { span, .. } => {
            *span
        }
    }
}

/// Whether `capture` may appear in `family`. Used by the body checker to
/// decide which result rule applies without restating the family rule.
pub(super) const fn capture_is_declarable(family: DocumentFamily, capture: RunCapture) -> bool {
    match family {
        DocumentFamily::Workflow => matches!(capture, RunCapture::Outcome),
        DocumentFamily::Worker => true,
    }
}
