//! Two-phase activity dispatch NIFs.

use std::sync::Arc;

use crate::activity::bridge::{ActivityDispatch, ActivityDispatcher};
use crate::durability::{Command, CorrelationKey, ResolveOutcome};
use crate::runtime::nif_activity::{
    context_error_term, correlation_id, decode_string_arg, json_payload, labels_from_config,
    record_started, runtime_context,
};
use crate::runtime::nif_context::NifContext;
use crate::runtime::nif_result_term::{NifRefusal, error_result_term, ok_result_term};
use aion_core::ActivityId;
use beamr::native::ProcessContext;
use beamr::term::Term;

/// NIF backing `aion_flow_ffi:dispatch_activity/3`.
pub(super) fn dispatch_activity_impl(
    args: &[Term],
    ctx: &mut ProcessContext,
) -> Result<Term, Term> {
    let Ok((name, input, config)) = decode_dispatch_args(args) else {
        return error_result_term(
            ctx,
            &format!(
                "dispatch_activity: expected 3 arguments, got {}",
                args.len()
            ),
        );
    };
    // Defense: an in-VM selection must cross the arity-4 wire that carries the
    // runner thunk. Refused BEFORE any ordinal allocation or resolve, so
    // nothing is recorded.
    if super::nif_activity::config_tier(&config).as_deref() == Some(super::nif_activity::IN_VM_TIER)
    {
        return error_result_term(
            ctx,
            "dispatch_activity: tier in_vm cannot cross the remote dispatch wire — \
             in-VM dispatch requires dispatch_activity_in_vm/4 carrying the runner thunk",
        );
    }
    let Some(pid) = ctx.pid() else {
        return error_result_term(ctx, "dispatch_activity: missing calling process pid");
    };
    let state = match super::nif_state::engine_nif_state(ctx) {
        Ok(state) => state,
        Err(error) => return error_result_term(ctx, &error),
    };
    // dispatch_activity records `ActivityScheduled`; a query handler must
    // stay read-only.
    if let Err(error) =
        super::nif_query_pump::ensure_not_servicing_query(&state, pid, "dispatch_activity")
    {
        return error_result_term(ctx, &error);
    }
    let runtime = match runtime_context(&state) {
        Ok(runtime) => runtime,
        Err(error) => return context_error_term(ctx, &error).into_nif_result(),
    };
    let context = match NifContext::new(
        pid,
        runtime.registry.as_ref(),
        runtime.tokio_handle.clone(),
        runtime.runtime.signal_delivery(),
    ) {
        Ok(context) => context,
        Err(error) => return context_error_term(ctx, &error).into_nif_result(),
    };
    let dispatcher = state.activity_dispatcher();
    let advisory_catalog = state.installed_workflow_catalog();
    match dispatch_activity_with_context(
        ctx,
        context,
        dispatcher,
        runtime.runtime,
        &runtime.tokio_handle,
        advisory_catalog.as_deref(),
        ActivityCall {
            name,
            input,
            config,
            attempt: FIRST_DELIVERY_ATTEMPT,
        },
    ) {
        Ok(term) => Ok(term),
        Err(refusal) => refusal.into_nif_result(),
    }
}

/// NIF backing `aion_flow_ffi:await_activity_result/1`.
pub(super) fn await_activity_result_impl(
    args: &[Term],
    ctx: &mut ProcessContext,
) -> Result<Term, Term> {
    if args.len() != 1 {
        return error_result_term(
            ctx,
            &format!(
                "await_activity_result: expected 1 argument, got {}",
                args.len()
            ),
        );
    }
    let correlation = match decode_string_arg(args[0]) {
        Ok(value) => value,
        Err(error) => {
            return error_result_term(ctx, &format!("await_activity_result id: {error}"));
        }
    };
    let Some(pid) = ctx.pid() else {
        return error_result_term(ctx, "await_activity_result: missing calling process pid");
    };
    let state = match super::nif_state::engine_nif_state(ctx) {
        Ok(state) => state,
        Err(error) => return error_result_term(ctx, &error),
    };
    let runtime = match runtime_context(&state) {
        Ok(runtime) => runtime,
        Err(error) => return context_error_term(ctx, &error).into_nif_result(),
    };
    let context = match NifContext::new(
        pid,
        runtime.registry.as_ref(),
        runtime.tokio_handle,
        runtime.runtime.signal_delivery(),
    ) {
        Ok(context) => context,
        Err(error) => return context_error_term(ctx, &error).into_nif_result(),
    };
    await_activity_result_with_context(&state, context, &runtime.runtime, ctx, &correlation)
}

