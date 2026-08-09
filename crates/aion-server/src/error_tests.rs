use super::{ServerError, StreamFailure};
use aion::{EngineError, QueryError, engine_seam::EngineSeamError};
use aion_core::WorkflowId;
use aion_proto::WireErrorCode;

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn server_error_is_send_sync() {
    assert_send_sync::<ServerError>();
}

#[test]
fn lagged_stream_maps_to_wire_lagged() {
    let error = ServerError::Stream {
        failure: StreamFailure::Lagged,
    };

    assert_eq!(error.to_wire_error().code, WireErrorCode::Lagged);
}

#[test]
fn missing_event_outcome_invariant_maps_to_typed_backend() {
    let error = ServerError::EngineCall {
        source: EngineError::ProcessExitOutcomeMissingAfterEvent { process_id: 23 },
    };

    let wire = error.to_wire_error();
    assert_eq!(wire.code, WireErrorCode::Backend);
    assert_eq!(
        wire.error_type.as_deref(),
        Some("ProcessExitOutcomeMissingAfterEvent")
    );
    assert_eq!(
        error.trace_fields().error_type,
        "ProcessExitOutcomeMissingAfterEvent"
    );
}

#[test]
fn activity_delivery_poison_maps_to_typed_backend() {
    let error = ServerError::EngineCall {
        source: EngineError::ActivityDeliveryPoisoned { process_id: 17 },
    };

    let wire = error.to_wire_error();
    assert_eq!(wire.code, WireErrorCode::Backend);
    assert_eq!(wire.error_type.as_deref(), Some("ActivityDeliveryPoisoned"));
    assert_eq!(error.trace_fields().error_type, "ActivityDeliveryPoisoned");
}

/// R-0: a fenced quorum write (`StoreError::NotOwner`) must surface as the
/// typed, retryable `NotOwner` wire code with a `NotOwner` `error_type`, NOT
/// the opaque `Backend` it used to collapse into.
#[test]
fn not_owner_store_error_maps_to_wire_not_owner() {
    let error = ServerError::StoreBackend {
        source: aion_store::StoreError::NotOwner { shard: 3 },
    };
    let wire = error.to_wire_error();
    assert_eq!(wire.code, WireErrorCode::NotOwner);
    assert_eq!(wire.error_type.as_deref(), Some("NotOwner"));
}

fn workflow_id() -> WorkflowId {
    WorkflowId::new(uuid::Uuid::from_u128(7))
}

fn query_wire(query: QueryError) -> aion_proto::WireError {
    ServerError::EngineCall {
        source: EngineError::Query(query),
    }
    .to_wire_error()
}

/// Pins the wire mapping for every `QueryError` arm (#45 decisions
/// Q1(b)/Q3): adding a variant breaks the exhaustive list below until its
/// mapping is decided and pinned here.
#[test]
fn every_query_error_arm_maps_to_its_pinned_wire_code() {
    let arms: Vec<(QueryError, WireErrorCode, Option<&str>)> = vec![
        (
            QueryError::UnknownQuery(String::from("state")),
            WireErrorCode::UnknownQuery,
            None,
        ),
        (QueryError::Timeout, WireErrorCode::QueryTimeout, None),
        (
            QueryError::NotRunning(workflow_id()),
            WireErrorCode::NotRunning,
            Some("QueryNotRunning"),
        ),
        (
            QueryError::Unknown(workflow_id()),
            WireErrorCode::NotFound,
            Some("QueryUnknownWorkflow"),
        ),
        // Q3: the workflow ended before answering — not_running, not backend.
        (
            QueryError::ReplyDropped,
            WireErrorCode::NotRunning,
            Some("QueryReplyDropped"),
        ),
        // Q1(b): the dedicated query_failed wire code.
        (
            QueryError::HandlerFailed {
                message: String::from("handler raised"),
            },
            WireErrorCode::QueryFailed,
            Some("QueryFailed"),
        ),
        // The caller's own arguments were malformed: a request defect, so it
        // maps to `invalid_input` — distinct from the handler having run and
        // failed (`query_failed`) and from an engine fault (`backend`).
        (
            QueryError::InvalidArguments {
                reason: String::from("arguments payload is not a well-formed JSON document"),
            },
            WireErrorCode::InvalidInput,
            Some("QueryInvalidArguments"),
        ),
        (
            QueryError::Engine(EngineSeamError::Delivery {
                reason: String::from("mailbox closed"),
            }),
            WireErrorCode::Backend,
            Some("QueryEngine"),
        ),
    ];

    // Count-lock: the pin list must grow with the enum. The exhaustive
    // match below numbers every variant; a new variant breaks the match
    // first, and updating the match without pinning the new mapping
    // breaks this assertion.
    let variant_count = arms
        .iter()
        .map(|(query, _, _)| match query {
            QueryError::UnknownQuery(_) => 0,
            QueryError::Timeout => 1,
            QueryError::NotRunning(_) => 2,
            QueryError::Unknown(_) => 3,
            QueryError::ReplyDropped => 4,
            QueryError::HandlerFailed { .. } => 5,
            QueryError::InvalidArguments { .. } => 6,
            QueryError::Engine(_) => 7,
        })
        .collect::<std::collections::BTreeSet<usize>>()
        .len();
    assert_eq!(
        arms.len(),
        variant_count,
        "every QueryError variant must appear exactly once in the pin list",
    );
    assert_eq!(variant_count, 8, "pin list must cover all 8 variants");

    for (query, expected_code, expected_type) in arms {
        let wire = query_wire(query.clone());
        assert_eq!(
            wire.code, expected_code,
            "{query:?} must map to {expected_code:?}",
        );
        assert_eq!(
            wire.error_type.as_deref(),
            expected_type,
            "{query:?} must carry error_type {expected_type:?}",
        );
    }
}

