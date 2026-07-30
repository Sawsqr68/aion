//! `send` statement checking: the declared outbound signal, the payload
//! against that declaration, and the target address.
//!
//! What this pass CAN prove and what it cannot is the whole point of the
//! feature, so it is worth stating plainly.
//!
//! The engine routes a signal by NAME. Most of the ways a send can go wrong
//! are already loud at runtime — an unresolvable target id and a target whose
//! run is terminal both come back as engine errors, and the lowering faults
//! the step on them. The ONE quiet case is a live target that never awaits
//! that name: `record_signal_received` succeeds, the payload lands in the
//! target's history, and nothing ever consumes it
//! (`aion/src/signal/router.rs`). That is the silent-failure class this
//! codebase refuses, so the language narrows it as far as a runtime address
//! allows —
//!
//!  * the name must be DECLARED outbound in this document's header, so a
//!    typo, or a name the author never meant to send, is a compile error;
//!  * the payload is checked against that declaration, so the bytes on the
//!    wire have a stated shape instead of an implicit one;
//!  * the target must be a `String` (a workflow id).
//!
//! What remains unprovable here is the far side's AGREEMENT: the target is a
//! runtime id, so the receiving workflow's own declarations are not in
//! scope. (A payload the receiver cannot decode is at least loud — it faults
//! the RECEIVER's step — but a name it never waits for is not.) Closing this
//! needs a spawn handle carrying the child's workflow type, which is a later
//! landing. The limit is written on the feature's face rather than left to
//! be discovered.

use crate::ast::{RoutePayload, SendStmt};
use crate::semantic::DeclarationKind;
use crate::spanned::Spanned;

use super::args::check_args;
use super::exprs::{View, type_of};
use super::types::{Ty, assignable, resolve};
use super::walk::{Scope, Walker};

pub(super) fn walk_send(w: &mut Walker<'_, '_>, scope: &Scope, send: &SendStmt) {
    let payload_ty = declared_payload(w, send);
    check_payload(w, scope, send, &payload_ty);
    check_target(w, scope, send);
}

/// Resolve the outbound signal's declared payload type, or report the
/// undeclared name.
fn declared_payload(w: &mut Walker<'_, '_>, send: &SendStmt) -> Ty {
    if let Some(ty) = w.ctx.sends.get(&send.signal).cloned() {
        if w.emit {
            w.ctx
                .semantic
                .reference(send.signal_span, DeclarationKind::Send, &send.signal);
            w.ctx.semantic.ty(send.signal_span, &ty.to_string());
        }
        return ty;
    }
    // A name declared as an INBOUND signal is the mistake worth naming:
    // `signal x` says what this workflow receives, which is silent about
    // what it may send.
    if let Some(inbound) = w.ctx.signals.get(&send.signal).cloned() {
        w.err(
            send.signal_span,
            format!(
                "`{}` is declared as a signal this workflow RECEIVES, not one it sends — \
                 a document states its outbound surface separately, so add \
                 `send {}: {inbound}` to the header (both directions may be declared)",
                send.signal, send.signal
            ),
        );
        return Ty::Unknown;
    }
    w.err(
        send.signal_span,
        format!(
            "no outbound signal named `{}` is declared in the workflow header — a send \
             names a signal the document declares with `send {}: <Type>`, so the payload \
             it puts on the wire has a stated shape",
            send.signal, send.signal
        ),
    );
    Ty::Unknown
}

/// Check the payload against the declared type, in the same three forms a
/// route payload takes.
fn check_payload(w: &mut Walker<'_, '_>, scope: &Scope, send: &SendStmt, ty: &Ty) {
    let view = View::plain(scope);
    let payload_span = send.payload_span.unwrap_or(send.signal_span);
    match &send.payload {
        Some(RoutePayload::Args(args)) => match resolve(ty, &w.ctx.types) {
            Ty::Record(record) => {
                let params: Vec<(String, Ty)> = record
                    .fields
                    .iter()
                    .map(|field| (field.name.clone(), field.ty.clone()))
                    .collect();
                check_args(
                    w,
                    view,
                    args,
                    &params,
                    &format!("signal `{}` (type {ty})", send.signal),
                    "field",
                    send.signal_span,
                );
            }
            Ty::Unknown => {}
            other => {
                w.err(
                    payload_span,
                    format!(
                        "signal `{}` carries {other} — named payload fields need a \
                         record-typed signal; send the value itself \
                         (`send {}(<value>) to …`)",
                        send.signal, send.signal
                    ),
                );
            }
        },
        Some(RoutePayload::Value(value)) => {
            let value_ty = type_of(w, view, value);
            if !assignable(&value_ty, ty, &w.ctx.types) {
                w.err(
                    value.span(),
                    format!(
                        "the payload value is {value_ty}, but signal `{}` carries {ty}",
                        send.signal
                    ),
                );
            }
        }
        // No parentheses is the payload-less form. Unlike a bare `route`,
        // it does NOT pick up a same-named binding: a signal name and a
        // binding name coinciding is a coincidence, and silently shipping
        // whatever that binding held is exactly the kind of implicit wiring
        // a send must not have.
        None => {
            if !matches!(resolve(ty, &w.ctx.types), Ty::Nil | Ty::Unknown) {
                w.err(
                    send.signal_span,
                    format!(
                        "signal `{}` carries {ty}, so the send must construct it — \
                         `send {}(…) to <workflow id>`; only a `Nil` payload is written bare",
                        send.signal, send.signal
                    ),
                );
            }
        }
    }
}

/// The target is a workflow id. Today that is any `String` the flow can
/// produce — `workflow.id` (a run addressing itself, or handed to a child as
/// an input so the child can signal back), an input, or a binding.
fn check_target(w: &mut Walker<'_, '_>, scope: &Scope, send: &SendStmt) {
    let view = View::plain(scope);
    let ty = type_of(w, view, &send.target);
    if !matches!(resolve(&ty, &w.ctx.types), Ty::Str | Ty::Unknown) {
        w.err(
            send.target.span(),
            format!(
                "a send addresses a workflow by id, which is a String — this target is \
                 {ty}"
            ),
        );
    }
}
