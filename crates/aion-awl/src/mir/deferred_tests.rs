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

/// The retired `substeps` refusal — the LAST refusal class the direct MIR
/// lowering path held — flipped to a lowering + verify coverage pin (the same
/// promotion as `parallel_layer_fixtures_now_lower`). Both fixtures lower;
/// neither merely moved to a later refusal.
///
/// The two arms are deliberately different machinery.
/// `substeps_two_stage` is the SIBLING CHAIN: `fetch_batch` routes to `scrub`
/// by name and `scrub` falls through into the parent's own outcome clauses.
/// `substep_on_failure_route_tail` is the `on failure` × substep INTERACTION,
/// and it also carries a value route payload fired through a parent arm — the
/// two constructs the preceding landings built.
#[test]
fn substep_fixtures_now_lower() -> Result<(), Box<dyn std::error::Error>> {
    for fixture in [
        "tests/fixtures/rev2/flow-shape/valid/substep_on_failure_route_tail.awl",
        "tests/fixtures/rev2/loop-outcomes/valid/substeps_two_stage.awl",
    ] {
        let module = lower_fixture(&manifest_dir().join(fixture))??;
        super::verify(&module)?;
    }
    Ok(())
}

/// The substep chain's SHAPE, pinned two-directionally on the fixture whose
/// parent carries two substeps.
///
/// A substep group is a chain of functions, not an inlined block: the parent
/// hands off to the first substep and does nothing else, a sibling route is a
/// tail call into that sibling's own function, and the parent's outcome clauses
/// are evaluated at the CHAIN'S END — inside the LAST substep — because that is
/// where the group's boundary is (`emitter/subs.rs::emit_sub_tail`).
///
/// Each arm needs its negative half. Asserting only that the parent tail-calls
/// the first substep would still pass on a lowering that ALSO evaluated the
/// parent's arms at the parent (running them before the substeps ever did);
/// asserting only that the last substep evaluates them would still pass if the
/// first substep evaluated them too, firing `hollow_source` one stage early.
/// So the parent must hold the hand-off AND NOT the arms, and the arms must
/// land in the last substep AND NOT the first — except for the ONE arm the
/// first substep legitimately fires by routing to it by name.
#[test]
fn a_substep_chain_hands_off_and_closes_on_the_parents_arms()
-> Result<(), Box<dyn std::error::Error>> {
    let path =
        manifest_dir().join("tests/fixtures/rev2/loop-outcomes/valid/substeps_two_stage.awl");
    let module = lower_fixture(&path)??;
    let printed = super::print_mir(&module);
    let body_of = |name: &str| -> Result<String, String> {
        printed
            .split(&format!("== fn {name}/1"))
            .nth(1)
            .and_then(|rest| rest.split("\n== fn ").next())
            .map(str::to_owned)
            .ok_or_else(|| format!("function `{name}` is missing from the module"))
    };

    let parent = body_of("step_prepare")?;
    assert!(
        parent.contains("tail_local sub_prepare_fetch_batch(v0)"),
        "the parent must hand off to its first substep:\n{parent}"
    );
    assert!(
        !parent.contains("prepared_outcome") && !parent.contains("awl_outcome_failure"),
        "the parent must evaluate NONE of its own outcome arms — they close the \
         substep chain, they do not run before it:\n{parent}"
    );

    let first = body_of("sub_prepare_fetch_batch")?;
    assert!(
        first.contains("tail_local sub_prepare_scrub("),
        "a route naming a sibling substep must tail-call that sibling's function:\n{first}"
    );
    assert!(
        !first.contains("prepared_outcome"),
        "the FIRST substep must not reach the parent's success arm — only the arm it \
         routes to by name:\n{first}"
    );

    let last = body_of("sub_prepare_scrub")?;
    assert!(
        last.contains("record(prepared_outcome,") && last.contains("record(awl_outcome_failure,"),
        "the LAST substep evaluates BOTH of the parent's arms — the chain's \
         boundary:\n{last}"
    );
    assert!(
        !last.contains("tail_local sub_prepare_"),
        "the last substep has no sibling to hand off to:\n{last}"
    );
    Ok(())
}

