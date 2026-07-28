//! The `on failure` compensation arm is EXECUTED, not merely lowered.
//!
//! `covered.rs` proves the two backends agree and pins the terminal bucket of
//! every covered fixture, but a bucket alone cannot say whether an arm RAN: a
//! failed activity that propagated straight out and a failed activity whose
//! compensation ran and routed a failure outcome both end in `WorkflowFailed`.
//! Unrun is unproven, so this suite reads the durable trail for the
//! compensation's OWN activity — an action that appears nowhere else in the
//! fixture — on BOTH productions of the same workflow.
//!
//! The same run then carries the REPLAY pin: a history whose compensation arm
//! executed replays from history alone with no divergence. The arm is ordinary
//! workflow code after an `ActivityFailed` that was recorded BEFORE the failure
//! value ever reached the program, so replay re-derives the identical value —
//! the same class as tolerant collect, no new determinism surface. This proves
//! it rather than asserting it.
//!
//! The failures are induced by the `fail_*` naming convention documented at
//! `fixtures::action_results`, shared by both runs by construction.

use aion_core::Event;

use crate::driver::build_spliced;
use crate::run::{Disposition, run_package};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// One arm-execution pin: the fixture, the activity only its compensation
/// block calls, and the disposition the compensated run must reach.
struct ArmPin {
    fixture: &'static str,
    /// The action ONLY the `on failure` block dispatches.
    compensation_activity: &'static str,
    /// The action whose induced failure opens the edge.
    failing_activity: &'static str,
    disposition: Disposition,
}

const ARM_PINS: &[ArmPin] = &[
    // Compensation routes a workflow FAILURE outcome.
    ArmPin {
        fixture: "flow-shape/valid/on_failure_compensates_to_outcome",
        compensation_activity: "snapshot",
        failing_activity: "fail_publish",
        disposition: Disposition::Failed,
    },
    // Compensation routes to a SIBLING STEP, whose refund completes the run —
    // a COMPLETED workflow reached only through the failure edge.
    ArmPin {
        fixture: "flow-shape/valid/on_failure_compensates_to_step",
        compensation_activity: "note_failure",
        failing_activity: "fail_charge",
        disposition: Disposition::Completed,
    },
];

/// Both productions of each fixture record the induced activity failure AND
/// then dispatch the compensation block's own activity — positive evidence
/// that the arm executed, on the direct backend and on the reference alike.
#[tokio::test(flavor = "multi_thread")]
async fn compensation_arms_execute_on_both_backends() -> TestResult {
    if crate::gleam_test_support::skip_if_unavailable() {
        return Ok(());
    }
    let names: Vec<String> = ARM_PINS.iter().map(|pin| pin.fixture.to_owned()).collect();
    let fixtures = build_spliced(&names, "compensation").await?;
    assert_eq!(
        fixtures.len(),
        ARM_PINS.len(),
        "every compensation fixture must build both backends"
    );

    for (pin, fixture) in ARM_PINS.iter().zip(&fixtures) {
        for (label, package, entry) in [
            (
                "direct",
                fixture.direct_package.clone(),
                fixture.entry_bytes.clone(),
            ),
            (
                "reference",
                fixture.reference_package.clone(),
                reference_entry(fixture)?,
            ),
        ] {
            let outcome = run_package(
                package,
                &fixture.entry_module,
                &fixture.input,
                fixture.action_results.clone(),
                &entry,
            )
            .await?;
            let name = pin.fixture;
            assert_eq!(
                outcome.disposition, pin.disposition,
                "{name} ({label}) reached the wrong disposition"
            );
            assert!(
                scheduled(&outcome.trail, pin.failing_activity),
                "{name} ({label}): the guarded body never dispatched `{}`",
                pin.failing_activity
            );
            assert!(
                outcome
                    .trail
                    .iter()
                    .any(|event| matches!(event, Event::ActivityFailed { .. })),
                "{name} ({label}): no activity failure was recorded, so the edge \
                 was never opened"
            );
            // THE pin: the compensation block's own activity was dispatched.
            // It appears nowhere else in the fixture, so nothing but the
            // executed arm can put it in the trail.
            assert!(
                scheduled(&outcome.trail, pin.compensation_activity),
                "{name} ({label}): the compensation activity `{}` never ran — the \
                 arm was lowered but not executed",
                pin.compensation_activity
            );
            // Replay pin: the compensated history replays byte-equal from
            // history alone.
            let run_id = started_run_id(&outcome.trail)
                .ok_or_else(|| format!("{name} ({label}): no WorkflowStarted in the trail"))?;
            let inspection =
                aion::durability::replay_inspect::inspect_run(outcome.trail.clone(), &run_id)?;
            assert!(
                inspection.divergence.is_none(),
                "{name} ({label}): a compensated run must replay from history alone \
                 with no divergence: {:#?}",
                inspection.divergence
            );
        }
    }
    Ok(())
}

/// The reference package's entry beam, for the splice binding `run_package`
/// enforces.
fn reference_entry(
    fixture: &crate::driver::SplicedFixture,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    fixture
        .reference_package
        .beams()
        .get(&fixture.entry_module)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| {
            format!(
                "reference package has no entry beam `{}`",
                fixture.entry_module
            )
            .into()
        })
}

/// The run id the trail's `WorkflowStarted` opened.
fn started_run_id(trail: &[Event]) -> Option<aion_core::RunId> {
    trail.iter().find_map(|event| match event {
        Event::WorkflowStarted { run_id, .. } => Some(run_id.clone()),
        _ => None,
    })
}

/// Whether the trail records an `ActivityScheduled` for this activity type.
fn scheduled(trail: &[Event], activity_type: &str) -> bool {
    trail.iter().any(|event| match event {
        Event::ActivityScheduled {
            activity_type: scheduled,
            ..
        } => scheduled == activity_type,
        _ => false,
    })
}
