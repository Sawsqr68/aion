//! Engine-owned NIF implementations for the `aion_flow_ffi` namespace.
//!
//! These NIFs back the `@external(erlang, "aion_flow_ffi", ...)` declarations
//! in the Gleam `aion_flow` SDK. Activity dispatch is split into a normal
//! dispatch NIF plus a selective-receive await NIF.

use beamr::native::{NativeFn, ProcessContext};
use beamr::term::Term;
use beamr::term::binary_ref::BinaryRef;

use super::nif::{Determinism, Mfa, NifEntry};
use super::nif_child;
use super::nif_continue_as_new;
use super::nif_determinism::{now_impl, random_impl, random_int_impl, workflow_id_impl};
use super::nif_signal;
use super::nif_timeout;
use super::nif_timer;

#[cfg(test)]
use crate::runtime::nif_result_term::error_result_term;

const FFI_MODULE: &str = "aion_flow_ffi";
#[cfg(test)]
const NOT_YET_IMPLEMENTED: &str = "not_yet_implemented";

pub(super) fn decode_string_arg(term: Term) -> Result<String, String> {
    let bin = BinaryRef::new(term).ok_or_else(|| "argument is not a binary".to_owned())?;
    String::from_utf8(bin.as_bytes().to_vec()).map_err(|_| "argument is not valid UTF-8".to_owned())
}

fn dispatch_activity(args: &[Term], ctx: &mut ProcessContext) -> Result<Term, Term> {
    super::nif_activity_dispatch::dispatch_activity_impl(args, ctx)
}

fn dispatch_activity_in_vm(args: &[Term], ctx: &mut ProcessContext) -> Result<Term, Term> {
    super::nif_activity_in_vm::dispatch_activity_in_vm_impl(args, ctx)
}

fn await_activity_result(args: &[Term], ctx: &mut ProcessContext) -> Result<Term, Term> {
    super::nif_activity_dispatch::await_activity_result_impl(args, ctx)
}

fn collect_all(args: &[Term], ctx: &mut ProcessContext) -> Result<Term, Term> {
    super::nif_concurrency::collect_all_impl(args, ctx)
}

fn collect_race(args: &[Term], ctx: &mut ProcessContext) -> Result<Term, Term> {
    super::nif_concurrency::collect_race_impl(args, ctx)
}

fn collect_map(args: &[Term], ctx: &mut ProcessContext) -> Result<Term, Term> {
    super::nif_concurrency::collect_map_impl(args, ctx)
}

#[cfg(test)]
fn not_yet_implemented(args: &[Term], ctx: &mut ProcessContext) -> Result<Term, Term> {
    let _ = args;
    let _ = ctx.pid();

    error_result_term(ctx, NOT_YET_IMPLEMENTED)
}

fn pure(function: &str, arity: u8, native: NativeFn) -> NifEntry {
    NifEntry::new(
        Mfa::new(FFI_MODULE, function, arity),
        native,
        Determinism::Pure,
    )
}

fn side_effectful(function: &str, arity: u8, native: NativeFn) -> NifEntry {
    NifEntry::new(
        Mfa::new(FFI_MODULE, function, arity),
        native,
        Determinism::SideEffectful,
    )
}

fn dirty_side_effectful(function: &str, arity: u8, native: NativeFn) -> NifEntry {
    NifEntry::dirty(
        Mfa::new(FFI_MODULE, function, arity),
        native,
        Determinism::SideEffectful,
    )
}

