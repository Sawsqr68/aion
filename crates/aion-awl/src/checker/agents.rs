//! The `agent` marker rule.
//!
//! `agent` on an action declaration says one thing and one thing only: this
//! action carries a PROMPT in a `String` parameter and its result is the
//! canonical agent-outcome record — a declared record with exactly four
//! fields, `text: String` (the whole accumulated output), `final_message: String`
//! (the last completed agent answer alone), `stop_reason: String` (the canonical
//! `snake_case` stop reason), and `session_id: String` (the handle of the conversation that produced the
//! outcome — a fact minted by the harness, not a promise that the session
//! can be resumed). That is the
//! whole of what an adapter needs in order to hand the call to an agent
//! harness instead of executing it as an ordinary activity, and it is the
//! shape every harness emits from its `wait_result` boundary (task #181's
//! ruling: the record is defined by aion, and every harness maps its native
//! outcome onto it — the old `-> String` rule had universalized one harness
//! path's shape and flattened the stop reason out of every other's).
//!
//! Because the shape IS the meaning, the shape has to be enforced here. A
//! marker accepted on an action of any shape would be a marker that means
//! nothing — a name in the grammar that every reader assumes has force, and
//! that nothing gives force to. That is precisely the dormant surface this
//! marker exists to remove (the hand-maintained `AGENT_ACTIVITY_TYPES` list in
//! a worker binary), so accepting it loosely would re-create the defect one
//! layer up instead of closing it.
//!
//! The author declares the record type themselves, under any name — the
//! `RunOutcome` / DECLARED-COMMANDS precedent. The checker demands the
//! STRUCTURE, not a blessed name.

use crate::ast::{ActionBody, ActionDecl};

use super::context::{Callable, Ctx};
use super::types::{RecordTy, Ty, TypeTable, resolve};

/// Refuse `agent` on an action whose shape cannot carry a prompt and the
/// canonical agent-outcome record.
///
/// Called once per action declaration, with the callable already resolved so
/// the parameter and result types are the checker's own types rather than
/// unresolved syntax. Nothing is reported when the action does not carry the
/// marker.
pub(super) fn check(
    ctx: &mut Ctx<'_>,
    action: &ActionDecl,
    callable: &Callable,
    bound_directory: Option<&str>,
) {
    let Some(marker) = action.config.as_ref().and_then(|config| config.agent) else {
        return;
    };

    check_prompt(ctx, action, callable, bound_directory, marker);

    if !returns_agent_outcome_record(&callable.returns, &ctx.types) {
        ctx.error(
            marker,
            format!(
                "`agent` action `{}` returns `{}` — an agent action returns the canonical \
                 agent-outcome record: a declared record with exactly four fields, \
                 `text: String`, `final_message: String`, `stop_reason: String`, and \
                 `session_id: String`; every harness emits that record, so declare it (any type name) and read \
                 `.text` where the flow needs the reply",
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

/// The prompt rule: exactly one `String` parameter that is not the harness's
/// working directory.
///
/// The prompt is defined by SUBTRACTION rather than by position, because the
/// two shapes an agent action can have differ only by the directory: a harness
/// naming a literal tree serves actions of one parameter, and a harness taking
/// each attempt's tree from the job serves actions that declare that parameter
/// too. Counting what remains once the directory is removed states one rule for
/// both, instead of an arity that has to be re-derived per shape — and it means
/// an action that never declares the bound parameter earns exactly one refusal,
/// the harness pass's, rather than a second one here saying the same thing in
/// different words.
fn check_prompt(
    ctx: &mut Ctx<'_>,
    action: &ActionDecl,
    callable: &Callable,
    bound_directory: Option<&str>,
    marker: crate::Span,
) {
    let prompts = callable
        .params
        .iter()
        .filter(|param| Some(param.name.as_str()) != bound_directory)
        .collect::<Vec<_>>();

    // A prompt is one argument. Zero leaves the harness nothing to send; more
    // than one leaves it choosing, and a marker whose meaning depends on an
    // adapter's choice is not a contract.
    if prompts.len() != 1 {
        let message = match bound_directory {
            None => format!(
                "`agent` action `{}` takes {} parameters — an agent action takes exactly \
                 one `String`, the prompt; drop `agent` if this is an ordinary action, or \
                 carry the extra inputs inside the prompt string",
                action.name,
                prompts.len()
            ),
            Some(directory) => format!(
                "`agent` action `{}` declares {} parameters besides `{directory}` — its \
                 worker's harness takes each attempt's working directory from \
                 `{directory}`, so this action takes that `String` and exactly one more, \
                 the prompt; carry any further inputs inside the prompt string",
                action.name,
                prompts.len()
            ),
        };
        ctx.error(marker, message);
        return;
    }

    let prompt = prompts[0];
    if !is_string(&prompt.ty) {
        let message = match bound_directory {
            None => format!(
                "`agent` action `{}` takes `{}` — an agent action's one parameter is the \
                 `String` prompt handed to the harness; a structured input has no place to \
                 go on the wire to an agent",
                action.name, prompt.ty
            ),
            Some(directory) => format!(
                "`agent` action `{}` declares its prompt parameter `{}` as `{}` — with the \
                 working directory taken from `{directory}`, the parameter that remains is \
                 the `String` prompt handed to the harness; a structured input has no place \
                 to go on the wire to an agent",
                action.name, prompt.name, prompt.ty
            ),
        };
        ctx.error(marker, message);
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

/// Whether the declared return type is (structurally) the canonical
/// agent-outcome record: exactly the four fields `text: String`, `final_message: String`,
/// `stop_reason: String`, and `session_id: String`, in any order, under any declared name.
///
/// The check is structural through the type table — a `Named` reference
/// resolves to its definition, and each field's own type resolves too, so a
/// projected schema record of the same shape satisfies it exactly as a
/// declared shorthand record does. [`Ty::Unknown`] passes so a return type
/// that already failed to resolve reports once, at its own site.
fn returns_agent_outcome_record(returns: &Ty, table: &TypeTable) -> bool {
    match resolve(returns, table) {
        Ty::Unknown => true,
        Ty::Record(record) => {
            record.fields.len() == 4
                && field_is_string(&record, "text", table)
                && field_is_string(&record, "final_message", table)
                && field_is_string(&record, "stop_reason", table)
                && field_is_string(&record, "session_id", table)
        }
        _ => false,
    }
}

/// Whether `record` carries a field `name` whose type resolves to `String`.
///
/// `String?` is deliberately refused: all four record fields are always present on the wire — an
/// empty reply or final message is the honest empty string, never an absent field — so an optional
/// field would declare an absence that cannot happen.
fn field_is_string(record: &RecordTy, name: &str, table: &TypeTable) -> bool {
    record
        .field(name)
        .is_some_and(|field| matches!(resolve(&field.ty, table), Ty::Str | Ty::Unknown))
}
