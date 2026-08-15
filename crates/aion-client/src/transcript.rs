//! Typed client subscription for one activity-attempt transcript stream.
//!
//! This is the sole caller-side implementation of the transcript WebSocket
//! contract. Consumers receive canonical activity events, plus an explicit lag
//! item that tells them to reattach from their last durable `store_seq`.

use std::pin::Pin;

use aion_core::{ActivityEvent, ActivityId, RunId, WorkflowId};
use aion_proto::{ProtoActivityId, ProtoRunId, ProtoWorkflowId, TranscriptSubscription};
use futures::Stream;

use crate::{Client, ClientError};

/// Boxed stream returned by [`Client::subscribe_transcript`].
pub type TranscriptStream =
    Pin<Box<dyn Stream<Item = Result<TranscriptStreamItem, ClientError>> + Send>>;

/// One decoded server frame on a transcript subscription.
#[derive(Clone, Debug, PartialEq)]
pub enum TranscriptStreamItem {
    /// One canonical activity transcript event.
    Event(Box<ActivityEvent>),
    /// The server's per-subscription broadcast receiver lagged. The caller can
    /// recover this leg by reattaching with its last applied durable sequence.
    Lagged {
        /// Number of live broadcast records skipped before the socket closed.
        skipped: u64,
    },
}

/// Full identity and optional durable resume cursor for one transcript stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranscriptTarget {
    /// Workflow containing the activity.
    pub workflow_id: WorkflowId,
    /// Concrete workflow generation.
    pub run_id: RunId,
    /// Activity within the run.
    pub activity_id: ActivityId,
    /// Attempt within the activity.
    pub attempt: u32,
    /// Highest durable `store_seq` already applied. `None` requests the full
    /// retained splice; ephemeral events do not advance this cursor.
    pub after_seq: Option<u64>,
}

impl TranscriptTarget {
    fn subscription(self, namespace: &str) -> TranscriptSubscription {
        TranscriptSubscription {
            namespace: namespace.to_owned(),
            workflow_id: Some(ProtoWorkflowId::from(self.workflow_id)),
            run_id: Some(ProtoRunId::from(self.run_id)),
            activity_id: Some(ProtoActivityId::from(self.activity_id)),
            attempt: self.attempt,
            after_seq: self.after_seq,
        }
    }
}

impl Client {
    /// Opens one typed transcript WebSocket subscription.
    ///
    /// Event frames and lag frames are decoded strictly. An unknown kind,
    /// unknown field, malformed body, terminal wire error, or abnormal socket
    /// close is returned as a named [`ClientError`]; no frame is discarded.
    /// A [`TranscriptStreamItem::Lagged`] item is recoverable per leg: callers
    /// should announce it and open a fresh subscription with their last applied
    /// `store_seq` as [`TranscriptTarget::after_seq`].
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::InvalidArgument`] for a missing/invalid stream
    /// endpoint, [`ClientError::Unauthenticated`] for a rejected upgrade, or
    /// [`ClientError::Unavailable`] when the socket cannot be established.
    pub async fn subscribe_transcript(
        &self,
        target: TranscriptTarget,
    ) -> Result<TranscriptStream, ClientError> {
        crate::transport::transcript_ws::open(&self.config, target.subscription(self.namespace()))
            .await
    }
}
