//! Refusal-ratchet tests kept separate from the broad MIR golden module.

use std::fs;
use std::path::{Path, PathBuf};

use super::{LowerError, MirModule, lower};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn lower_fixture(path: &Path) -> Result<Result<MirModule, LowerError>, Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    let document = crate::parse(&source)?;
    Ok(lower(&document, path.parent()))
}

fn lower_source(source: &str) -> Result<Result<MirModule, LowerError>, Box<dyn std::error::Error>> {
    let document = crate::parse(source)?;
    Ok(lower(&document, Some(Path::new("."))))
}

/// The retired `on failure` refusal: `nested_handlers` was the exact-span pin
/// for the whole class while the direct path refused every `on failure`
/// block. The edge now lowers (`lower/failure.rs`), so the pin flips to a
/// lowering + verify coverage pin (the same promotion as
/// `guard_optional_wait_now_lowers`) — and the SUBSTEP half of the deferral
/// keeps its own exact-span pin below, so retiring the top-level refusal can
/// never retire the substep one by accident.
#[test]
fn nested_handlers_on_failure_now_lowers() -> Result<(), Box<dyn std::error::Error>> {
    let path = manifest_dir().join("tests/fixtures/rev2/loop-outcomes/valid/nested_handlers.awl");
    let module = lower_fixture(&path)??;
    super::verify(&module)?;
    Ok(())
}

/// The substep deferral is UNTOUCHED by the value-payload landing: both
/// substep fixtures still refuse, each pinned to its OWN exact class and
/// COMPLETE span (start/end/line/column), so range or column drift fails too.
///
/// The two now refuse at the SAME gate, and that convergence is the
/// value-payload landing's evidence rather than a weakening of this pin.
/// Before it, `substep_on_failure_route_tail` carried a value route payload
/// (`route failed("substep finished")`) and never reached step lowering at
/// all — the driver's staging gate took it first, at the payload's own span
/// (513..531, line 20). That gate is retired: the payload lowers now, so the
/// fixture reaches `step::lower_step` and refuses at `substeps`, anchored to
/// its guarding top-level step (`publish_it`) — exactly as its sibling
/// `substeps_two_stage` always has. The refusal MOVED to the fixture's real
/// remaining gap; it was not removed, and the fixture is still out of the
/// covered ratchet. A future substep lowering must retire BOTH knowingly.
#[test]
fn substep_fixtures_still_refuse() -> Result<(), Box<dyn std::error::Error>> {
    for (fixture, expected_shape, expected) in [
        (
            "tests/fixtures/rev2/flow-shape/valid/substep_on_failure_route_tail.awl",
            "substeps",
            crate::Span {
                start: 323,
                end: 333,
                line: 11,
                column: 6,
            },
        ),
        (
            "tests/fixtures/rev2/loop-outcomes/valid/substeps_two_stage.awl",
            "substeps",
            crate::Span {
                start: 503,
                end: 510,
                line: 18,
                column: 6,
            },
        ),
    ] {
        match lower_fixture(&manifest_dir().join(fixture))? {
            Err(LowerError::Unsupported { shape, span }) => {
                assert_eq!(shape, expected_shape, "{fixture} refusal class drifted");
                assert_eq!(span, expected, "{fixture} refusal span drifted");
            }
            other => {
                return Err(format!("{fixture} must still refuse: {other:?}").into());
            }
        }
    }
    Ok(())
}

/// BC-4 adversarial fixture `zero_step_workflow` is below BOTH backends' floor:
/// it checks clean but has no steps, so the direct path refuses. The refusal is
/// the span-carrying `Message` variant (`LowerError::new` in `build.rs`), so the
/// pin asserts the EXACT variant, the exact message, and the COMPLETE span
/// (start/end/line/column) — never a substring or a lone line. It lives outside
/// any `valid/` sweep directory because neither backend can produce runnable
/// code for it (the reference emitter also refuses, "no steps to execute").
#[test]
fn bc4_zero_step_workflow_refuses_no_start_region() -> Result<(), Box<dyn std::error::Error>> {
    let path = manifest_dir().join("tests/fixtures/rev2/header-types/zero_step_workflow.awl");
    match lower_fixture(&path)? {
        Err(LowerError::Message { message, span }) => {
            assert_eq!(
                message, "the workflow has no start region",
                "zero_step_workflow refusal message drifted"
            );
            assert_eq!(
                span,
                crate::Span {
                    start: 0,
                    end: 189,
                    line: 1,
                    column: 1
                },
                "zero_step_workflow refusal span drifted"
            );
        }
        other => {
            return Err(format!(
                "zero_step_workflow must refuse (Message, no start region): {other:?}"
            )
            .into());
        }
    }
    Ok(())
}