/// Collect engine-owned NIF entries for `aion_flow_ffi`.
pub(super) fn engine_nif_entries() -> Vec<NifEntry> {
    vec![
        side_effectful("dispatch_activity", 3, dispatch_activity),
        // The in-VM wire is non-dirty by the same reasoning as the remote
        // wire: the NIF itself only resolves/records and spawns — the runner
        // executes in its own linked child process, and the blocking exit
        // wait lives on the Tokio blocking pool, never a scheduler thread.
        side_effectful("dispatch_activity_in_vm", 4, dispatch_activity_in_vm),
        side_effectful("await_activity_result", 1, await_activity_result),
        pure("workflow_id", 0, workflow_id_impl),
        side_effectful("now", 0, now_impl),
        side_effectful("random", 0, random_impl),
        side_effectful("random_int", 2, random_int_impl),
        // Suspending awaits run on the normal schedulers: they park via
        // request_suspend instead of blocking, so a dirty thread would only
        // be wasted on them.
        side_effectful("sleep", 1, nif_timer::sleep_impl),
        dirty_side_effectful("start_timer", 2, nif_timer::start_timer_impl),
        dirty_side_effectful("cancel_timer", 1, nif_timer::cancel_timer_impl),
        side_effectful("with_timeout", 2, nif_timeout::with_timeout_impl),
        dirty_side_effectful(
            "continue_as_new",
            1,
            nif_continue_as_new::continue_as_new_impl,
        ),
        side_effectful("receive_signal", 2, nif_signal::receive_signal),
        dirty_side_effectful("send_signal", 3, nif_signal::send_signal),
        side_effectful("register_query", 2, super::nif_query::register_query),
        // The reply NIFs are non-blocking (registry check, map removal, a
        // oneshot send) and run on the normal schedulers. Running them dirty
        // routed every query reply through beamr's dirty-result resume,
        // which deep-copies the result onto the workflow heap *without GC*:
        // on a full heap the copy fails and the VM kills the workflow with
        // `Badarg`, surfacing as `ReplyDropped` to the caller (F8).
        side_effectful("reply_query", 2, super::nif_query::reply_query),
        side_effectful("reply_query_error", 2, super::nif_query::reply_query_error),
        side_effectful(
            "report_query_diagnostic",
            2,
            super::nif_query_diagnostic::report_query_diagnostic,
        ),
        dirty_side_effectful("dispatch_query", 2, super::nif_query::dispatch_query),
        // spawn_child runs on the normal schedulers: its blocking work (one
        // recorder append plus the child start round trip) is short and
        // bounded, and its replay fast-path does no blocking work at all.
        // Running it dirty rode beamr's dirty completion bridge on every
        // workflow's spawn — and on replay of every recovered parent — where
        // a lost dirty resume parks the process forever *before* its
        // await_child can arm the child-terminal watcher, stranding the run
        // for the epoch. `wake_process` deliberately refuses dirty-in-flight
        // pids, so no engine-side wake (including the wake-confirmation
        // ladder) can heal that park; the only robust embedder-side remedy
        // is to keep this call off the dirty bridge entirely.
        side_effectful("spawn_child", 3, nif_child::spawn_child_impl),
        // await_child is a two-phase suspending native: it parks via
        // request_suspend and the child-terminal watcher wakes it, so a
        // dirty thread would only be wasted (and ten long-lived children
        // would wedge the default dirty IO pool).
        side_effectful("await_child", 1, nif_child::await_child_impl),
        // collect_* are two-phase suspending natives over parallel activity
        // dispatch: they park via request_suspend and completion markers wake
        // them, so dirty threads would only be wasted (and N parents parked
        // on slow fan-outs would wedge the default dirty IO pool).
        side_effectful("collect_all", 2, collect_all),
        side_effectful("collect_race", 2, collect_race),
        side_effectful("collect_map", 2, collect_map),
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use beamr::native::ProcessContext;
    use beamr::term::Term;
    use beamr::term::binary_ref::BinaryRef;
    use beamr::term::boxed::Tuple;

    use super::super::nif::Determinism;
    use super::{
        FFI_MODULE, NOT_YET_IMPLEMENTED, dispatch_activity, engine_nif_entries, not_yet_implemented,
    };

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    const TEST_DOUBLE_SOURCE: &str =
        include_str!("../../../../gleam/aion_flow/test/aion_flow_ffi.erl");

    fn test_double_engine_mfas(
        source: &str,
    ) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
        let export_marker = "-export([";
        let export_start = source
            .find(export_marker)
            .ok_or("test double is missing an Erlang export list")?;
        let after_marker = &source[export_start + export_marker.len()..];
        let export_end = after_marker
            .find("]).")
            .ok_or("test double export list is not terminated")?;
        let export_body = &after_marker[..export_end];
        let mut mfas = BTreeSet::new();

        for line in export_body.lines() {
            let export = line.trim().trim_end_matches(',');
            if export.is_empty() {
                continue;
            }
            let (function, arity_text) = export
                .split_once('/')
                .ok_or_else(|| format!("invalid test-double export `{export}`"))?;
            // `testing_*` entries are harness controls, not production SDK
            // bindings. Every other export must have an engine registration.
            if function.starts_with("testing_") {
                continue;
            }
            let arity = arity_text
                .parse::<u8>()
                .map_err(|error| format!("invalid arity in `{export}`: {error}"))?;
            mfas.insert(format!("{FFI_MODULE}:{function}/{arity}"));
        }

        Ok(mfas)
    }

    fn decode_result_tuple(term: Term) -> Result<(String, String), Box<dyn std::error::Error>> {
        let tuple = Tuple::new(term).ok_or("result should be a tuple")?;
        if tuple.arity() != 2 {
            return Err(format!("expected arity 2, got {}", tuple.arity()).into());
        }
        let tag = tuple.get(0).ok_or("missing tag element")?;
        let value = tuple.get(1).ok_or("missing value element")?;
        let tag_name = if tag == Term::atom(beamr::atom::Atom::OK) {
            "ok"
        } else {
            "error"
        };
        let bin = BinaryRef::new(value).ok_or("value should be a binary")?;
        let text = String::from_utf8(bin.as_bytes().to_vec())
            .map_err(|_| "value should be valid UTF-8")?;
        Ok((tag_name.to_owned(), text))
    }

    #[test]
    fn returns_error_on_wrong_arity() -> TestResult {
        let mut ctx = ProcessContext::new();

        let result = dispatch_activity(&[], &mut ctx);

        match result {
            Ok(term) => {
                let (tag, message) = decode_result_tuple(term)?;
                assert_eq!(tag, "error");
                assert!(
                    message.contains("expected 3 arguments"),
                    "unexpected: {message}"
                );
            }
            Err(_) => return Err("NIF should return Ok at the beamr level".into()),
        }
        Ok(())
    }

    #[test]
    fn registers_all_engine_nifs_as_unique_entries_with_correct_scheduling() -> TestResult {
        let entries = engine_nif_entries();
        let unique = entries
            .iter()
            .map(|entry| entry.mfa.display())
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(entries.len(), 24);
        assert_eq!(unique.len(), entries.len());
        for normal_nif in [
            "dispatch_activity",
            "dispatch_activity_in_vm",
            "await_activity_result",
            "workflow_id",
            "now",
            "random",
            "random_int",
            "sleep",
            "receive_signal",
            "with_timeout",
            "register_query",
            "reply_query",
            "reply_query_error",
            "report_query_diagnostic",
            "spawn_child",
            "await_child",
            "collect_all",
            "collect_race",
            "collect_map",
        ] {
            let found = entries
                .iter()
                .any(|entry| entry.mfa.function == normal_nif && !entry.is_dirty);
            assert!(found, "{normal_nif} should be a registered normal NIF");
        }
        assert!(
            entries
                .iter()
                .filter(|entry| !matches!(
                    entry.mfa.function.as_str(),
                    "dispatch_activity"
                        | "dispatch_activity_in_vm"
                        | "await_activity_result"
                        | "workflow_id"
                        | "now"
                        | "random"
                        | "random_int"
                        | "sleep"
                        | "receive_signal"
                        | "with_timeout"
                        | "register_query"
                        | "reply_query"
                        | "reply_query_error"
                        | "report_query_diagnostic"
                        | "spawn_child"
                        | "await_child"
                        | "collect_all"
                        | "collect_race"
                        | "collect_map"
                ))
                .all(|entry| entry.is_dirty)
        );
        for name in [
            "dispatch_activity_in_vm",
            "collect_all",
            "collect_race",
            "collect_map",
            "register_query",
            "reply_query",
            "reply_query_error",
            "dispatch_query",
            "spawn_child",
            "await_child",
            "continue_as_new",
        ] {
            let entry = entries
                .iter()
                .find(|entry| entry.mfa.function == name)
                .ok_or_else(|| format!("missing {name}"))?;
            assert!(
                !std::ptr::fn_addr_eq(
                    entry.function,
                    not_yet_implemented as beamr::native::NativeFn
                ),
                "{name} should not use the stub"
            );
        }
        Ok(())
    }

    #[test]
    fn engine_and_test_double_export_identical_mfa_sets() -> TestResult {
        let expected_determinism = [
            ("dispatch_activity", Determinism::SideEffectful),
            ("dispatch_activity_in_vm", Determinism::SideEffectful),
            ("await_activity_result", Determinism::SideEffectful),
            ("workflow_id", Determinism::Pure),
            ("now", Determinism::SideEffectful),
            ("random", Determinism::SideEffectful),
            ("random_int", Determinism::SideEffectful),
            ("sleep", Determinism::SideEffectful),
            ("start_timer", Determinism::SideEffectful),
            ("cancel_timer", Determinism::SideEffectful),
            ("with_timeout", Determinism::SideEffectful),
            ("continue_as_new", Determinism::SideEffectful),
            ("receive_signal", Determinism::SideEffectful),
            ("send_signal", Determinism::SideEffectful),
            ("register_query", Determinism::SideEffectful),
            ("reply_query", Determinism::SideEffectful),
            ("reply_query_error", Determinism::SideEffectful),
            ("report_query_diagnostic", Determinism::SideEffectful),
            ("dispatch_query", Determinism::SideEffectful),
            ("spawn_child", Determinism::SideEffectful),
            ("await_child", Determinism::SideEffectful),
            ("collect_all", Determinism::SideEffectful),
            ("collect_race", Determinism::SideEffectful),
            ("collect_map", Determinism::SideEffectful),
        ]
        .into_iter()
        .map(|(function, determinism)| (function.to_owned(), determinism))
        .collect::<std::collections::BTreeMap<_, _>>();
        let entries = engine_nif_entries();
        let engine_determinism = entries
            .iter()
            .map(|entry| (entry.mfa.function.clone(), entry.determinism))
            .collect::<std::collections::BTreeMap<_, _>>();
        let engine_mfas = entries
            .into_iter()
            .map(|entry| entry.mfa.display())
            .collect::<BTreeSet<_>>();
        let test_double_mfas = test_double_engine_mfas(TEST_DOUBLE_SOURCE)?;

        assert_eq!(engine_determinism, expected_determinism);
        assert_eq!(engine_mfas, test_double_mfas);
        Ok(())
    }

    #[test]
    fn unimplemented_stub_returns_standard_error_tuple() -> TestResult {
        let mut ctx = ProcessContext::new();

        let result = not_yet_implemented(&[], &mut ctx);

        match result {
            Ok(term) => {
                let (tag, message) = decode_result_tuple(term)?;
                assert_eq!(tag, "error");
                assert_eq!(message, NOT_YET_IMPLEMENTED);
            }
            Err(_) => return Err("stub should return Ok at the beamr level".into()),
        }
        Ok(())
    }
}
