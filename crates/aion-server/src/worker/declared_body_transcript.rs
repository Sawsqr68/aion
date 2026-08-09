//! The declared command's output, into the transcript the server already
//! serves.
//!
//! A declared body executes IN the server process, so there is no worker drain
//! between the command and the sequencer: the activity's transcript seam is
//! drained here and published straight through
//! [`ActivityEventPublisher`](crate::activity_publisher::ActivityEventPublisher),
//! which is the same durable append, retention bound, and live fan-out an agent
//! step's transcript goes through after crossing the wire. Downstream — the
//! cursor reads, the WS tail, the ops console — cannot tell the two apart, and
//! is not meant to.
//!
//! # Losing a transcript never fails a command
//!
//! Publishing is observability, and the command's replay-authoritative result is
//! its own. A store that refuses an event costs a log line and the event, never
//! the activity: the first failure of a run names the reason, the rest are
//! counted, and the total is reported when the command ends. That is the same
//! coalescing discipline the worker-side drain uses, for the same reason — a
//! failing sink must not turn one line of output into one log line per line of
//! output.

use aion_core::ActivityEvent;
use tokio::sync::mpsc;

use crate::activity_publisher::ActivityEventPublisher;

/// Drain `events` into `publisher` until the activity's transcript seam closes.
///
/// Returns when the sender is dropped — which the declared-command executor does
/// as soon as its command has finished — so the caller can await this and know
/// every observed line has been offered to the sequencer.
pub(crate) async fn publish_declared_transcript(
    publisher: ActivityEventPublisher,
    mut events: mpsc::UnboundedReceiver<ActivityEvent>,
) {
    let mut dropped: u64 = 0;
    while let Some(event) = events.recv().await {
        if let Err(error) = publisher.publish(&event).await {
            if dropped == 0 {
                tracing::warn!(
                    %error,
                    operation = "declared_command_dispatch",
                    workflow_id = %event.workflow_id,
                    activity_id = %event.activity_id,
                    attempt = event.attempt,
                    "declared command transcript: the sequencer refused an output line; the \
                     command is unaffected and further refusals are counted, not logged"
                );
            }
            dropped = dropped.saturating_add(1);
        }
    }
    if dropped > 0 {
        tracing::warn!(
            dropped,
            operation = "declared_command_dispatch",
            "declared command transcript: output lines were not retained"
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use aion_core::{ActivityEvent, ActivityEventKind, ActivityId, MessageRole, WorkflowId};
    use aion_store::{ActivityStreamKey, InMemoryObservabilityStore};
    use uuid::Uuid;

    use super::publish_declared_transcript;
    use crate::activity_publisher::ActivityEventPublisher;

    /// What a test returns. Every fallible step is carried rather than
    /// unwrapped, because the workspace denies panicking accessors in test code
    /// as firmly as in library code.
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn line_event(worker_seq: u64, text: &str) -> ActivityEvent {
        ActivityEvent {
            workflow_id: WorkflowId::new(Uuid::from_u128(7)),
            activity_id: ActivityId::from_sequence_position(2),
            attempt: 1,
            agent_id: Uuid::nil(),
            agent_role: "command stdout".to_owned(),
            emitted_at: chrono::Utc::now(),
            worker_seq,
            store_seq: None,
            ephemeral: false,
            kind: ActivityEventKind::Message {
                role: MessageRole::Tool,
                text: text.to_owned(),
            },
        }
    }

    /// Every drained line is sequenced into the durable transcript keyspace, in
    /// order, under the stream key the event carries.
    #[tokio::test]
    async fn drained_lines_are_sequenced_into_the_durable_transcript() -> TestResult {
        let store = Arc::new(InMemoryObservabilityStore::default());
        let capacity = std::num::NonZeroUsize::new(16).ok_or("capacity must be non-zero")?;
        let publisher = ActivityEventPublisher::new(store, capacity);
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

        let pump = tokio::spawn(publish_declared_transcript(publisher.clone(), receiver));
        sender.send(line_event(0, "first"))?;
        sender.send(line_event(1, "second"))?;
        drop(sender);
        pump.await?;

        let key = ActivityStreamKey::of(&line_event(0, ""));
        let retained = publisher.replay_from(&key, 0).await?;
        let texts = retained
            .iter()
            .filter_map(|record| match &record.event.kind {
                ActivityEventKind::Message { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(texts, vec!["first".to_owned(), "second".to_owned()]);
        Ok(())
    }
}