fn decode_dispatch_args(args: &[Term]) -> Result<(String, String, String), ()> {
    if args.len() != 3 {
        return Err(());
    }
    let name = decode_string_arg(args[0]).map_err(|_| ())?;
    let input = decode_string_arg(args[1]).map_err(|_| ())?;
    let config = decode_string_arg(args[2]).map_err(|_| ())?;
    Ok((name, input, config))
}

/// First delivery: a dispatch for an ordinal with no recorded attempt trail
/// is attempt 1.
///
/// The remote-tier retry loop ([`spawn_completion_task`], #197) re-dispatches
/// with the incremented attempt when the SDK-declared retry policy (decoded
/// from the dispatch `config` JSON by [`super::nif_activity_retry`]) has
/// budget left, and a live re-dispatch after a crash continues the recorded
/// trail via [`super::nif_activity_retry::next_delivery_attempt`]. This is
/// the single documented producer-side constant; no consumer guesses an
/// attempt.
///
/// In-VM retry-seam constraint: unlike remote activities, an in-VM retry must
/// be driven from a seam that HOLDS THE RUNNER — the dispatch NIF itself
/// (replay's reopen path re-supplies the thunk on every re-execution of
/// workflow code) or an SDK-level loop. The remote retry loop deliberately
/// does not cover the in-VM tier, whose dispatches stay single-attempt.
pub(super) const FIRST_DELIVERY_ATTEMPT: u32 = 1;

/// Grouped parameters for the activity being dispatched.
///
/// Shared with the `collect_*` fan-out natives, which dispatch N of these
/// through the same completion-task machinery.
pub(super) struct ActivityCall {
    pub(super) name: String,
    pub(super) input: String,
    pub(super) config: String,
    /// One-based delivery attempt stamped onto the dispatch (and from there
    /// onto the worker wire). See [`FIRST_DELIVERY_ATTEMPT`].
    pub(super) attempt: u32,
}

/// #266 Defect B: an ORDINARY activity's attempt that the dead server left in
/// flight gets its honest NON-terminal failure record BEFORE the re-dispatch
/// starts the next one — the record that stops an orphaned run reading as
/// silently still working.
///
/// Budget-inert by the ruled honesty-only semantics: infrastructure death is
/// attempt-neutral, so nothing about the dispatch decision changes; only the
/// durable trail gains the truth about the superseded attempt.
///
/// The caller decides WHETHER this is the truthful record for the dangling
/// attempt it found — for a declared agent action it is not, and the adoption
/// offer is recorded instead (#36) — so this function takes the attempt rather
/// than re-deriving it, and the two records can never be written for one
/// attempt.
fn record_superseded_attempt(
    ctx: &mut ProcessContext,
    context: &NifContext,
    activity_id: &ActivityId,
    superseded: u32,
) -> Result<(), NifRefusal> {
    context
        .record_activity_failed(
            chrono::Utc::now(),
            activity_id.clone(),
            aion_core::ActivityError {
                kind: aion_core::ActivityErrorKind::Retryable,
                message: super::nif_activity_retry::SUPERSEDED_BY_SERVER_DEATH_REASON.to_owned(),
                details: None,
            },
            superseded,
        )
        .map_err(|error| context_error_term(ctx, &error))
}

/// Everything the opening record of ONE delivery needs, so the decision below
/// can be read on its own rather than inside a two-hundred-line dispatch.
struct Opening<'a> {
    /// The ordinal being delivered.
    activity_id: &'a ActivityId,
    /// The declared action's name — the key the class is resolved by.
    activity_type: &'a str,
    /// The payload the delivery carries.
    input: aion_core::Payload,
    /// The queue the dispatch resolved to.
    task_queue: &'a str,
    /// The node affinity the dispatch resolved to, if any.
    node: Option<&'a str>,
    /// The attempt a FRESH ordinal delivers at (`FIRST_DELIVERY_ATTEMPT`).
    first_delivery_attempt: u32,
}

