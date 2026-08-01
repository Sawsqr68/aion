//! The `lower` entry point: run the emitter's shared planning passes (D-BC1),
//! assemble the module skeleton, fill region bodies, and return the
//! `MirModule`. Deferred shapes surface as `LowerError::Unsupported` — a
//! BC-2-scope marker distinct from a D-BC3 parity refusal.
//!
//! There is no longer a document-level staging gate here. The B4 gate held
//! the rev-3 constructs the direct path had not yet mirrored, and it emptied
//! construct by construct — `distribute`/`sequence` regions, `collect` steps
//! and `max … visits` with the fan-out parity landing, value route payloads
//! (`route out(<value>)`) with the value-payload landing, and `subflow`
//! declarations with the subflow landing, which was the last one it held. The
//! refusals that remain are per-construct and anchored where the construct
//! actually is (`step`'s `substeps`) plus the one shared `crate::unlowered`
//! refusal both backends keep. `flow`'s `parallel region` refusal is gone:
//! dependency-parallel layers lower through `layers`/`parallel`.

use std::fmt;
use std::mem;
use std::path::Path;

use crate::ast::Document;
use crate::emitter::{prepare, snake};

use super::super::unit::MirModule;
use super::build;
use super::ctx::Ctx;
use super::flow;

/// A lowering failure. `Unsupported` marks a shape this BC-2 increment does not
/// yet lower (NOT a reference refusal); `Planning` wraps an emitter planning
/// error (a genuine refusal or a document that did not check cleanly).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerError {
    Unsupported { shape: String, span: crate::Span },
    Message { message: String, span: crate::Span },
    Planning { message: String },
}

impl LowerError {
    pub(super) fn unsupported(shape: &str, span: crate::Span) -> Self {
        Self::Unsupported {
            shape: shape.to_owned(),
            span,
        }
    }

    pub(super) fn new(span: crate::Span, message: impl Into<String>) -> Self {
        Self::Message {
            message: message.into(),
            span,
        }
    }