/// Both halves of `EngineTaskEpochClosed`'s server mapping — the wire error and
/// the trace fields — pinned together, because they were added together and
/// neither had a test.
///
/// 🔴 The class assertion is the load-bearing one and it is `Backend`, not
/// `NotRunning`. The variant's own doc (`crates/aion/src/error.rs:389`) says
/// "in both cases THE RUN STAYS `Running`", so `not_running` would be a false
/// statement about the target dressed as a classification — and the CLI renders
/// that class as "the target run is no longer running", which would send an
/// operator looking for a terminal that never landed. A mutation putting
/// `not_running_with_type` back turns this red.
///
/// The `error_type` is asserted on BOTH halves against the same literal, since
/// the discriminator's whole purpose is to let an operator join a log line to a
/// client branch, and two sides that name the variant differently cannot be
/// joined at all.
#[test]
fn engine_task_epoch_closed_maps_to_typed_backend_on_both_halves() {
    let error = ServerError::EngineCall {
        source: EngineError::EngineTaskEpochClosed {
            workflow_id: String::from("orders"),
            run_id: String::from("019000ff-0000-7000-8000-000000000001"),
        },
    };

    let wire = error.to_wire_error();
    assert_eq!(
        wire.code,
        WireErrorCode::Backend,
        "an epoch-closed refusal must not claim the run stopped running"
    );
    assert_eq!(wire.error_type.as_deref(), Some("EngineTaskEpochClosed"));
    assert_eq!(error.trace_fields().error_type, "EngineTaskEpochClosed");

    // The refused run is named in the message an operator actually reads —
    // without it the class alone says a server failed, not WHICH run needs a
    // sweep to repair it.
    assert!(
        wire.message.contains("orders"),
        "the wire message dropped the workflow the refusal is about: {}",
        wire.message
    );
}

/// The OTHER `EngineTaskEpochClosed` — the durability-level one — reaching the
/// same two server surfaces, and landing on the same name.
///
/// 🔴 THIS TEST EXISTS BECAUSE THE MISTAKE ABOVE REPEATED ITSELF. The sibling
/// test's own opening line says its two halves were "added together and neither
/// had a test". The continue-as-new epoch gate then added two MORE halves of the
/// same mapping — `DurabilityError::EngineTaskEpochClosed` through
/// `durability_wire` and through `durability_trace_fields` — and again shipped
/// no test with them. A mutation swapping either to `not_running_with_type`, or
/// to the generic `"Durability"` label, passed the whole tree.
///
/// **The label is the load-bearing assertion, and it is deliberately the SAME
/// literal the engine-level variant gets.** Two Rust variants, one wire name, on
/// purpose: a caller is being told one thing — this engine has begun closing —
/// and which internal seam raised it is not theirs to act on. An operator
/// searching traces for "did this engine begin closing" has to find both seams
/// under one name, not one name and a generic bucket. Asserting the literal on
/// BOTH halves is what makes a trace line joinable to a client branch at all.
///
/// The class is `Backend` for the same reason it is above: the run stays
/// `Running`, so `NotRunning` would be a false statement about the target
/// dressed up as a classification.
#[test]
fn durability_epoch_closed_maps_to_the_same_name_on_both_halves() {
    use aion::durability::DurabilityError;

    let error = ServerError::EngineCall {
        source: EngineError::Durability(DurabilityError::EngineTaskEpochClosed {
            reason: String::from(
                "continue_as_new refused for run 019000ff-0000-7000-8000-000000000002",
            ),
        }),
    };

    let wire = error.to_wire_error();
    assert_eq!(
        wire.code,
        WireErrorCode::Backend,
        "a durability-level epoch refusal leaves the run Running, so it must not be classified \
         as NotRunning"
    );
    assert_eq!(
        wire.error_type.as_deref(),
        Some("EngineTaskEpochClosed"),
        "the wire half must carry the SAME discriminator as the engine-level variant — a caller \
         branching on the condition cannot branch on two names for it"
    );
    assert_eq!(
        error.trace_fields().error_type,
        "EngineTaskEpochClosed",
        "the trace half must carry it too, and must not fall back to the generic `Durability` \
         label — an operator searching for one engine's shutdown would miss this seam entirely"
    );

    // CONTROL: a sibling `DurabilityError` must NOT pick up this name, or the
    // assertions above would pass for a mapping that labelled everything the
    // same and measured nothing.
    let other = ServerError::EngineCall {
        source: EngineError::Durability(DurabilityError::HistoryShape {
            reason: String::from("history shape"),
        }),
    };
    assert_eq!(
        other.trace_fields().error_type,
        "Durability",
        "control: an ordinary durability fault keeps the generic label, so the epoch's own label \
         above is attributable to the epoch and not to the family"
    );

    // The refusal's own words survive to the operator. Without this a correct
    // class and a correct label still hand over nothing actionable.
    assert!(
        wire.message
            .contains("019000ff-0000-7000-8000-000000000002"),
        "the wire message dropped the run the refusal is about: {}",
        wire.message
    );
}

/// The trace discriminator for `HandlerFailed` matches the wire
/// `error_type` so operators can correlate logs with client branches.
#[test]
fn handler_failed_trace_fields_use_query_failed_type() {
    let error = ServerError::EngineCall {
        source: EngineError::Query(QueryError::HandlerFailed {
            message: String::from("handler raised"),
        }),
    };

    assert_eq!(error.trace_fields().error_type, "QueryFailed");
}
