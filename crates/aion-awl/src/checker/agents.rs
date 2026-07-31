//! The `agent` marker rule.
//!
//! `agent` on an action declaration says one thing and one thing only: this
//! action's single `String` parameter carries a PROMPT and its `String` result
//! carries the REPLY. That is the whole of what an adapter needs in order to
//! hand the call to an agent harness instead of executing it as an ordinary
//! activity.
//!
//! Because the shape IS the meaning, the shape has to be enforced here. A
//! marker accepted on an action of any shape would be a marker that means
//! nothing — a name in the grammar that every reader assumes has force, and
//! that nothing gives force to. That is precisely the dormant surface this
//! marker exists to remove (the hand-maintained `AGENT_ACTIVITY_TYPES` list in
//! a worker binary), so accepting it loosely would re-create the defect one
//! layer up instead of closing it.

use crate::ast::{ActionBody, ActionDecl};

use super::context::{Callable, Ctx};
use super::types::Ty;

/// Refuse `agent` on an action whose shape cannot carry a prompt and a reply.
///
/// Called once per action declaration, with the callable already resolved so
/// the parameter and result types are the checker's own types rather than
/// unresolved syntax. Nothing is reported when the action does not carry the
/// marker.
pub(super) fn check(ctx: &mut Ctx<'_>, action: &ActionDecl, callable: &Callable) {
    let Some(marker) = action.config.as_ref().and_then(|config| config.agent) else {
        return;
    };

    // A prompt is one argument. Zero leaves the harness nothing to send; more
    // than one leaves it choosing, and a marker whose meaning depends on an
    // adapter's choice is not a contract.
    if callable.params.len() != 1 {
        ctx.error(
            marker,
            format!(
                "`agent` action `{}` takes {} parameters — an agent action takes exactly \
                 one `String`, the prompt; drop `agent` if this is an ordinary action, or \
                 carry the extra inputs inside the prompt string",
                action.name,
                callable.params.len()
            ),
        );
    } else if !is_string(&callable.params[0].ty) {
        ctx.error(
            marker,
            format!(
                "`agent` action `{}` takes `{}` — an agent action's one parameter is the \
                 `String` prompt handed to the harness; a structured input has no place to \
                 go on the wire to an agent",
                action.name, callable.params[0].ty
            ),
        );
    }

    if !is_string(&callable.returns) {
        ctx.error(
            marker,
            format!(
                "`agent` action `{}` returns `{}` — an agent action returns the harness's \
                 `String` reply; parse it downstream if the flow needs structure",
                action.name, callable.returns
            ),
        );
    }

    // A declared body is executed by the server itself. An `agent` action is
    // executed by a harness on a worker. Both at once is two implementations
    // of one action, and nothing in the system arbitrates between them.
    if let Some(ActionBody::Run { span, .. }) = &action.body {
        ctx.error(
            *span,
            format!(
                "action `{}` is marked `agent` and also declares a body — the server runs \
                 a declared body itself and never hands it to an agent harness; declare \
                 one or the other",
                action.name
            ),
        );
    }
}

/// Whether a resolved type is the builtin `String`.
///
/// Deliberately NOT tolerant of `String?`: an optional prompt is a prompt that
/// may be absent, and an absent prompt is not a call an agent can serve.
/// [`Ty::Unknown`] passes so a type that already failed to resolve reports once,
/// at its own site, rather than a second time as a shape complaint here.
fn is_string(ty: &Ty) -> bool {
    matches!(ty, Ty::Str | Ty::Unknown)
}
