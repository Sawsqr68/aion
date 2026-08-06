//! #147: a contract refusal on the gRPC registration path must name itself.
//!
//! Measured 2026-08-06, isolated world, `aion 0.11.0`: a shell worker whose
//! queue had accumulated a second document's action was refused at every
//! restart with **0 bytes** in the worker's log and **0 ERROR / 0 WARN** in the
//! server's. The refusal is fully diagnosed — `contracts.rs` builds a
//! field-level `Mismatch` whose own doc comment calls the message *"the only
//! thing standing between a two-minute fix and an hour of guessing"* — and
//! `worker_grpc.rs` maps it straight to a `Status` and returns. No `tracing`
//! call on the path. The diagnosis is produced and delivered only to a worker
//! that installs no subscriber.
//!
//! The liminal transport does not have this defect: it names every refusal at
//! WARN, and `dead_man_switch_refusal_logs.rs` pins it. That fix was written on
//! 2026-07-29 for the SAME phenomenon — `admit_registration`'s doc comment
//! records a worker *"refused on every single dial with the server silent on
//! all of them"* — and it landed in one CALLER instead of in the shared gate,
//! so the other transport re-manifested it verbatim within a week with nothing
//! in the tree able to notice.
//!
//! ⇒ These tests assert on the GATE's line, not on any caller's, because the
//! naming belongs where the refusal is decided. A per-caller log is a
//! convention a third transport can forget.
//!
//! This binary is separate from `worker_contract_registration` because the
//! tonic server runs inside a `tokio::spawn`: only a globally installed
//! subscriber sees its events, and a global subscriber is process-wide.

use std::sync::{Arc, Mutex};

use tracing::Level;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::registry::Registry;
use tracing_subscriber::util::SubscriberInitExt as _;

#[path = "test_support/capture.rs"]
mod capture;
#[path = "test_support/grpc_admission.rs"]
mod grpc_admission;

use capture::{CaptureLayer, CapturedEvent, capture_log, render};
use grpc_admission::{TestResult, mismatched_fetch, register_as, state_with_contract};

/// The message the GATE emits when it refuses a worker's contract.
const REFUSAL: &str = "REFUSED worker registration";

/// Install the process-global capture. Called once per binary; a second call is
/// a test-authoring error, not a runtime condition, so it is reported loudly.
fn install_capture() -> TestResult<Arc<Mutex<Vec<CapturedEvent>>>> {
    let events = capture_log();
    Registry::default()
        .with(CaptureLayer {
            events: Arc::clone(&events),
        })
        .try_init()
        .map_err(|error| format!("global subscriber already installed: {error}"))?;
    Ok(events)
}

fn refusals(events: &[CapturedEvent]) -> Vec<&CapturedEvent> {
    events
        .iter()
        .filter(|event| event.message.contains(REFUSAL))
        .collect()
}

/// Everything captured, snapshotted. Reading under the lock and cloning keeps
/// the assertions off a live buffer another task may still be appending to.
fn snapshot(events: &Arc<Mutex<Vec<CapturedEvent>>>) -> TestResult<Vec<CapturedEvent>> {
    Ok(events
        .lock()
        .map_err(|_| "capture log poisoned")?
        .clone())
}