/// The dependency-parallel layer (`step b after a`) is still unlowered, and
/// these fixtures pin it. `dev_brief` always refused here. `layer_hygiene`
/// JOINED this pin with the value-payload landing rather than joining the
/// covered ratchet: its `route done(joined)` lowers now, but the document
/// also carries `step left after root` / `step right after root`, so the
/// payload gate simply stopped being the first thing it hit. Pinning it here
/// is what keeps that boundary honest — the fixture is NOT covered, and the
/// reason it is not covered is a different gap than the one this landing
/// closed.
#[test]
fn parallel_layer_fixtures_still_refuse() -> Result<(), Box<dyn std::error::Error>> {
    for fixture in [
        "tests/fixtures/rev2/flagship/valid/dev_brief.awl",
        "tests/fixtures/rev2/flow-shape/valid/layer_hygiene.awl",
    ] {
        match lower_fixture(&manifest_dir().join(fixture))? {
            Err(LowerError::Unsupported { shape, .. }) => {
                assert_eq!(shape, "parallel region", "{fixture} refusal class drifted");
            }
            other => return Err(format!("{fixture} must still refuse: {other:?}").into()),
        }
    }
    Ok(())
}

/// The value-payload coverage pin (the same promotion as
/// `nested_handlers_on_failure_now_lowers`): `route <outcome>(<value>)`
/// lowers, and the payload is the EXPRESSION rather than a bare binding
/// pickup. `temporary_hygiene` is the arm that proves the difference — its
/// payload is `status + workflow.id`, a concat whose lowering emits prelude
/// statements and a runtime call, so a lowering that only accepted bare
/// bindings would fail here rather than quietly routing the wrong term.
#[test]
fn value_route_payloads_now_lower() -> Result<(), Box<dyn std::error::Error>> {
    for fixture in [
        "tests/fixtures/rev2/flow-shape/valid/value_route_payload.awl",
        "tests/fixtures/rev2/flow-shape/valid/temporary_hygiene.awl",
    ] {
        let module = lower_fixture(&manifest_dir().join(fixture))??;
        super::verify(&module)?;
    }
    Ok(())
}

/// A value payload is COERCED toward the outcome's type, not passed through
/// raw: an `Int` routed to an `Int?` outcome must arrive wrapped in `Some`,
/// exactly as the reference `render_arg_for` wraps it. Without the coercion
/// the direct path would hand the engine a bare integer where the Gleam
/// oracle hands it `Some(1)` — a silent wire divergence, so it is pinned on
/// the term rather than left to the differential alone.
#[test]
fn a_value_payload_is_wrapped_for_an_optional_outcome() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
//! Focused optional-coercion pin for value route payloads.
workflow optional_value_payload
  input docs: [Doc]
  outcome done: type Int?, route success

type Doc { title: String }

step count_them
  docs |> count -> total
  route done(total)
";
    let module = lower_source(source)??;
    super::verify(&module)?;
    let printed = super::print_mir(&module);
    // Pinned on the exact ROUTE TERM, not on the `some` atom merely appearing
    // in the pool: an `Int?` outcome's codec interns `some`/`none` regardless,
    // so a pool-presence check would pass even with the coercion stripped.
    // The count must be wrapped and the WRAPPED value must be what the outcome
    // constructor receives.
    for term in ["v2 = record(some, [v1])", "v3 = record(done_outcome, [v2])"] {
        assert!(
            printed.contains(term),
            "an Int payload routed to an `Int?` outcome must reach the outcome \
             constructor wrapped in `Some` (missing `{term}`):\n{printed}"
        );
    }
    Ok(())
}

/// `guard_optional_wait` was the wait-deferral pin; timeout waits now lower,
/// so it flips to a lowering + verify coverage pin (the same promotion as
/// `post_join_combinator_now_lowers`).
#[test]
fn guard_optional_wait_now_lowers() -> Result<(), Box<dyn std::error::Error>> {
    let path =
        manifest_dir().join("tests/fixtures/rev2/loop-outcomes/valid/guard_optional_wait.awl");
    let module = lower_fixture(&path)??;
    super::verify(&module)?;
    Ok(())
}