    /// Preserve the retired refusal's source anchor while narrowing its shape.
    pub(super) fn reanchor_unsupported(self, anchor: crate::Span) -> Self {
        match self {
            Self::Unsupported { shape, .. } => Self::Unsupported {
                shape,
                span: anchor,
            },
            other => other,
        }
    }
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { shape, span } => {
                write!(f, "BC-2 does not yet lower {shape} (line {})", span.line)
            }
            Self::Message { message, span } => write!(f, "{message} (line {})", span.line),
            Self::Planning { message } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for LowerError {}

/// Lower a checked document to its MIR module.
///
/// # Errors
///
/// Returns [`LowerError::Planning`] for a document the shared passes refuse or
/// that did not check cleanly, and [`LowerError::Unsupported`] for a shape this
/// BC-2 increment does not yet cover.
pub fn lower(document: &Document, root: Option<&Path>) -> Result<MirModule, LowerError> {
    // Lowering is defined only for documents that check cleanly: fold-time
    // const substitution is name-based and relies on the checker's
    // invariants (no shadowed consts, no input/signal collisions).
    let diagnostics = match root {
        Some(root) => crate::checker::check_in(document, root),
        None => crate::checker::check(document),
    };
    if let Some(first) = diagnostics.first() {
        return Err(LowerError::Message {
            message: format!("document does not check cleanly: {}", first.message),
            span: first.span,
        });
    }
    // The one rev-3 refusal both backends keep: substep `after` would drop
    // on the floor — refuse honestly, with a span, before planning.
    if let Some((span, what)) = crate::unlowered::first_unlowered(document) {
        return Err(LowerError::Unsupported {
            shape: format!("{what} — a later landing carries them"),
            span,
        });
    }
    // Fold the B1 ergonomics vocabulary down to plain literals, then shape
    // the rev-3 flow constructs (regions, subflows, visit counters) into
    // the planned form both backends lower.
    let shaped =
        crate::emitter::shape_document(document, root).map_err(|error| LowerError::Planning {
            message: error.to_string(),
        })?;
    let (emitter, plans) = prepare(&shaped, root).map_err(|error| LowerError::Planning {
        message: error.to_string(),
    })?;
    let document = emitter.document;
    let module_name = snake(&document.name);
    let source = format!("{module_name}.awl");

    let mut ctx = Ctx::new(&emitter, &plans, module_name.clone());
    let mut skeleton = build::skeleton(&mut ctx)?;
    ctx.set_dynamic_start(skeleton.plan.dynamic_start);
    let mut slots = super::slots::Slots {
        loops: super::loops::LoopSlots::new(skeleton.plan.loops.clone()),
        forks: super::slots::ForkSlots::new(skeleton.plan.forks.clone()),
        waits: super::slots::WaitSlots::new(skeleton.plan.waits.clone()),
        compensations: super::slots::CompensationSlots::new(skeleton.plan.compensations.clone()),
    };
    let host_flow = flow::FlowCtx {
        steps: &document.steps,
        regions: emitter.host_regions,
        plan: &plans.host,
        fns: &skeleton.plan.host,
        bindings: &emitter.bindings,
        exit: None,
        prefix: String::new(),
        label: None,
        ret_ty: super::super::tydesc::TyDesc::Result(
            Box::new(build::output_tydesc(&ctx)),
            Box::new(super::super::tydesc::TyDesc::AwlError),
        ),
    };
    let host_env = flow::FlowEnv {
        plan: &skeleton.plan,
        flow: &host_flow,
    };
    flow::lower_flow_fns(&mut ctx, host_env, &mut skeleton.functions, &mut slots)?;
    // Each subflow lowers once (wrapper + chain fns), in declaration order,
    // right after the host chains — then each per-item member flow, ascending
    // region id. This is exactly the order `plan_slots` reserved the slots in
    // and the order the reference `emitter/flows.rs::emit_nested` emits them.
    super::nested::lower_subflow_flows(
        &mut ctx,
        &skeleton.plan,
        &mut skeleton.functions,
        &mut slots,
    )?;
    super::nested::lower_nested_flows(
        &mut ctx,
        &skeleton.plan,
        &mut skeleton.functions,
        &mut slots,
    )?;
    // Loop bodies fill their skeleton-reserved slots after every chain fn;
    // fork-lifted bodies follow after every loop fn, wait-lifted bodies
    // after every fork fn, and lifted compensation bodies after every wait
    // fn (the reserved order).
    slots.loops.append_into(&mut skeleton.functions)?;
    slots.forks.append_into(&mut skeleton.functions)?;
    slots.waits.append_into(&mut skeleton.functions)?;
    slots.compensations.append_into(&mut skeleton.functions)?;
    // Implicit-child adapter shells follow every compensation slot (ascending region
    // id), before the fixed helpers.
    for shell in super::nested::adapter_shells(&ctx, &skeleton.plan)? {
        skeleton.functions.push(shell);
    }
    // The shared dead-body function (T-DEAD) is a real, sidecar-visible entry
    // (S8): append exactly one when the module has any activity to close over.
    if !skeleton.plan.activities.is_empty() {
        skeleton.functions.push(build::dead_shell());
    }
    if skeleton.plan.child_witness.is_some() {
        skeleton.functions.push(build::child_witness_shell());
    }
    for lifted in ctx.take_dynamic_fns()? {
        skeleton.functions.push(lifted);
    }

    let mut module = MirModule {
        name: module_name,
        source,
        atoms: mem::take(&mut ctx.atoms),
        literals: mem::take(&mut ctx.literals),
        exports: skeleton.exports,
        functions: skeleton.functions,
        types: skeleton.types,
    };
    // S14: compute the backward-liveness y-spill contract over every body.
    super::liveness::annotate(&mut module);
    Ok(module)
}