/// Record what recovery owes this ordinal, and answer the attempt the live
/// dispatch must carry.
///
/// What a DANGLING attempt means depends on whether its holder can outlive the
/// engine, and only the declared class can say. See
/// [`super::nif_activity_agent`] for the argument in full:
///
/// * agent (#36): the holder is its own OS process and may still be working.
///   RETAIN the attempt identity — the worker's spawn gate and single-flight
///   recognise the execution they already hold — and record an explicit
///   adoption OFFER. The engine must not pre-emptively lie that it died.
/// * ordinary (#266 Defect B): nothing survives to settle it. Record the
///   honest supersession and continue the trail at the NEXT attempt, so the run
///   stops reading as silently still working.
///
/// #197: either way a live re-dispatch continues the ordinal's recorded trail
/// rather than restarting it, and a fresh ordinal resolves to
/// `first_delivery_attempt` exactly as before.
///
/// The two records are mutually exclusive by construction — one `match`, one
/// arm each — so no attempt can ever carry both.
fn open_this_delivery(
    ctx: &mut ProcessContext,
    context: &NifContext,
    catalog: Option<&crate::loader::WorkflowCatalog>,
    opening: Opening<'_>,
) -> Result<u32, NifRefusal> {
    let Opening {
        activity_id,
        activity_type,
        input,
        task_queue,
        node,
        first_delivery_attempt,
    } = opening;
    let dangling = super::nif_activity_retry::dangling_attempt(context.history(), activity_id);
    let adopts = dangling.is_some()
        && super::nif_activity_agent::declared_agent(
            catalog,
            &context.workflow_handle(),
            activity_type,
        );
    match dangling {
        Some(dangling) if adopts => {
            context
                .record_activity_adoption_offered(chrono::Utc::now(), activity_id.clone(), dangling)
                .map_err(|error| context_error_term(ctx, &error))?;
            Ok(dangling)
        }
        dangling => {
            let attempt =
                super::nif_activity_retry::next_delivery_attempt(context.history(), activity_id)
                    .max(first_delivery_attempt);
            if let Some(superseded) = dangling {
                record_superseded_attempt(ctx, context, activity_id, superseded)?;
            }
            // A crash after recording PolicyRefused but before recording its
            // hop leaves the routing decision pending. The async retry seam
            // must record the hop (or final exhaustion) before any next start;
            // eagerly recording one here would invert the durable R1-e order.
            if super::nif_activity_fallback::trailing_policy_refusal(context.history(), activity_id)
                .is_none()
            {
                record_started(
                    ctx,
                    context,
                    activity_id.clone(),
                    super::nif_activity::ScheduledActivity {
                        activity_type: activity_type.to_owned(),
                        input,
                        task_queue: task_queue.to_owned(),
                        node: node.map(str::to_owned),
                        // NOI-0: stamp the SAME one-based attempt onto the recorded
                        // `ActivityStarted` that is stamped onto the live wire.
                        attempt,
                    },
                )?;
            }
            Ok(attempt)
        }
    }
}

