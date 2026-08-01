//! Region-body lowering, parametric over the flow being lowered (the host
//! workflow or a per-item region member flow — [`FlowCtx`], the MIR twin of
//! the reference `emitter/steps.rs::FlowCtx`). A multi-step chain lowers as
//! one `FlowFn` per step, each non-terminal step ending in a tail call to
//! its successor (IR-14) with the chain-boundary live set
//! (`chain::chain_params`) as arguments; a `max … visits` step opens with the
//! increment-and-test prologue; a chain whose last step falls through hands
//! control to the next step's region (or a member flow's exit return of the
//! collected binding). Bounded loops lower through `loops`, forks through
//! `forks`, collapsed per-item regions through `fanout`, routes through
//! `route`. One step's own statements lower through `step`, which also owns
//! the deferred-shape refusals a step body can still hit.

use std::collections::BTreeMap;

use crate::ast::Step;
use crate::emitter::{GType, Plan, RegionShape, snake};

use super::super::func::{FlowFn, FnOrigin, MirFn};
use super::super::ids::Span;
use super::super::ops::{Block, Tail};
use super::super::tydesc::TyDesc;
use super::build::{FlowFns, FnPlan};
use super::chain::chain_params;
use super::ctx::Ctx;
use super::driver::LowerError;
use super::expr::{Binding, Scope};
use super::slots::Slots;
use super::step::{Next, lower_step};

/// A nested flow's exit contract (the MIR twin of
/// `emitter/steps.rs::FlowExit`).
pub(super) struct FlowExit {
    /// The route-target name that exits the flow (a region's close step, or a
    /// subflow's declared outcome).
    pub(super) name: String,
    pub(super) kind: ExitKind,
}

/// What a nested flow's exit returns.
pub(super) enum ExitKind {
    /// A subflow outcome: `route out(<payload>)` returns `Ok(payload)`, the
    /// payload rendered toward the declared outcome type.
    Subflow { ty: GType },
    /// A per-item region member flow: routing to (or falling into) the close
    /// step returns `Ok(<collected binding>)`.
    Region {
        /// The per-instance binding the collect gathers.
        binding: String,
    },
}

/// The flow whose steps are being lowered: the host workflow (`prefix`
/// empty, no exit) or a nested flow — a subflow or a per-item region member
/// flow — whose functions are name-prefixed and whose exit returns `Ok(...)`
/// instead of a workflow outcome.
pub(super) struct FlowCtx<'a> {
    pub(super) steps: &'a [Step],
    pub(super) regions: &'a BTreeMap<String, RegionShape>,
    pub(super) plan: &'a Plan,
    /// This flow's reserved region-chain function refs.
    pub(super) fns: &'a FlowFns,
    /// Binding/type environment owned by this flow.
    pub(super) bindings: &'a BTreeMap<String, GType>,
    pub(super) exit: Option<FlowExit>,
    /// Generated-name prefix (`""` for the host, `"awl_r<id>_<open>_"` for a
    /// region member flow, `"awl_sf_<name>_"` for a subflow).
    pub(super) prefix: String,
    /// Origin qualifier for nested flows (`r0:wave`, `sf:dev_item`).
    pub(super) label: Option<String>,
    /// The full return type of this flow's functions (`Result(_, AwlError)`).
    pub(super) ret_ty: TyDesc,
}

impl FlowCtx<'_> {
    pub(super) fn origin_step(&self, step: &str) -> String {
        match &self.label {
            Some(label) => format!("{label}::{step}"),
            None => step.to_owned(),
        }
    }
}

/// The per-flow lowering environment: the module-wide fn-ref plan plus the
/// flow being lowered.
#[derive(Clone, Copy)]
pub(super) struct FlowEnv<'a> {
    pub(super) plan: &'a FnPlan,
    pub(super) flow: &'a FlowCtx<'a>,
}

/// Lower every region of one flow into `step_<name>` `FlowFn`s (one per chain
/// step, in chain order), appended to `functions`.
pub(super) fn lower_flow_fns(
    ctx: &mut Ctx<'_>,
    env: FlowEnv<'_>,
    functions: &mut Vec<MirFn>,
    slots: &mut Slots,
) -> Result<(), LowerError> {
    let mut retired_outcome_anchor = None;
    for region_index in 0..env.flow.plan.regions.len() {
        let outcome_anchor = env.flow.plan.regions[region_index]
            .layers
            .iter()
            .flatten()
            .find_map(|step_index| {
                let step = &env.flow.steps[*step_index];
                (!step.outcomes.is_empty()).then_some(step.name_span)
            });
        if let Err(error) = lower_region(ctx, env, region_index, functions, slots) {
            return Err(match retired_outcome_anchor {
                Some(anchor) => error.reanchor_unsupported(anchor),
                None => error,
            });
        }
        retired_outcome_anchor = retired_outcome_anchor.or(outcome_anchor);
    }
    Ok(())
}

