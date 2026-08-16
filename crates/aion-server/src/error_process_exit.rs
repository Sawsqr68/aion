//! Stable server trace and wire mappings for process-exit drainer failures.

use std::borrow::Cow;

use aion::EngineError;
use aion_proto::WireError;

use super::ErrorTraceFields;

pub(super) fn trace<'a>(error_type: &'static str, source: &'a EngineError) -> ErrorTraceFields<'a> {
    ErrorTraceFields {
        error_type: Cow::Borrowed(error_type),
        store_error_type: None,
        reason: source,
    }
}

pub(super) fn wire(error_type: &'static str, source: &EngineError) -> WireError {
    WireError::backend_with_type(error_type, source.to_string())
}

pub(super) fn drainer_trace(source: &EngineError) -> ErrorTraceFields<'_> {
    trace(drainer_error_type(source), source)
}

pub(super) fn drainer_wire(source: &EngineError) -> WireError {
    wire(drainer_error_type(source), source)
}

/// This helper is called only from the exhaustive drainer arms in `error.rs`
/// (one grouped arm each in the trace and wire matches, so the label mapping
/// lives in exactly one place); the fallback remains an honest family name for
/// future variants.
fn drainer_error_type(source: &EngineError) -> &'static str {
    match source {
        EngineError::ProcessExitStatePoisoned { .. } => "ProcessExitStatePoisoned",
        EngineError::ProcessExitSubscriptionUnavailable => "ProcessExitSubscriptionUnavailable",
        EngineError::ProcessExitDrainerSpawn { .. } => "ProcessExitDrainerSpawn",
        EngineError::ProcessExitDrainerPoisoned => "ProcessExitDrainerPoisoned",
        EngineError::ProcessExitOutcomeMissingAfterEvent { .. } => {
            "ProcessExitOutcomeMissingAfterEvent"
        }
        EngineError::ProcessExitEventStreamDisconnected => "ProcessExitEventStreamDisconnected",
        EngineError::ProcessExitDrainerShutdownTimedOut { .. } => {
            "ProcessExitDrainerShutdownTimedOut"
        }
        EngineError::ProcessExitDrainerPanicked => "ProcessExitDrainerPanicked",
        _ => "ProcessExitDrainer",
    }
}

pub(super) fn callback_trace(source: &EngineError) -> ErrorTraceFields<'_> {
    trace(callback_error_type(source), source)
}

pub(super) fn callback_wire(source: &EngineError) -> WireError {
    wire(callback_error_type(source), source)
}

/// This helper is called only from the exhaustive callback-dispatcher arm in
/// `error.rs`; the fallback remains an honest family name for future variants.
fn callback_error_type(source: &EngineError) -> &'static str {
    match source {
        EngineError::ProcessExitCallbackDispatcherPoisoned => {
            "ProcessExitCallbackDispatcherPoisoned"
        }
        EngineError::ProcessExitCallbackDispatcherUnavailable => {
            "ProcessExitCallbackDispatcherUnavailable"
        }
        EngineError::ProcessExitCallbackDispatcherShutdownTimedOut { .. } => {
            "ProcessExitCallbackDispatcherShutdownTimedOut"
        }
        _ => "ProcessExitCallbackDispatcher",
    }
}