fn dispatch_activity_with_context(
    ctx: &mut ProcessContext,
    mut context: NifContext,
    dispatcher: Option<Arc<dyn ActivityDispatcher>>,
    runtime: Arc<crate::RuntimeHandle>,
    tokio_handle: &tokio::runtime::Handle,
    catalog: Option<&crate::loader::WorkflowCatalog>,
    call: ActivityCall,
) -> Result<Term, NifRefusal> {
    let input_payload = json_payload(ctx, &call.input, "dispatch_activity", "input")?;
    let ordinal = context.next_activity_ordinal();
    let key = CorrelationKey::Activity(ordinal);
    let activity_id = ActivityId::from_sequence_position(ordinal);
    let correlation = correlation_id(ordinal);
    let namespace = context.workflow_handle().namespace().to_owned();
    // UNOBSERVED seam. The dispatch hands workflow code a correlation id, not
    // the activity's outcome — the outcome is delivered by
    // `await_activity_result`, which is the OBSERVED seam. Recorded resolution
    // here reaches the activity's TERMINAL, which the live path has not reached
    // at this position (it has only just recorded `ActivityScheduled`), so
    // advancing workflow-visible now here would make a replayed run serve a
    // timestamp its live original could not have served (aion#1).
    match context
        .resolve_command_unobserved(Command::RunActivity {
            key,
            activity_type: call.name.clone(),
            input: input_payload.clone(),
        })
        .map_err(|error| context_error_term(ctx, &error))?
    {
        ResolveOutcome::Recorded(_) => {
            ok_result_term(ctx, correlation.as_bytes()).map_err(NifRefusal::Unbuildable)
        }
        ResolveOutcome::ResumeLive => {
            let Some(dispatcher) = dispatcher else {
                return error_result_term(
                    ctx,
                    "no activity dispatcher configured — set one via EngineBuilder::activity_dispatcher",
                )
                .map_err(NifRefusal::Unbuildable);
            };
            // NSTQ-4 (+#144): resolve the dispatch's task queue once at this
            // schedule seam (activity override > workflow declared default >
            // the workflow's RECORDED start-time queue > the named default),
            // then stamp the same value onto BOTH the recorded
            // `ActivityScheduled` and the live dispatch so history and routing
            // never diverge. The start-time queue is read from recorded history,
            // so replay re-resolves identically.
            let start_time_task_queue = context.start_time_task_queue();
            let initial_task_queue = super::nif_activity::resolve_task_queue(
                &call.config,
                start_time_task_queue.as_deref(),
            );
            // A recorded hop is a post-failure override of the initial choice,
            // not another precedence level inside `resolve_task_queue`. Reading
            // the durable answer makes crash recovery reuse the selected queue.
            let task_queue =
                super::nif_activity_fallback::recorded_hop_queue(context.history(), &activity_id)
                    .unwrap_or(initial_task_queue);
            // NODE-4: resolve the OPTIONAL node affinity once at the same seam
            // (activity pin, else None — no workflow default), and stamp the same
            // value onto BOTH the recorded `ActivityScheduled` and the live
            // dispatch so history and routing never diverge.
            let node = super::nif_activity::resolve_node(&call.config);
            let attempt = open_this_delivery(
                ctx,
                &context,
                catalog,
                Opening {
                    activity_id: &activity_id,
                    activity_type: &call.name,
                    input: input_payload,
                    task_queue: &task_queue,
                    node: node.as_deref(),
                    first_delivery_attempt: call.attempt,
                },
            )?;
            let labels = labels_from_config(&call.config);
            // R5: the declared class comes off the contract this run is pinned
            // to, never off the dispatch config the SDK builds.
            let advisory = super::nif_activity_advisory::declared_advisory(
                catalog,
                &context.workflow_handle(),
                &call.name,
            );
            let request = ActivityDispatch {
                namespace,
                task_queue,
                node,
                workflow_id: context.workflow_id().clone(),
                run_id: context.workflow_handle().run_id().clone(),
                activity_id,
                name: call.name,
                input: call.input,
                config: call.config,
                attempt,
                labels,
                advisory,
            };
            // Taken BEFORE the call: argument evaluation is left-to-right, so
            // `runtime` is moved into the second parameter before the seam
            // literal is built.
            let engine_tasks = runtime.engine_tasks();
            spawn_completion_task(
                tokio_handle,
                runtime,
                dispatcher,
                RetryRecorderSeam {
                    recorder: context.recorder(),
                    run_id: context.workflow_handle().run_id().clone(),
                    engine_tasks,
                },
                context.pid(),
                correlation.clone(),
                request,
            );
            ok_result_term(ctx, correlation.as_bytes()).map_err(NifRefusal::Unbuildable)
        }
    }
}

#[cfg(test)]
use super::nif_activity_retry_dispatch::{RetryLoopTerminal, dispatch_with_retries};
pub(super) use super::nif_activity_retry_dispatch::{RetryRecorderSeam, spawn_completion_task};

use super::nif_activity_await::await_activity_result_with_context;
#[cfg(test)]
pub(super) use super::nif_activity_await::{ActivityAwaitStep, await_activity_step};

#[cfg(test)]
#[path = "nif_activity_dispatch_tests/mod.rs"]
mod tests;
