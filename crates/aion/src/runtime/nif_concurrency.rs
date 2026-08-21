//! Two-phase suspending `collect_*` NIFs over parallel activity dispatch.
//!
//! `collect_all`/`collect_race`/`collect_map` fan out N activities and park
//! the workflow process instead of blocking a dirty thread. This module is
//! the BEAM-facing shell — argument decoding, result-term encoding, the
//! servicing guard, and wake-marker consumption; one full resolution pass
//! per invocation (pin, batch record/dispatch, per-ordinal sweep,
//! settlement) lives in [`super::nif_collect`].

use beamr::native::ProcessContext;
use beamr::term::Term;
use beamr::term::binary_ref::BinaryRef;
use beamr::term::boxed::Cons;

use crate::runtime::nif_activity::runtime_context;
use crate::runtime::nif_collect::{
    ActivitySpec, CollectDeps, CollectError, CollectStep, collect_step,
};
use crate::runtime::nif_result_term::{
    NifRefusal, error_result_term, ok_result_term, raise_reason,
};
use crate::runtime::nif_state::{CollectKind, engine_nif_state};

fn decode_string_arg(term: Term) -> Result<String, String> {
    let bin = BinaryRef::new(term).ok_or_else(|| "argument is not a binary".to_owned())?;
    String::from_utf8(bin.as_bytes().to_vec()).map_err(|_| "argument is not valid UTF-8".to_owned())
}

fn decode_spec_list(
    ctx: &mut ProcessContext,
    term: Term,
    label: &str,
) -> Result<Vec<ActivitySpec>, NifRefusal> {
    let mut specs = Vec::new();
    let mut tail = term;
    while !tail.is_nil() {
        // Every error-term allocation below may collect and move the list
        // being walked, so each failure path allocates and returns without
        // touching `cons`/`tail` again.
        let Some(cons) = Cons::new(tail) else {
            return Err(NifRefusal::reported(error_result_term(
                ctx,
                &format!("{label}: activities argument is not a proper list"),
            )));
        };
        let head = cons.head();
        let next_tail = cons.tail();
        let encoded = decode_string_arg(head).map_err(|error| {
            NifRefusal::reported(error_result_term(
                ctx,
                &format!("{label}: activity spec: {error}"),
            ))
        })?;
        let spec: ActivitySpec = serde_json::from_str(&encoded).map_err(|error| {
            NifRefusal::reported(error_result_term(
                ctx,
                &format!("{label}: invalid activity spec JSON: {error}"),
            ))
        })?;
        // Defense twin of the arity-3 dispatch wire: fan-out members dispatch
        // remotely and carry no runner thunk, so an in-VM selection is refused
        // at decode time — before any ordinal is pinned or event recorded.
        if spec.selects_in_vm() {
            return Err(NifRefusal::reported(error_result_term(
                ctx,
                &format!(
                    "{label}: activity {} selects tier in_vm — in-VM activities cannot \
                     join a collect fan-out (dispatch them individually via workflow.run)",
                    spec.spec_name()
                ),
            )));
        }
        specs.push(spec);
        tail = next_tail;
    }
    Ok(specs)
}

fn decode_concurrency_args(
    ctx: &mut ProcessContext,
    args: &[Term],
    label: &str,
) -> Result<Vec<ActivitySpec>, NifRefusal> {
    if args.len() != 2 {
        return Err(NifRefusal::reported(error_result_term(
            ctx,
            &format!("{label}: expected 2 arguments, got {}", args.len()),
        )));
    }
    decode_string_arg(args[0]).map_err(|error| {
        NifRefusal::reported(error_result_term(
            ctx,
            &format!("{label}: collection id: {error}"),
        ))
    })?;
    decode_spec_list(ctx, args[1], label)
}

fn encoded_results(ctx: &mut ProcessContext, results: &[String]) -> Result<Term, Term> {
    match serde_json::to_vec(results) {
        Ok(payload) => ok_result_term(ctx, &payload),
        Err(error) => error_result_term(
            ctx,
            &format!("collect: failed to encode result list: {error}"),
        ),
    }
}

