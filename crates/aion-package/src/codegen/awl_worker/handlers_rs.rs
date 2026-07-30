//! Emission of `src/handlers.rs` — the one file of the scaffold the author
//! owns.
//!
//! Every stub FAILS its activity, terminally, with a message naming the
//! action. Not a `todo!()` and not a `panic!()` (both are denied by the lint
//! table the generated crate carries), and above all never a silent success:
//! an activity that reports a result nobody computed is the worst possible
//! outcome for a durable workflow, because the run continues on a lie that is
//! then recorded in history forever.
//!
//! Each stub carries the DOCUMENT's declared input and output schemas in its
//! documentation, which is what the author needs to fill the body in — and
//! what the server admits the advertisement against.

use std::fmt::Write as _;

use super::error::AwlScaffoldError;
use super::main_rs::handler_ident;
use super::plan::{ConnectionPlan, ServableAction};
use super::text::doc_lines;
use crate::canonical::CanonicalJson;

/// Emits the author-owned `handlers` module: one stub per servable action.
///
/// # Errors
///
/// Returns [`AwlScaffoldError::SchemaUnrenderable`] when a declared schema
/// cannot be rendered as JSON for the stub's documentation.
pub(super) fn emit(plan: &ConnectionPlan) -> Result<String, AwlScaffoldError> {
    let mut out = String::new();
    emit_header(&mut out, plan);
    for action in &plan.servable {
        emit_stub(&mut out, action)?;
    }
    Ok(out)
}

/// The module documentation: this file is yours, and what the stubs promise.
fn emit_header(out: &mut String, plan: &ConnectionPlan) {
    let _ = write!(
        out,
        "//! The activity BODIES this worker serves — THE FILE YOU OWN.\n\
         //!\n\
         //! `aion awl scaffold` writes this file ONCE and never rewrites it. Every other\n\
         //! file in this crate is regenerated from the document; this one is yours from the\n\
         //! moment it exists.\n\
         //!\n\
         //! Each stub below FAILS its activity terminally with a message naming the action,\n\
         //! so a stub that ships is loud at the first dispatch instead of reporting a result\n\
         //! nobody computed. A durable workflow records what an activity returns and never\n\
         //! runs it again, so a silent success is a lie written into history forever.\n\
         //!\n\
         //! The input arrives as JSON matching the document's declared input schema, and the\n\
         //! value you return must match its declared output schema — both are reproduced in\n\
         //! each stub's documentation. Those are the very schemas the server admits this\n\
         //! worker's advertisement against, so a shape you invent here is a decode failure\n\
         //! on the workflow's side, not a new contract.\n\
         //!\n\
         //! When the document GROWS an action, re-run `aion awl scaffold`: it rewrites the\n\
         //! generated files — which then reference a handler that is not here — and leaves\n\
         //! this file alone, so the build fails naming the function you owe. The worker also\n\
         //! refuses to start if a declared action is unserved, so neither route ends in a\n\
         //! parked dispatch.\n\
         //!\n\
         //! Serving the `{queue}` queue: {names}.\n\
         \n\
         use aion_worker::{{ActivityContext, ActivityFailure, HandlerFuture}};\n\
         use serde_json::Value;\n",
        queue = plan.task_queue,
        names = super::text::quoted_list(plan.servable.iter().map(|action| action.name.as_str())),
    );
}

/// One handler stub: its documented contract and its loud refusal.
fn emit_stub(out: &mut String, action: &ServableAction) -> Result<(), AwlScaffoldError> {
    let locality = action.node.as_deref().map_or_else(
        || {
            "no node, so its dispatch reaches every worker on the queue and every \
             connection this binary opens serves it"
                .to_owned()
        },
        |node| format!("node `{node}`, so only that node's connection serves it"),
    );
    let input = rendered_schema(action, "input", &action.input_schema)?;
    let output = rendered_schema(action, "output", &action.output_schema)?;
    let documentation = format!(
        "`{name}` — declared on {locality}.\n\
         \n\
         INPUT, as the document declares it:\n\
         \n\
         ```json\n\
         {input}\n\
         ```\n\
         \n\
         OUTPUT — the workflow decodes exactly this, so the value returned must match:\n\
         \n\
         ```json\n\
         {output}\n\
         ```",
        name = action.name,
    );
    out.push('\n');
    doc_lines(out, 0, &documentation);
    let _ = write!(
        out,
        "pub fn {ident}(input: Value, context: &ActivityContext) -> HandlerFuture<'_, Value> {{\n\
         \x20   Box::pin(async move {{\n\
         \x20       // Replace this body. `input` is the declared input above; return a `Value`\n\
         \x20       // matching the declared output, or an `ActivityFailure` classified\n\
         \x20       // `retryable` (the engine may try again) or `terminal` (it must not).\n\
         \x20       let _ = (input, context);\n\
         \x20       Err::<Value, ActivityFailure>(ActivityFailure::terminal(\n\
         \x20           \"activity `{name}` is not implemented: `aion awl scaffold` wrote this stub \\\n\
         \x20            and nothing has replaced it\",\n\
         \x20       ))\n\
         \x20   }})\n\
         }}\n",
        ident = handler_ident(&action.name),
        name = action.name,
    );
    Ok(())
}

/// Renders one declared schema for the stub's documentation.
///
/// Canonically encoded: the schema an author reads here is key-ordered the way
/// every other emitted artifact orders it, so re-generating the scaffold from
/// the same document cannot produce a different file.
fn rendered_schema(
    action: &ServableAction,
    role: &'static str,
    schema: &serde_json::Value,
) -> Result<String, AwlScaffoldError> {
    serde_json::to_string_pretty(&CanonicalJson::new(schema.clone())).map_err(|error| {
        AwlScaffoldError::SchemaUnrenderable {
            action: action.name.clone(),
            role,
            reason: error.to_string(),
        }
    })
}