/// The `on failure` × substep interaction, pinned two-directionally.
///
/// A substep's guarded body puts every fallible result behind an
/// `is_tagged ok/2` test whose FAILURE arm tail-calls the lifted compensation.
/// The substep's TAIL — here a `route finish` that fires the parent's arm and
/// returns `Error(AwlOutcomeFailure(...))` — is a `Tail`, never a `TryBind`, so
/// it lands in the SUCCESS arm and can never trigger compensation. That is the
/// reference's invariant (`emitter/failure.rs`: a body-terminal route renders
/// outside the attempt closure), and the two halves must be asserted together:
/// a body that put the routed failure in the ERROR arm instead would still
/// contain both terms, and would run `cleanup` on a deliberate failure outcome.
///
/// The compensation is checked for the same pairing from the other side: it
/// runs `cleanup` and then fires the same parent arm, which is what the
/// reference's `Error(_) ->` block does with the substep frame still in hand.
#[test]
fn a_substeps_failure_edge_keeps_its_routed_outcome_out_of_the_guard()
-> Result<(), Box<dyn std::error::Error>> {
    let path = manifest_dir()
        .join("tests/fixtures/rev2/flow-shape/valid/substep_on_failure_route_tail.awl");
    let module = lower_fixture(&path)??;
    let printed = super::print_mir(&module);
    let attempt = printed
        .split("== fn sub_publish_it_attempt/1")
        .nth(1)
        .and_then(|rest| rest.split("\n== fn ").next())
        .ok_or("the guarded substep function is missing")?;
    let (success_arm, failure_arm) = attempt
        .split_once("  else:\n")
        .ok_or("the guarded body has no failure arm")?;
    assert!(
        success_arm.contains("if is_tagged v4 ok/2:"),
        "the substep's fallible publish must be tested in place:\n{attempt}"
    );
    assert!(
        success_arm.contains("record(awl_outcome_failure,")
            && !failure_arm.contains("record(awl_outcome_failure,"),
        "the routed failure outcome belongs to the SUCCESS arm — a route is a tail, \
         never a fallible result:\n{attempt}"
    );
    assert!(
        failure_arm.contains("tail_local sub_publish_it_attempt_on_failure(")
            && !success_arm.contains("sub_publish_it_attempt_on_failure"),
        "compensation is reachable ONLY from the failure arm:\n{attempt}"
    );

    let compensation = printed
        .split("== fn sub_publish_it_attempt_on_failure/1")
        .nth(1)
        .and_then(|rest| rest.split("\n== fn ").next())
        .ok_or("the substep's compensation function is missing")?;
    assert!(
        compensation.contains("call_local cleanup_activity("),
        "the compensation must run the substep's cleanup:\n{compensation}"
    );
    assert!(
        compensation.contains("record(awl_outcome_failure,"),
        "the compensation's own `route finish` must fire the parent's arm from the \
         substep frame:\n{compensation}"
    );
    Ok(())
}

/// A substep's `max … visits` bound REFUSES rather than lowering, and the
/// refusal is anchored on the bound itself.
///
/// This is a divergence from the reference that runs the SAFE way, and it is
/// MEASURED rather than reasoned: `aion awl emit` on this exact source produces
/// `sub_publish_it_attempt` whose `True ->` arm is a bare
/// `sub_publish_it_attempt(target)` — an unbounded self-recursion where the
/// author wrote `max 3 visits`. `flowshape::rewrite_visits` recurses into
/// substeps and allocates the counter, but `emitter/subs.rs::emit_sub_chain`
/// emits no visit prologue, so nothing ever increments or tests it. Refusing is
/// loud; mirroring would be exactly the silent divergence the direct path
/// exists to avoid.
///
/// The guard here deliberately does NOT read `visits`. A guard that does drags
/// the counter into the entry region's parameters, where it is neither an input
/// nor a registered flow counter, and BOTH backends already refuse the document
/// at that earlier gate — which would make this test pass for the wrong reason.
/// No corpus fixture carries either shape, which is why this needs its own
/// source.
#[test]
fn a_substep_visit_bound_refuses_rather_than_going_unenforced()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "\
//! Focused pin: a `max … visits` bound the reference would drop on the floor.
workflow bounded_substep
  input target: String
  outcome done: type String, route success

worker shell
  action publish(target: String) -> String