fn run_collect(
    args: &[Term],
    ctx: &mut ProcessContext,
    kind: CollectKind,
    label: &str,
) -> Result<Term, Term> {
    let specs = match decode_concurrency_args(ctx, args, label) {
        Ok(specs) => specs,
        Err(refusal) => return refusal.into_nif_result(),
    };
    let Some(pid) = ctx.pid() else {
        return error_result_term(ctx, &format!("{label}: missing calling process pid"));
    };
    let state = match engine_nif_state(ctx) {
        Ok(state) => state,
        Err(error) => return error_result_term(ctx, &error),
    };
    // Every collect_* NIF records activity events; a query handler must stay
    // read-only. The refusal precedes the marker consumption so a refused
    // handler call never eats a wake.
    if let Err(error) = super::nif_query_pump::ensure_not_servicing_query(&state, pid, label) {
        return error_result_term(ctx, &error);
    }
    let runtime = match runtime_context(&state) {
        Ok(runtime) => runtime,
        Err(error) => return error_result_term(ctx, &error.to_string()),
    };
    // One wake marker is consumed per invocation; leaving it queued would
    // insta-rewake the suspend below into a busy spin.
    super::nif_wake::consume_wake_marker(ctx, &runtime.runtime);
    let deps = CollectDeps {
        registry: runtime.registry,
        runtime: runtime.runtime,
        tokio_handle: runtime.tokio_handle,
        dispatcher: state.activity_dispatcher(),
        advisory_catalog: state.installed_workflow_catalog(),
    };
    match collect_step(&state, &deps, pid, kind, &specs, label) {
        Ok(CollectStep::QuerySentinel(sentinel)) => error_result_term(ctx, &sentinel),
        Ok(CollectStep::AllCompleted(results)) => encoded_results(ctx, &results),
        Ok(CollectStep::RaceWon(Ok(payload))) => ok_result_term(ctx, payload.as_bytes()),
        Ok(
            CollectStep::RaceWon(Err(message))
            | CollectStep::FailFast(message)
            | CollectStep::ScopeExpired(message),
        ) => error_result_term(ctx, &message),
        Ok(CollectStep::Suspend) => {
            // Park the process; the next mailbox wake re-invokes this native
            // from the top with the ordinal base pinned. The NIL return is
            // never observed by workflow code.
            ctx.request_suspend(None);
            Ok(Term::NIL)
        }
        Err(CollectError::Message(message)) => {
            error_result_term(ctx, &format!("{label}:{message}"))
        }
        Err(CollectError::Engine(error)) => Err(raise_reason(ctx, &error.to_string())),
    }
}

/// NIF backing `aion_flow_ffi:collect_all/2`.
pub(super) fn collect_all_impl(args: &[Term], ctx: &mut ProcessContext) -> Result<Term, Term> {
    run_collect(args, ctx, CollectKind::All, "collect_all")
}

/// NIF backing `aion_flow_ffi:collect_race/2`.
pub(super) fn collect_race_impl(args: &[Term], ctx: &mut ProcessContext) -> Result<Term, Term> {
    run_collect(args, ctx, CollectKind::Race, "collect_race")
}

/// NIF backing `aion_flow_ffi:collect_map/2`.
pub(super) fn collect_map_impl(args: &[Term], ctx: &mut ProcessContext) -> Result<Term, Term> {
    run_collect(args, ctx, CollectKind::All, "collect_map")
}

#[cfg(test)]
mod allocation_failure_control {
    use super::encoded_results;
    use beamr::atom::Atom;
    use beamr::native::ProcessContext;
    use beamr::process::Process;
    use beamr::term::Term;
    use beamr::term::binary_ref::BinaryRef;
    use beamr::term::boxed::Tuple;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn decode_result(term: Term) -> Result<(Term, String), Box<dyn std::error::Error>> {
        let tuple = Tuple::new(term).ok_or("result should be a tuple")?;
        let tag = tuple.get(0).ok_or("result should have a tag")?;
        let payload = tuple.get(1).ok_or("result should have a payload")?;
        let binary = BinaryRef::new(payload).ok_or("payload should be a binary")?;
        let text = String::from_utf8(binary.as_bytes().to_vec())?;
        Ok((tag, text))
    }

    #[test]
    fn control_empty_results_encode_successfully() -> TestResult {
        let mut ctx = ProcessContext::new();

        let result = encoded_results(&mut ctx, &[]);
        let term = result.map_err(|reason| format!("result allocation failed: {reason:?}"))?;
        let (tag, payload) = decode_result(term)?;

        println!("CONTROL healthy empty: tag=ok payload={payload}");
        assert_eq!(tag, Term::atom(Atom::OK));
        assert_eq!(payload, "[]");
        Ok(())
    }

    #[test]
    fn allocation_failure_refuses_with_beamrs_exact_reason() {
        let mut process = Process::new(1, 0);
        process.heap_mut().set_max_capacity(0);
        let mut ctx = ProcessContext::new();
        ctx.attach_process(&mut process, 0);

        let failed_result = encoded_results(&mut ctx, &[]);

        assert_eq!(failed_result, Err(Term::atom(Atom::BADARG)));
    }
}