fn lower_region(
    ctx: &mut Ctx<'_>,
    env: FlowEnv<'_>,
    region_index: usize,
    functions: &mut Vec<MirFn>,
    slots: &mut Slots,
) -> Result<(), LowerError> {
    let flow = env.flow;
    let region = &flow.plan.regions[region_index];
    let entry_index = region.entry;
    let layers = region.layers.clone();
    let entry_step = flow.steps[entry_index].clone();

    // A sequential chain has exactly one member per layer; dependency-parallel
    // layers stay deferred.
    let mut chain = Vec::with_capacity(layers.len());
    for layer in &layers {
        let [member] = layer.as_slice() else {
            return Err(LowerError::unsupported(
                "parallel region",
                entry_step.name_span,
            ));
        };
        chain.push(*member);
    }
    if chain.first() != Some(&entry_index) {
        return Err(LowerError::unsupported(
            "parallel region",
            entry_step.name_span,
        ));
    }
    let region_last = chain.last().copied().unwrap_or(entry_index);

    let params = chain_params(ctx, flow, &chain)?;
    for (position, &step_index) in chain.iter().enumerate() {
        let step = flow.steps[step_index].clone();
        // The entry's parameter list is the shared plan's fixed point (the
        // parity anchor); chain boundaries use the backward live sets.
        let param_names = if position == 0 {
            flow.plan.region_params(region_index).to_vec()
        } else {
            params[position].clone()
        };
        let next = chain.get(position + 1).map(|_| Next {
            callee: flow.fns.chains[region_index][position + 1],
            param_names: params[position + 1].clone(),
        });
        let chain_step = ChainStep {
            entry_step: &entry_step,
            step: &step,
            position,
            param_names: &param_names,
            next,
            region_last,
        };
        let flow_fn = lower_chain_step(ctx, env, chain_step, slots)?;
        functions.push(MirFn::Flow(flow_fn));
    }
    Ok(())
}

struct ChainStep<'a> {
    entry_step: &'a Step,
    step: &'a Step,
    position: usize,
    param_names: &'a [String],
    next: Option<Next>,
    region_last: usize,
}

fn lower_chain_step(
    ctx: &mut Ctx<'_>,
    env: FlowEnv<'_>,
    chain: ChainStep<'_>,
    slots: &mut Slots,
) -> Result<FlowFn, LowerError> {
    let flow = env.flow;
    ctx.reset_vars();
    let mut scope: Scope = Scope::new();
    let mut param_vars = Vec::new();
    let mut param_tys = Vec::new();
    for name in chain.param_names {
        let ty = flow.bindings.get(name).cloned().ok_or_else(|| {
            LowerError::new(
                chain.step.name_span,
                format!("binding `{name}` has no type"),
            )
        })?;
        let var = ctx.fresh_var();
        param_vars.push(var);
        param_tys.push(ctx.tydesc(&ty));
        scope.insert(name.clone(), Binding { var, ty });
    }

    // The visit-bound prologue of a `max … visits` step: increment the
    // language-owned counter and refuse the visit past the bound with the
    // spanned `AwlVisitsExceeded` runtime failure (the reference
    // `emitter/steps.rs::emit_visits_prologue`).
    let mut prologue = Vec::new();
    let guard = match &chain.step.max_visits {
        Some(max_visits) => Some(super::visits::visits_prologue(
            ctx,
            chain.step,
            max_visits,
            &mut scope,
            &mut prologue,
        )?),
        None => None,
    };
    // The `on failure` edge is planned BEFORE the body lowers: its call
    // arguments are the step's `avail_in`, read from the scope as it stands
    // here — after the visit prologue's counter rebind, before any statement
    // rebinds a name (the reference clones exactly this scope into its
    // compensation arm).
    let edge = match &chain.step.on_failure {
        Some(on_failure) => Some((
            on_failure,
            super::failure::plan_edge(
                ctx,
                chain.step,
                on_failure,
                chain.param_names,
                &scope,
                slots,
            )?,
        )),
        None => None,
    };
    let body = lower_step(
        ctx,
        env,
        chain.step,
        &mut scope,
        chain.next,
        chain.region_last,
        slots,
    )?;
    // The compensation function is built after the body so the step's own
    // vars stay contiguous; the guard then rewrites every fallible result in
    // the body into a test whose failure arm tail-calls it.
    let body = match edge {
        Some((on_failure, edge)) => {
            let body = super::failure::guard(chain.step, body, &edge)?;
            super::failure::lower_compensation(
                ctx,
                env,
                chain.step,
                on_failure,
                chain.param_names,
                &edge,
                slots,
            )?;
            body
        }
        None => body,
    };
    let body = match guard {
        Some((test, error_block)) => Block {
            stmts: prologue,
            tail: Tail::If {
                test,
                then_block: error_block,
                else_block: Box::new(body),
                span: Span::from_source(chain.step.name_span),
            },
        },
        None => body,
    };
    let origin = if chain.position == 0 {
        FnOrigin::Region {
            entry_step: flow.origin_step(&chain.entry_step.name),
        }
    } else {
        FnOrigin::ChainStep {
            entry_step: flow.origin_step(&chain.entry_step.name),
            step: chain.step.name.clone(),
        }
    };
    Ok(FlowFn {
        origin,
        name: format!("{}step_{}", flow.prefix, snake(&chain.step.name)),
        params: param_vars,
        param_tys,
        ret_ty: flow.ret_ty.clone(),
        body,
        span: Span::from_source(chain.step.name_span),
        degraded_parallel: false,
    })
}
