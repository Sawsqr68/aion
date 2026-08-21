//! Describe facts and windowed workflow-history wire contracts.

use crate::{ProtoRunId, ProtoWorkflowId, WireEnvelope};

/// Proto representation of `DescribeWorkflowResponse`.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoDescribeWorkflowResponse {
    /// Serde-encoded `aion_core::WorkflowSummary` envelope.
    #[prost(message, optional, tag = "1")]
    pub summary: Option<WireEnvelope>,
    /// Optional serde-encoded `aion_core::Event` envelopes.
    #[prost(message, repeated, tag = "2")]
    pub history: Vec<WireEnvelope>,
    /// Run resolved by the describe read.
    #[prost(message, optional, tag = "3")]
    pub run_id: Option<ProtoRunId>,
    /// Sequence number at the head of the history snapshot.
    #[prost(uint64, tag = "4")]
    pub history_head_seq: u64,
    /// Current lease's terminal workflow event, when present.
    #[prost(message, optional, tag = "5")]
    pub terminal_event: Option<WireEnvelope>,
}

/// Proto representation of `ReadHistoryRequest`.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoReadHistoryRequest {
    /// Namespace that scopes the operation.
    #[prost(string, tag = "1")]
    pub namespace: String,
    /// Target workflow identifier.
    #[prost(message, optional, tag = "2")]
    pub workflow_id: Option<ProtoWorkflowId>,
    /// First sequence number to include; absent starts at the beginning.
    #[prost(uint64, optional, tag = "3")]
    pub from_seq: Option<u64>,
    /// Maximum number of events in this page; absent uses the server default.
    #[prost(uint32, optional, tag = "4")]
    pub limit: Option<u32>,
}

/// Proto representation of `ReadHistoryResponse`.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoReadHistoryResponse {
    /// Serde-encoded `aion_core::Event` envelopes.
    #[prost(message, repeated, tag = "1")]
    pub events: Vec<WireEnvelope>,
    /// Cursor for the next page, absent when this page reaches the head.
    #[prost(uint64, optional, tag = "2")]
    pub next_from_seq: Option<u64>,
    /// Sequence number at the head of the history snapshot.
    #[prost(uint64, tag = "3")]
    pub head_seq: u64,
}