step publish_it
  step attempt
    max 3 visits
    publish(target: target) -> receipt
    outcome again: when receipt == \"retry\", route attempt
    outcome give_up: otherwise, route finish

  outcome finish: otherwise, route done(\"stopped\")
";
    match lower_source(source)? {
        Err(LowerError::Unsupported { shape, span }) => {
            assert_eq!(
                shape, "a `max … visits` bound on a substep",
                "the substep visit-bound refusal class drifted"
            );
            assert_eq!(
                span,
                crate::Span {
                    start: 265,
                    end: 277,
                    line: 11,
                    column: 5
                },
                "the substep visit-bound refusal span drifted"
            );
        }
        other => {
            return Err(format!("a substep `max … visits` bound must refuse: {other:?}").into());
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

/// The retired `parallel region` refusal, flipped to a lowering + verify
/// coverage pin (the same promotion as `subflow_declarations_now_lower`). A
/// dependency-parallel layer arises from an `after` naming MORE THAN ONE step,
/// or from two steps declaring `after` the same predecessor; it is not a
/// keyword, which is why `dev_brief` hit it without containing any visibly
/// parallel syntax.
///
/// All four fixtures lower; none merely moved to a later refusal. They cover
/// both halves of the reference's classification: `after_multi_diamond`,
/// `dev_brief`, and `layer_hygiene` dispatch their layer concurrently, while
/// `release_pipeline_combined`'s layer degrades to written order because
/// `build_all`'s body is a collection fork rather than one bare action call.
#[test]
fn parallel_layer_fixtures_now_lower() -> Result<(), Box<dyn std::error::Error>> {
    for fixture in [
        "tests/fixtures/rev2/dag-fork/valid/after_multi_diamond.awl",
        "tests/fixtures/rev2/dag-fork/valid/release_pipeline_combined.awl",
        "tests/fixtures/rev2/flagship/valid/dev_brief.awl",
        "tests/fixtures/rev2/flow-shape/valid/layer_hygiene.awl",
    ] {
        let module = lower_fixture(&manifest_dir().join(fixture))??;
        super::verify(&module)?;
    }
    Ok(())
}

/// A parallelizable layer dispatches CONCURRENTLY: both members' activity
/// values reach ONE `aion@workflow:all/1`, and neither rides `workflow.run`.
/// The pin is two-directional on purpose — asserting only that `all/1` appears
/// would still pass on a lowering that ALSO ran each call sequentially first,
/// and asserting only the absence of `run/1` would pass on a body that
/// dispatched nothing at all.
///
/// Ordering is the risk this guards: a layer lowered in written order records
/// `gather -> extract_claims -> collect_citations` as three sequential
/// activity round trips, while the reference records one concurrent dispatch
/// of two. Both trails are internally consistent, so only the pairing of
/// presence and absence distinguishes them here; the BC-4 differential proves
/// the durable consequence against the Gleam oracle.
#[test]
fn a_parallel_layer_dispatches_through_one_workflow_all() -> Result<(), Box<dyn std::error::Error>>
{
    let path = manifest_dir().join("tests/fixtures/rev2/dag-fork/valid/after_multi_diamond.awl");
    let module = lower_fixture(&path)??;
    let printed = super::print_mir(&module);
    let layer = printed
        .split("== fn step_claims/1")
        .nth(1)
        .and_then(|rest| rest.split("\n== fn ").next())
        .ok_or("the diamond's parallel layer function is missing")?;
    assert_eq!(
        layer.matches("aion@workflow:all/1").count(),
        1,
        "the layer must dispatch through exactly one `workflow.all`:\n{layer}"
    );
    assert!(
        layer.contains("v7 = list([v3, v6])"),
        "both members' configured activity values must reach that one list:\n{layer}"
    );
    assert!(
        !layer.contains("aion@workflow:run/1"),
        "a concurrently dispatched layer must run NO member through \
         `workflow.run` — that is the written-order form:\n{layer}"
    );
    // The heterogeneous join decodes each position with THAT action's return
    // codec: a wrong-codec decode is validator-clean and runtime-wrong.
    for term in ["claims_codec()", "citations_codec()"] {
        assert!(
            layer.contains(term),
            "each bound position must decode with its own action's codec \
             (missing `{term}`):\n{layer}"
        );
    }
    Ok(())
}

/// A dependency-parallel layer the SDK cannot dispatch concurrently degrades
/// to written order and SAYS SO (S13). `release_pipeline_combined`'s layer
/// pairs `build_all` (a collection fork) with `notes` (a bare call), so the
/// reference falls back to written order and names the degradation in the
/// generated module; the direct path carries the same fact as
/// `degraded_parallel` on the layer's first member.
///
/// Two-directional: the marker must be on `build_all` and on NOTHING else, so
/// a lowering that marked every chain step — which would make the marker
/// meaningless — fails here, and so does one that dropped it.
#[test]
fn a_degraded_parallel_layer_is_marked_on_its_first_member_only()
-> Result<(), Box<dyn std::error::Error>> {
    let path =
        manifest_dir().join("tests/fixtures/rev2/dag-fork/valid/release_pipeline_combined.awl");
    let module = lower_fixture(&path)??;
    let printed = super::print_mir(&module);
    let marked: Vec<&str> = printed
        .split("== fn ")
        .skip(1)
        .filter(|body| body.contains("\n  degraded_parallel: true\n"))
        .filter_map(|body| body.split('/').next())
        .collect();
    assert_eq!(
        marked,
        vec!["step_build_all"],
        "exactly the degraded layer's FIRST member carries the S13 marker"
    );
    // The control arm: a fixture whose layer DOES dispatch concurrently must
    // carry no marker at all, so the marker tracks the degradation rather than
    // the mere presence of a multi-member layer.
    let parallel = lower_fixture(
        &manifest_dir().join("tests/fixtures/rev2/dag-fork/valid/after_multi_diamond.awl"),
    )??;
    assert!(
        !super::print_mir(&parallel).contains("degraded_parallel: true"),
        "a layer that DOES dispatch concurrently must carry no degradation marker"
    );
    Ok(())
}

/// R5: an ADVISORY call is not a layer-completion dependency, so a layer
/// containing one degrades to written order rather than joining a
/// `workflow.all` that would gate the layer on the advisory's outcome — the
/// reference's own gate (`emitter/steps.rs::layer_calls`).
///
/// No corpus fixture pairs an advisory call with a dependency-parallel layer,
/// so this is the only place that gate is exercised. Its control is the SAME
/// document with the `advisory` marker removed, which must dispatch
/// concurrently — the two arms differ by one word, so a classifier that
/// ignored `advisory` would fail here and a classifier that refused every
/// layer would fail on the control.
#[test]
fn an_advisory_call_keeps_its_layer_in_written_order() -> Result<(), Box<dyn std::error::Error>> {
    let template = "\
//! Focused advisory-in-a-parallel-layer pin.
workflow advisory_layer
  input seed: String
  outcome done: type String, route success

worker local
  action root(value: String) -> String
  action left(value: String) -> String
  action right(value: String) -> String{marker}
  action merge(left: String, right: String) -> String

step root
  root(value: seed) -> rooted

step left after root
  left(value: rooted) -> left_value

step right after root
  right(value: rooted)

step merge after left, right
  merge(left: left_value, right: left_value) -> joined
  route done(joined)
";
    let advisory = super::print_mir(&lower_source(
        &template.replace("{marker}", "\n    advisory"),
    )??);
    let control = super::print_mir(&lower_source(&template.replace("{marker}", ""))??);
    assert!(
        advisory.contains("degraded_parallel: true") && !advisory.contains("aion@workflow:all/1"),
        "a layer holding an advisory call must run in written order:\n{advisory}"
    );
    assert!(
        !control.contains("degraded_parallel: true") && control.contains("aion@workflow:all/1"),
        "the same layer WITHOUT the advisory marker must dispatch \
         concurrently:\n{control}"
    );
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

/// The retired `subflow` refusal, flipped to a lowering + verify coverage pin
/// (the same promotion as `value_route_payloads_now_lower`). `subflow` was the
/// LAST construct the driver's B4 staging gate held, so retiring it removed
/// that gate — and `flow_shape_gap` with it — entirely.
///
/// All three fixtures lower; none merely moved to a later refusal. `dev_flow`
/// is the one worth naming: it carries `step wave after plan`, and an `after`
/// on the IMMEDIATELY PRECEDING step is normalized to a fall-through edge by
/// `flowshape::normalize_adjacent_after` before planning, so it never reaches
/// the dependency-parallel layer refusal that `dev_brief` and `layer_hygiene`
/// still hit. It is pinned here because that is easy to mistake for the
/// parallel-layer gap having quietly weakened, which it has not —
/// `parallel_layer_fixtures_still_refuse` above is untouched by this landing.
#[test]
fn subflow_declarations_now_lower() -> Result<(), Box<dyn std::error::Error>> {
    for fixture in [
        "tests/fixtures/rev2/flow-shape/valid/dev_flow.awl",
        "tests/fixtures/rev2/flow-shape/valid/subflow_local_names.awl",
        "tests/fixtures/rev2/flow-shape/valid/subflow_nested.awl",
    ] {
        let module = lower_fixture(&manifest_dir().join(fixture))??;
        super::verify(&module)?;
    }
    Ok(())
}

/// A subflow's outcome payload is COERCED toward the subflow's DECLARED
/// outcome type, exactly as a workflow outcome's payload is: an `Int` routed
/// to an `Int?` subflow outcome must arrive wrapped in `Some`. A region member
/// flow's exit takes the collected binding raw, so an exit lowering that
/// treated a subflow the same way would hand the caller a bare integer where
/// the Gleam oracle hands it `Some(1)`.
///
/// No corpus fixture declares an optional subflow outcome, so this is the only
/// place that path is exercised. Pinned on the exact TERMS inside the
/// subflow's own flow function — not on the `some` atom merely appearing in
/// the pool, which an `Int?` codec interns regardless.
#[test]
fn a_subflow_outcome_payload_is_wrapped_for_an_optional_outcome()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "\
//! Focused optional-coercion pin for a subflow's outcome payload.
workflow subflow_optional_payload
  input docs: [Doc]
  outcome done: type Int?, route success

type Doc { title: String }

subflow tally(items: [Doc])
  outcome out: type Int?
  step count_them
    items |> count -> total
    route out(total)

step run_it
  tally(items: docs) -> counted

step finish
  counted |> route done
";
    let module = lower_source(source)??;
    super::verify(&module)?;
    let printed = super::print_mir(&module);
    let flow = printed
        .split("== fn awl_sf_tally_step_count_them/1")
        .nth(1)
        .ok_or("the subflow's flow function is missing")?;
    // The count must be wrapped, and the WRAPPED value must be what the exit
    // returns — the `Ok` carries `Some(count)`, never the bare count.
    for term in ["v2 = record(some, [v1])", "v3 = record(ok, [v2])"] {
        assert!(
            flow.contains(term),
            "an Int payload routed to an `Int?` SUBFLOW outcome must reach the exit \
             wrapped in `Some` (missing `{term}`):\n{flow}"
        );
    }
    // The other direction: the raw count must NOT be what the exit returns. A
    // `contains` check alone passes on a body that emits both, so the absence
    // of the unwrapped form is asserted too.
    assert!(
        !flow.contains("record(ok, [v1])"),
        "the subflow exit must not return the UNWRAPPED count:\n{flow}"
    );
    Ok(())
}

/// Each subflow owns an INDEPENDENT binding environment, so two subflows may
/// reuse the same author names for their parameters and outcomes at unrelated
/// types. The two wrappers below are each other's control: `echo` takes and
/// returns `String`, `same_number` takes and returns `Int`, and a lowering
/// that resolved either flow's names against the wrong environment (or against
/// the host's, which reuses `value` again) would mistype one of them.
#[test]
fn each_subflow_keeps_its_own_binding_environment() -> Result<(), Box<dyn std::error::Error>> {
    let path = manifest_dir().join("tests/fixtures/rev2/flow-shape/valid/subflow_local_names.awl");
    let module = lower_fixture(&path)??;
    super::verify(&module)?;
    let printed = super::print_mir(&module);
    for (header, signature) in [
        (
            "== fn awl_sf_echo/1 origin=subflow_wrapper(echo) ==",
            "sig: (String) -> Result(String, AwlError)",
        ),
        (
            "== fn awl_sf_same_number/1 origin=subflow_wrapper(same_number) ==",
            "sig: (Int) -> Result(Int, AwlError)",
        ),
    ] {
        let body = printed
            .split(header)
            .nth(1)
            .ok_or_else(|| format!("missing subflow wrapper `{header}`"))?;
        let first = body.lines().nth(1).unwrap_or_default().trim();
        assert_eq!(
            first, signature,
            "subflow wrapper `{header}` did not keep its own declared types"
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
