//! Focused wire mappings for nested engine error families.

use aion::EngineError;
use aion::durability::DurabilityError;
use aion_proto::WireError;
use aion_store::StoreError;

pub(super) fn invalid_state_wire(reason: &str) -> WireError {
    WireError::invalid_state_with_type("InvalidState", reason.to_owned())
}

pub(super) fn backend_wire(error_type: &'static str, source: &EngineError) -> WireError {
    WireError::backend_with_type(error_type, source.to_string())
}

pub(super) fn contract_refusal_wire(error_type: &'static str, source: &EngineError) -> WireError {
    WireError::invalid_state(source.to_string()).with_error_type(error_type)
}

/// Wire mapping for a declared-contract boundary refusal — a start input or a
/// signal payload that did not satisfy the type the package declared.
///
/// These are the CALLER's to fix and cost the target run nothing: the refusal
/// is returned before anything is recorded and before any arrival is consumed.
/// So they are invalid input (HTTP 400), never a backend fault and never a
/// state conflict.
pub(super) fn declared_contract_wire(error_type: &'static str, source: &EngineError) -> WireError {
    WireError::invalid_input(source.to_string()).with_error_type(error_type)
}

pub(super) const fn store_error_type(source: &StoreError) -> &'static str {
    match source {
        StoreError::SequenceConflict { .. } => "SequenceConflict",
        StoreError::NotFound { .. } => "NotFound",
        StoreError::NotOwner { .. } => "NotOwner",
        StoreError::Backend(_) => "Backend",
        StoreError::Serialization(_) => "Serialization",
    }
}

pub(super) fn durability_wire(durability: &DurabilityError, source: &EngineError) -> WireError {
    match durability {
        DurabilityError::Store(store) => super::wire_from_store(store),
        DurabilityError::NonDeterminism(_)
        | DurabilityError::HistoryShape { .. }
        | DurabilityError::SearchAttribute(_) => {
            WireError::backend_with_type("Durability", source.to_string())
        }
        // Same wire name as [`aion::EngineError::EngineTaskEpochClosed`] (see
        // `error.rs`'s engine mapping), because a caller is being told about one
        // condition — this engine has begun closing — and which internal seam
        // raised it is not theirs to act on. The two Rust variants stay separate
        // for a reason that lives on the Rust side, not on the wire: the engine
        // variant carries a documented single-construction-site invariant.
        //
        // 🔴 WHY THIS DISCRIMINANT IS DROPPED WHEN A SIBLING CHANGE INSISTS ON
        // KEEPING ONE. `runtime/nif_timer_fire.rs` introduces `RefusedAppend`
        // expressly so a refusal's cause CANNOT be dropped — its own comment
        // says collapsing one "let one sentence be raised for several cases".
        // Landing both in one changeset looks like arguing both sides, so the
        // rule that separates them is stated here rather than left to be
        // inferred.
        //
        // **A discriminant travels when it changes what the recipient should
        // DO.** `RefusedAppend` decides which SENTENCE an operator reads, and
        // the three cases it separates carry three different remedies: wait for
        // workflow code to reissue, check the run's status, or nothing at all.
        // Collapse them and the operator is sent to the wrong one. Here the two
        // variants share a single condition and a single remedy — this engine
        // is closing; the run is untouched; a successor picks it up — so the
        // seam of origin changes nothing a caller can act on, and splitting the
        // label would only fragment the trace an operator searches. Keeping a
        // distinction that cannot alter a decision is not caution; it is a
        // second name for one fact, which is the drift this codebase treats as
        // a defect in its own right.
        //
        // Reached only if a durable write refused by the epoch is ever surfaced
        // through a transport operation. Today the one construction site is the
        // continue-as-new NIF, whose error returns into workflow code rather
        // than into a response — but this mapping is total over the type, so it
        // decides rather than assuming that stays true.
        DurabilityError::EngineTaskEpochClosed { .. } => {
            WireError::backend_with_type("EngineTaskEpochClosed", source.to_string())
        }
    }
}

/// Trace and wire discriminator for live-query dispatch failures.
pub(super) fn query_error_type(source: &aion::QueryError) -> &'static str {
    match source {
        aion::QueryError::UnknownQuery(_) => "UnknownQuery",
        aion::QueryError::Timeout => "QueryTimeout",
        aion::QueryError::NotRunning(_) => "QueryNotRunning",
        aion::QueryError::Unknown(_) => "QueryUnknownWorkflow",
        aion::QueryError::ReplyDropped => "QueryReplyDropped",
        aion::QueryError::HandlerFailed { .. } => "QueryFailed",
        aion::QueryError::Engine(_) => "QueryEngine",
    }
}

/// Wire mapping for live-query dispatch failures.
pub(super) fn query_wire(query: &aion::QueryError, source: &EngineError) -> WireError {
    match query {
        aion::QueryError::UnknownQuery(_) => WireError::unknown_query(source.to_string()),
        aion::QueryError::Timeout => WireError::query_timeout(source.to_string()),
        aion::QueryError::NotRunning(_) | aion::QueryError::ReplyDropped => {
            WireError::not_running_with_type(query_error_type(query), source.to_string())
        }
        aion::QueryError::Unknown(_) => {
            WireError::not_found_with_type(query_error_type(query), source.to_string())
        }
        aion::QueryError::HandlerFailed { .. } => {
            WireError::query_failed(source.to_string()).with_error_type(query_error_type(query))
        }
        aion::QueryError::Engine(_) => {
            WireError::backend_with_type(query_error_type(query), source.to_string())
        }
    }
}
