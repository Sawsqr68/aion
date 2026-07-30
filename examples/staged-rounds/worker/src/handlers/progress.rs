//! The progress channel the multi-minute gate batteries report on.
//!
//! WHY THIS EXISTS: `gate_item` and `verify_integration` run the operator's
//! real battery — `cargo build --workspace`, `cargo clippy --all-targets`,
//! `cargo test --workspace`. On a whole-project run those dominate wall-clock,
//! and a shell activity that reports nothing between dispatch and result gives
//! the operator a detail-free liveness beat for many minutes: the console shows
//! the activity start and then nothing until it finishes. So each gate
//! command's START and its RESULT are reported as heartbeat progress detail.
//!
//! LIVENESS IS NOT WHAT THIS BUYS. The worker runtime already heartbeats every
//! in-flight activity inside the server-assigned window, so a handler that
//! never reports still stays live for as long as it genuinely runs
//! (`ActivityContext::heartbeat`'s own contract). This is the PROGRESS channel:
//! it attaches the detail an operator can read.
//!
//! The seam is a trait, not a bare `ActivityContext`, for two reasons: the
//! handler bodies stay plain synchronous functions the hermetic tests drive
//! directly, and the tests can RECORD what would have been reported and assert
//! on it — the reports are proven to happen, not assumed.

use aion_integrations::Payload;
use aion_worker::ActivityContext;

/// The progress sink one gate battery reports each command's start and result
/// on.
pub trait GateProgress {
    /// Report one progress detail line. Reporting is best-effort by design —
    /// a battery must never fail because a progress beat could not be
    /// delivered — so an implementation that cannot deliver LOGS the reason
    /// and returns; it never swallows it silently and never propagates it
    /// into the battery's verdict.
    fn report(&self, detail: &str);
}

/// A sink that discards. Used by the hermetic tests that assert on the
/// battery's OUTCOME rather than its progress, so the intent is explicit at
/// every call site rather than a stray `()`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SilentProgress;

impl GateProgress for SilentProgress {
    fn report(&self, _detail: &str) {}
}

/// Reports onto the activity's heartbeat progress channel.
#[derive(Clone, Debug)]
pub struct HeartbeatProgress {
    context: ActivityContext,
}

impl HeartbeatProgress {
    /// Wrap one activity's context as a progress sink.
    #[must_use]
    pub fn new(context: ActivityContext) -> Self {
        Self { context }
    }
}

impl GateProgress for HeartbeatProgress {
    fn report(&self, detail: &str) {
        let payload = match gate_progress_payload(detail) {
            Ok(payload) => payload,
            Err(error) => {
                tracing::warn!(
                    %error,
                    %detail,
                    "could not encode the gate progress detail; the battery continues"
                );
                return;
            }
        };
        if let Err(error) = self.context.heartbeat(Some(payload)) {
            tracing::warn!(
                %error,
                %detail,
                "could not deliver the gate progress heartbeat; the battery continues"
            );
        }
    }
}

/// The heartbeat detail payload for one gate progress line: a JSON object
/// under a named key, so a console reading the progress detail sees a typed
/// field rather than having to guess at a bare string.
///
/// # Errors
///
/// Returns a message when the detail cannot be serialized.
pub fn gate_progress_payload(detail: &str) -> Result<Payload, String> {
    Payload::from_json(&serde_json::json!({ "gate_progress": detail }))
        .map_err(|error| format!("gate progress detail is not encodable: {error}"))
}

#[cfg(test)]
mod tests {
    use aion_integrations::ActivityId;
    use aion_worker::ActivityContext;

    use super::{GateProgress, HeartbeatProgress, SilentProgress, gate_progress_payload};

    #[test]
    fn the_progress_payload_names_its_field() -> anyhow::Result<()> {
        let payload =
            gate_progress_payload("gate `test` running: cargo test").map_err(anyhow::Error::msg)?;
        let value = payload.to_json()?;
        assert_eq!(value["gate_progress"], "gate `test` running: cargo test");
        Ok(())
    }

    #[test]
    fn a_context_without_a_live_seam_reports_without_failing() {
        // The no-op context every isolated test constructs: heartbeat is a
        // no-op, so a battery driven under it must not observe an error.
        let (context, _cancel) = ActivityContext::new(ActivityId::from_sequence_position(1), 1);
        HeartbeatProgress::new(context).report("gate `fmt` running");
    }

    #[test]
    fn the_silent_sink_discards() {
        SilentProgress.report("nothing happens");
    }
}