/// R1 and R1a in one binary, in one sequence, because the transition rule is a
/// property of a SERIES of dials and a global subscriber cannot be partitioned
/// between concurrently running tests. Each phase states what it proves.
#[tokio::test(flavor = "multi_thread")]
async fn the_gate_names_its_refusal_once_and_again_when_the_reason_changes() -> TestResult {
    let events = install_capture()?;
    let (state, action) = state_with_contract().await?;

    // ---- PHASE 1 (R1): the refusal is named at all ----------------------
    // RED at HEAD: worker_grpc.rs:67 propagates the diagnosis to the wire with
    // no tracing call, so this finds nothing.
    let first = register_as(
        state.clone(),
        String::new(),
        vec![mismatched_fetch(&action)],
        "build-a",
    )
    .await?;
    assert!(
        first.is_err(),
        "the fixture must actually be refused, or every assertion below is \
         vacuous — a test whose subject never happened reads exactly like a pass"
    );

    let captured = snapshot(&events)?;
    let named = refusals(&captured);
    let refusal = named.first().ok_or_else(|| {
        format!(
            "the server refused the registration and logged NOTHING about it; \
             captured: {}",
            render(&captured)
        )
    })?;
    assert_eq!(
        refusal.level,
        Level::WARN,
        "a refusal an operator must act on belongs at WARN: {refusal:?}"
    );

    // The whole point of naming it is that the operator can act. A line that
    // says "refused" without saying WHICH queue, WHICH build, or WHY is a
    // notification, not a diagnosis.
    let reason = refusal.fields.get("reason").map_or("", String::as_str);
    assert!(
        reason.contains("WORKER_CONTRACT_MISMATCH"),
        "the logged reason must carry the gate's own diagnosis: {refusal:?}"
    );
    assert!(
        reason.contains("input_schema"),
        "the field-level diff is the actionable part and must survive into the \
         log: {refusal:?}"
    );
    assert_eq!(
        refusal.fields.get("task_queue").map(String::as_str),
        Some("typed_queue"),
        "the queue names which deploy caused this: {refusal:?}"
    );
    assert_eq!(
        refusal.fields.get("identity").map(String::as_str),
        Some("build-a"),
        "the worker build identity names WHICH process to fix: {refusal:?}"
    );
    assert_eq!(named.len(), 1, "one refusal, one line: {:?}", named);

    // ---- PHASE 2 (R1a): an identical repeat says nothing new ------------
    // A refused worker redials forever (#94: 2/second, 12 MB/hour on the
    // liminal path). Repeats carry no information the first line did not, so
    // they are silence. This is a semantic, NOT an interval or a cap: nothing
    // here is timed and nothing is counted down.
    for _ in 0..3 {
        let repeat = register_as(
            state.clone(),
            String::new(),
            vec![mismatched_fetch(&action)],
            "build-a",
        )
        .await?;
        assert!(repeat.is_err(), "the repeat dials must still be refused");
    }
    let captured = snapshot(&events)?;
    assert_eq!(
        refusals(&captured).len(),
        1,
        "three identical redials re-stated a diagnosis that had not changed; \
         captured: {}",
        render(&captured)
    );

    // ---- PHASE 3 (R1a): a CHANGED reason speaks again -------------------
    // Without this the rule is indistinguishable from never logging at all —
    // the mutation has to sit on the path it claims to guard.
    let changed = register_as(
        state.clone(),
        String::new(),
        vec![tonic_descriptor_with_wrong_output(&action)],
        "build-a",
    )
    .await?;
    assert!(changed.is_err(), "the changed advertisement is still refused");
    let captured = snapshot(&events)?;
    let named = refusals(&captured);
    assert_eq!(
        named.len(),
        2,
        "the refusal reason changed and the gate stayed silent — suppression \
         that cannot tell a new fault from an old one is just muting; \
         captured: {}",
        render(&captured)
    );

    // ---- PHASE 4 (R1a): a DIFFERENT worker build speaks for itself ------
    // Keyed on (identity, queue, node): another operator's process is not a
    // repeat of this one, however identical its complaint.
    let other = register_as(
        state.clone(),
        String::new(),
        vec![mismatched_fetch(&action)],
        "build-b",
    )
    .await?;
    assert!(other.is_err(), "the second build is refused too");
    let captured = snapshot(&events)?;
    let named = refusals(&captured);
    assert_eq!(
        named.len(),
        3,
        "a second worker build was refused and inherited the first one's \
         silence; captured: {}",
        render(&captured)
    );
    assert!(
        named
            .iter()
            .any(|event| event.fields.get("identity").map(String::as_str) == Some("build-b")),
        "the third line must name the build it is about: {named:?}"
    );
    Ok(())
}

/// A second, DIFFERENT contract disagreement on the same action: the output
/// schema is wrong rather than the input. Used to move the refusal's reason
/// without moving the identity, queue, or node it is keyed on.
fn tonic_descriptor_with_wrong_output(
    action: &aion_package::ActionContract,
) -> aion_proto::generated::ActivityDescriptor {
    aion_proto::generated::ActivityDescriptor {
        name: "fetch".to_owned(),
        input_schema_json: action.input_schema.to_string(),
        output_schema_json: serde_json::json!({"type": "string"}).to_string(),
    }
}