/// The historical post-join combinator refusal, kept as a coverage pin: a
/// supported collection join followed by exactly one combinator now lowers.
#[test]
fn post_join_combinator_now_lowers() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
//! Focused post-join combinator coverage pin.
workflow post_join_combinator
  input docs: [Doc]
  outcome done: type Done, route success

type Doc     { title: String }
type Verdict { accepted: Bool }
type Done    { count: Int }

worker review
  action check_doc(doc: Doc) -> Verdict

step check_all
  fork doc in docs
    check_doc(doc: doc)
  join -> verdicts

  verdicts |> count -> total
  route done(count: total)
";
    let module = lower_source(source)??;
    super::verify(&module)?;
    Ok(())
}

/// A document whose ONLY child-spawn forms are STATEMENT-LEVEL (an awaited
/// child call, or a fire-and-forget `spawn`) must still plan the T-WIT
/// witness — `needs_child_witness` covers `Statement::Call`/`Statement::Spawn`,
/// not just collection forks and pipe stages. The salvage defect was a
/// `child spawn has no planned witness` Planning error at lower time.
#[test]
fn statement_level_child_forms_plan_the_witness() -> Result<(), Box<dyn std::error::Error>> {
    for fixture in [
        "tests/fixtures/rev2/declarations/valid/child_call_awaited.awl",
        "tests/fixtures/rev2/declarations/valid/spawn_detached.awl",
    ] {
        let module = lower_fixture(&manifest_dir().join(fixture))??;
        super::verify(&module)?;
        let printed = format!("{module:?}");
        assert!(
            printed.contains("ChildWitness"),
            "{fixture} must carry the planned T-WIT function"
        );
    }
    Ok(())
}

/// `sort` over a non-comparable key is the reference emitter's hard gate; the
/// checker does not constrain key comparability, so a check-clean document
/// must refuse here with the reference class. Bool keys refuse too: the
/// shipped `gleam_stdlib` exports no `gleam@bool:compare/2`, so a Bool key
/// could only ever be a Gleam compile error (reference path) or an `undef`
/// crash (direct path).
#[test]
fn sort_non_comparable_key_refuses_with_the_reference_class()
-> Result<(), Box<dyn std::error::Error>> {
    for (key_type, field) in [("Inner", "inner"), ("Bool", "flag")] {
        let source = format!(
            "\
//! Focused non-comparable sort-key refusal.
workflow sort_non_comparable
  input docs: [Doc]
  outcome done: type Done, route success

type Inner {{ note: String }}
type Doc   {{ inner: Inner, flag: Bool }}
type Done  {{ total: Int }}

step order
  docs |> sort(.{field}) |> count -> total
  route done(total: total)
"
        );
        match lower_source(&source)? {
            Err(LowerError::Unsupported { shape, .. }) => {
                assert_eq!(
                    shape, "`sort` over a non-comparable key (needs Int, Float, String)",
                    "sort key type {key_type} must refuse"
                );
            }
            other => {
                return Err(format!(
                    "expected the non-comparable sort refusal for {key_type}, got {other:?}"
                )
                .into());
            }
        }
    }
    Ok(())
}

/// An ordinary action call whose argument indexes (`docs[0]`) now lowers
/// through `aion@awl@runtime:index/3`. The span-anchoring intent of the old
/// refusal pin survives as the byte-identical out-of-range message literal
/// (line 13, column 18 — the exact `docs[0]` slice), which must appear among
/// the module literals alongside the `RtIndex` runtime call.
/// The parallel-fork indexing-prelude class remains separately pinned in
/// `fork_tests::stopgap_refusals_match_the_reference_classes`.
#[test]
fn ordinary_call_index_lowers_with_the_anchored_runtime_message()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "\
//! Focused ordinary-call index coverage pin.
workflow ordinary_index
  input docs: [Doc]
  outcome done: type Done, route success

type Doc  { title: String }
type Done { title: String }

worker review
  action check_doc(doc: Doc) -> Done

step check_one
  check_doc(doc: docs[0]) -> checked
  route done(title: checked.title)
";
    let module = lower_source(source)??;
    super::verify(&module)?;
    let printed = format!("{module:?}");
    assert!(
        printed.contains("RtIndex"),
        "index lowering must ride aion@awl@runtime:index/3"
    );
    let message: Vec<u8> = b"index 0 out of range at line 13, column 18".to_vec();
    let anchored = module
        .literals
        .iter()
        .any(|literal| matches!(literal, super::MirLiteral::Binary(bytes) if *bytes == message));
    assert!(
        anchored,
        "the out-of-range message must stay anchored to line 13, column 18"
    );
    Ok(())
}
