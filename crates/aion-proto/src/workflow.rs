//! Workflow-management serde/prost wire types.

use crate::convert::{ProtoPayload, ProtoRunId, ProtoWorkflowId, WireEnvelope};
use crate::error::ProtoWireError;

/// Proto representation of `StartWorkflowRequest`.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoStartWorkflowRequest {
    /// Namespace that scopes the operation.
    #[prost(string, tag = "1")]
    pub namespace: String,
    /// Workflow type name registered with the engine.
    #[prost(string, tag = "2")]
    pub workflow_type: String,
    /// Workflow start input payload.
    #[prost(message, optional, tag = "3")]
    pub input: Option<ProtoPayload>,
    /// R-4 steered-start routing key. When set, the start is steered to
    /// `shard_for(routing_key)`'s owner (forwarded there when this node is not the
    /// owner). `None`/empty keeps the unsteered R-1 remint behaviour.
    #[prost(string, optional, tag = "4")]
    pub routing_key: Option<String>,
    /// Optional task queue this workflow defaults its activities to (the
    /// namespace × `task_queue` targeting story). When set, the server records it
    /// durably on the start so it survives replay/failover. `None`/empty keeps
    /// the namespace's default queue.
    #[prost(string, optional, tag = "5")]
    pub task_queue: Option<String>,
    /// Optional operator-facing display name for the workflow this start
    /// creates (#211). A LABEL over the UUID identity, never an address:
    /// nothing resolves a workflow by name. When set, the server records it
    /// durably (as the `aion.display_name` search attribute) in the same atomic
    /// append as the start. `None` starts it unnamed; a present but blank
    /// (empty or whitespace-only) value is REFUSED with `invalid_input` rather
    /// than reinterpreted as unnamed, so pass `None` to mean "no name" — never
    /// `Some("")`. Unlike `routing_key` and `task_queue`, empty here is not
    /// "not selected".
    ///
    /// The recorded attribute carries no run id, so it reads back per WORKFLOW
    /// (folded over the whole history, last write wins) rather than per run.
    #[prost(string, optional, tag = "6")]
    pub display_name: Option<String>,
}

/// Proto representation of `StartWorkflowResponse`.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoStartWorkflowResponse {
    /// Assigned workflow identifier.
    #[prost(message, optional, tag = "1")]
    pub workflow_id: Option<ProtoWorkflowId>,
    /// Assigned concrete run identifier.
    #[prost(message, optional, tag = "2")]
    pub run_id: Option<ProtoRunId>,
}

/// Proto representation of `SignalRequest`.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoSignalRequest {
    /// Namespace that scopes the operation.
    #[prost(string, tag = "1")]
    pub namespace: String,
    /// Target workflow identifier.
    #[prost(message, optional, tag = "2")]
    pub workflow_id: Option<ProtoWorkflowId>,
    /// Target run identifier.
    #[prost(message, optional, tag = "3")]
    pub run_id: Option<ProtoRunId>,
    /// Signal name registered by workflow code.
    #[prost(string, tag = "4")]
    pub signal_name: String,
    /// Signal payload.
    #[prost(message, optional, tag = "5")]
    pub payload: Option<ProtoPayload>,
}

/// Proto representation of `SignalResponse`.
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoSignalResponse {}

/// Proto representation of `QueryRequest`.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoQueryRequest {
    /// Namespace that scopes the operation.
    #[prost(string, tag = "1")]
    pub namespace: String,
    /// Target workflow identifier.
    #[prost(message, optional, tag = "2")]
    pub workflow_id: Option<ProtoWorkflowId>,
    /// Target run identifier.
    #[prost(message, optional, tag = "3")]
    pub run_id: Option<ProtoRunId>,
    /// Query name registered by workflow code.
    #[prost(string, tag = "4")]
    pub query_name: String,
    /// Caller-supplied arguments handed to the workflow's query handler.
    ///
    /// Absent means "no arguments": the server materializes the JSON `null`
    /// document so a handler always receives one well-formed input. Arguments
    /// are read-only handler inputs and are never recorded in history.
    #[prost(message, optional, tag = "5")]
    pub arguments: Option<ProtoPayload>,
}

/// Proto representation of `QueryResponse`.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoQueryResponse {
    /// Query result or typed wire error.
    #[prost(oneof = "proto_query_response::Outcome", tags = "1, 2")]
    pub outcome: Option<proto_query_response::Outcome>,
}

/// Types nested under [`ProtoQueryResponse`].
pub mod proto_query_response {
    /// Proto oneof for successful query payloads and typed failures.
    #[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Oneof)]
    pub enum Outcome {
        /// Query result payload.
        #[prost(message, tag = "1")]
        Result(super::ProtoPayload),
        /// Typed query error.
        #[prost(message, tag = "2")]
        Error(super::ProtoWireError),
    }
}

/// Proto representation of `CancelRequest`.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoCancelRequest {
    /// Namespace that scopes the operation.
    #[prost(string, tag = "1")]
    pub namespace: String,
    /// Target workflow identifier.
    #[prost(message, optional, tag = "2")]
    pub workflow_id: Option<ProtoWorkflowId>,
    /// Target run identifier.
    #[prost(message, optional, tag = "3")]
    pub run_id: Option<ProtoRunId>,
    /// Human-readable cancellation reason.
    #[prost(string, tag = "4")]
    pub reason: String,
}

/// Proto representation of `CancelResponse`.
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoCancelResponse {}

/// Proto representation of `ReopenRequest`.
///
/// Mirrors [`ProtoCancelRequest`] without a `reason`: the reopen carries only a
/// target. An absent `run_id` means the latest run.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoReopenRequest {
    /// Namespace that scopes the operation.
    #[prost(string, tag = "1")]
    pub namespace: String,
    /// Target workflow identifier.
    #[prost(message, optional, tag = "2")]
    pub workflow_id: Option<ProtoWorkflowId>,
    /// Target run identifier (absent means the latest run).
    #[prost(message, optional, tag = "3")]
    pub run_id: Option<ProtoRunId>,
}

/// Proto representation of `ReopenResponse`.
///
/// Unlike [`ProtoCancelResponse`] (an empty ack) this returns the reopened run
/// id and its projected status (Running) so the caller learns the run is live
/// again.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoReopenResponse {
    /// The reopened concrete run identifier.
    #[prost(message, optional, tag = "1")]
    pub run_id: Option<ProtoRunId>,
    /// The projected workflow status after the reopen (Running).
    #[prost(enumeration = "crate::convert::ProtoWorkflowStatus", tag = "2")]
    pub status: i32,
}

/// Proto representation of `PauseRequest` (#204).
///
/// Mirrors [`ProtoCancelRequest`]: a target plus an optional reason. An absent
/// `run_id` means the latest run.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoPauseRequest {
    /// Namespace that scopes the operation.
    #[prost(string, tag = "1")]
    pub namespace: String,
    /// Target workflow identifier.
    #[prost(message, optional, tag = "2")]
    pub workflow_id: Option<ProtoWorkflowId>,
    /// Target run identifier (absent means the latest run).
    #[prost(message, optional, tag = "3")]
    pub run_id: Option<ProtoRunId>,
    /// Optional operator-supplied pause reason.
    #[prost(string, tag = "4")]
    pub reason: String,
}

/// Proto representation of `PauseResponse` (#204): the paused run and its status.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoPauseResponse {
    /// The paused concrete run identifier.
    #[prost(message, optional, tag = "1")]
    pub run_id: Option<ProtoRunId>,
    /// The projected workflow status after the pause (Paused).
    #[prost(enumeration = "crate::convert::ProtoWorkflowStatus", tag = "2")]
    pub status: i32,
}

/// Proto representation of `ResumeRequest` (#204).
///
/// Mirrors [`ProtoReopenRequest`]: only a target. An absent `run_id` means the
/// latest run.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoResumeRequest {
    /// Namespace that scopes the operation.
    #[prost(string, tag = "1")]
    pub namespace: String,
    /// Target workflow identifier.
    #[prost(message, optional, tag = "2")]
    pub workflow_id: Option<ProtoWorkflowId>,
    /// Target run identifier (absent means the latest run).
    #[prost(message, optional, tag = "3")]
    pub run_id: Option<ProtoRunId>,
}

/// Proto representation of `ResumeResponse` (#204): the resumed run and status.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoResumeResponse {
    /// The resumed concrete run identifier.
    #[prost(message, optional, tag = "1")]
    pub run_id: Option<ProtoRunId>,
    /// The projected workflow status after the resume (Running).
    #[prost(enumeration = "crate::convert::ProtoWorkflowStatus", tag = "2")]
    pub status: i32,
}

/// Proto representation of `RenameRequest` (#211).
///
/// Mirrors [`ProtoPauseRequest`]'s shape: a target plus the operator's payload
/// (here the new display name). An absent `run_id` means the latest run. The
/// name is a LABEL over the UUID identity, never an address — this request
/// SETS a name on an id-addressed run; nothing resolves a workflow by name.
///
/// Both facts hold at once, and they are easy to confuse: you ADDRESS a run
/// (workflow id plus run id, so the server can refuse a rename it cannot append
/// safely), but what gets recorded is a WORKFLOW-level attribute with no run id
/// in it. The name therefore reads back for every run of that workflow, which
/// is exactly why the engine refuses to rename a continue-as-new predecessor:
/// its name would land on the successor that now owns the history head.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoRenameRequest {
    /// Namespace that scopes the operation.
    #[prost(string, tag = "1")]
    pub namespace: String,
    /// Target workflow identifier.
    #[prost(message, optional, tag = "2")]
    pub workflow_id: Option<ProtoWorkflowId>,
    /// Target run identifier (absent means the latest run).
    #[prost(message, optional, tag = "3")]
    pub run_id: Option<ProtoRunId>,
    /// The new display name. Trimmed by the server; must be non-empty after
    /// trimming.
    #[prost(string, tag = "4")]
    pub display_name: String,
}

/// Proto representation of `RenameResponse` (#211): the renamed run and the
/// display name as recorded (trimmed).
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoRenameResponse {
    /// The renamed concrete run identifier.
    #[prost(message, optional, tag = "1")]
    pub run_id: Option<ProtoRunId>,
    /// The display name as recorded (trimmed).
    #[prost(string, tag = "2")]
    pub display_name: String,
}

/// Proto representation of `ListWorkflowsRequest`.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoListWorkflowsRequest {
    /// Namespace that scopes the operation.
    #[prost(string, tag = "1")]
    pub namespace: String,
    /// Serde-encoded `aion_store::visibility::ListWorkflowsFilter` envelope.
    #[prost(message, optional, tag = "2")]
    pub filter: Option<WireEnvelope>,
}

/// Proto representation of `ListWorkflowsResponse`.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoListWorkflowsResponse {
    /// Serde-encoded `aion_store::visibility::WorkflowSummary` envelopes.
    #[prost(message, repeated, tag = "1")]
    pub summaries: Vec<WireEnvelope>,
}

/// Proto representation of `CountWorkflowsRequest`.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoCountWorkflowsRequest {
    /// Namespace that scopes the operation.
    #[prost(string, tag = "1")]
    pub namespace: String,
    /// Serde-encoded `aion_store::visibility::ListWorkflowsFilter` envelope.
    #[prost(message, optional, tag = "2")]
    pub filter: Option<WireEnvelope>,
}

/// Proto representation of `CountWorkflowsResponse`.
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoCountWorkflowsResponse {
    /// Number of visibility summaries matching the filter.
    #[prost(uint64, tag = "1")]
    pub count: u64,
}

/// Proto representation of `DescribeWorkflowRequest`.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, prost::Message)]
pub struct ProtoDescribeWorkflowRequest {
    /// Namespace that scopes the operation.
    #[prost(string, tag = "1")]
    pub namespace: String,
    /// Target workflow identifier.
    #[prost(message, optional, tag = "2")]
    pub workflow_id: Option<ProtoWorkflowId>,
    /// Target run identifier.
    #[prost(message, optional, tag = "3")]
    pub run_id: Option<ProtoRunId>,
    /// Whether event history should be included in the response.
    #[prost(bool, tag = "4")]
    pub include_history: bool,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use aion_core::SearchAttributeValue;
    use aion_store::visibility::{ListWorkflowsFilter, SearchAttributePredicate};
    use chrono::{DateTime, Utc};
    use prost::Message;
    use serde::de::DeserializeOwned;
    use serde_json::json;

    use super::{
        ProtoCountWorkflowsRequest, ProtoCountWorkflowsResponse, ProtoListWorkflowsRequest,
        ProtoListWorkflowsResponse, ProtoQueryRequest, ProtoQueryResponse, ProtoReopenRequest,
        ProtoReopenResponse, ProtoStartWorkflowRequest, ProtoStartWorkflowResponse,
        proto_query_response,
    };
    use crate::convert::{
        ProtoPayload, ProtoRunId, ProtoWorkflowId, decode_core_value, encode_core_value,
    };
    use crate::error::{ProtoWireError, WireError};

    fn workflow_id() -> aion_core::WorkflowId {
        aion_core::WorkflowId::new(uuid::Uuid::nil())
    }

    fn run_id() -> aion_core::RunId {
        aion_core::RunId::new(uuid::Uuid::nil())
    }

    fn payload(label: &str) -> Result<ProtoPayload, aion_core::PayloadError> {
        Ok(ProtoPayload::from(aion_core::Payload::from_json(
            &json!({ "label": label }),
        )?))
    }

    fn recorded_at() -> Result<DateTime<Utc>, chrono::ParseError> {
        Ok(DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")?.with_timezone(&Utc))
    }

    fn assert_json_round_trip<T>(value: &T) -> Result<(), serde_json::Error>
    where
        T: Clone + PartialEq + serde::Serialize + DeserializeOwned,
    {
        let encoded = serde_json::to_string(value)?;
        let decoded = serde_json::from_str::<T>(&encoded)?;
        assert!(decoded == *value);
        Ok(())
    }

    fn assert_proto_round_trip<T>(value: &T) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Clone + PartialEq + Message + Default,
    {
        let mut bytes = Vec::new();
        value.encode(&mut bytes)?;
        let decoded = T::decode(bytes.as_slice())?;
        assert!(decoded == *value);
        Ok(())
    }

    #[test]
    fn start_workflow_round_trips_json_and_proto() -> Result<(), Box<dyn std::error::Error>> {
        let request = ProtoStartWorkflowRequest {
            namespace: String::from("tenant-a"),
            workflow_type: String::from("checkout"),
            input: Some(payload("input")?),
            routing_key: Some(String::from("tenant-a/order-1")),
            task_queue: Some(String::from("gpu")),
            display_name: Some(String::from("Order 1 checkout")),
        };
        let response = ProtoStartWorkflowResponse {
            workflow_id: Some(ProtoWorkflowId::from(workflow_id())),
            run_id: Some(ProtoRunId::from(run_id())),
        };

        assert_json_round_trip(&request)?;
        assert_proto_round_trip(&request)?;
        assert_json_round_trip(&response)?;
        assert_proto_round_trip(&response)?;
        Ok(())
    }

    #[test]
    fn list_workflows_round_trips_json_and_proto() -> Result<(), Box<dyn std::error::Error>> {
        let filter = ListWorkflowsFilter {
            workflow_type: Some(String::from("checkout")),
            status: Some(aion_core::WorkflowStatus::Running),
            search_attributes: vec![SearchAttributePredicate::Equals {
                name: String::from("customer_id"),
                value: SearchAttributeValue::String(String::from("12345")),
            }],
            limit: Some(10),
            offset: Some(5),
            ..ListWorkflowsFilter::default()
        };
        let summary = aion_store::visibility::WorkflowSummary {
            workflow_id: workflow_id(),
            run_id: run_id(),
            workflow_type: String::from("checkout"),
            status: aion_core::WorkflowStatus::Running,
            start_time: recorded_at()?,
            close_time: None,
            failed_step: None,
            failure_reason: None,
            search_attributes: HashMap::from([(
                String::from("customer_id"),
                SearchAttributeValue::String(String::from("12345")),
            )]),
        };
        let filter_envelope = encode_core_value("tenant-a", Some(String::from("r1")), &filter)?;
        let summary_envelope = encode_core_value("tenant-a", None, &summary)?;
        let request = ProtoListWorkflowsRequest {
            namespace: String::from("tenant-a"),
            filter: Some(filter_envelope.clone()),
        };
        let response = ProtoListWorkflowsResponse {
            summaries: vec![summary_envelope.clone()],
        };
        let count_request = ProtoCountWorkflowsRequest {
            namespace: String::from("tenant-a"),
            filter: Some(filter_envelope.clone()),
        };
        let count_response = ProtoCountWorkflowsResponse { count: 1 };

        assert_json_round_trip(&request)?;
        assert_proto_round_trip(&request)?;
        assert_json_round_trip(&response)?;
        assert_proto_round_trip(&response)?;
        assert_json_round_trip(&count_request)?;
        assert_proto_round_trip(&count_request)?;
        assert_json_round_trip(&count_response)?;
        assert_proto_round_trip(&count_response)?;
        assert_eq!(
            decode_core_value::<ListWorkflowsFilter>(&filter_envelope)?,
            filter
        );
        assert_eq!(
            decode_core_value::<aion_store::visibility::WorkflowSummary>(&summary_envelope)?,
            summary
        );
        Ok(())
    }

    #[test]
    fn query_round_trips_json_and_proto() -> Result<(), Box<dyn std::error::Error>> {
        let request = ProtoQueryRequest {
            namespace: String::from("tenant-a"),
            workflow_id: Some(ProtoWorkflowId::from(workflow_id())),
            run_id: Some(ProtoRunId::from(run_id())),
            query_name: String::from("state"),
            arguments: Some(payload("arguments")?),
        };
        // A caller that supplies no arguments is a distinct encoded shape, not
        // an error: the field is optional on the wire and the server supplies
        // the canonical `null` document in its place.
        let no_arguments_request = ProtoQueryRequest {
            arguments: None,
            ..request.clone()
        };
        let result_response = ProtoQueryResponse {
            outcome: Some(proto_query_response::Outcome::Result(payload("result")?)),
        };
        let error_response = ProtoQueryResponse {
            outcome: Some(proto_query_response::Outcome::Error(ProtoWireError::from(
                WireError::unknown_query("state query is not registered"),
            ))),
        };

        assert_json_round_trip(&request)?;
        assert_proto_round_trip(&request)?;
        assert_json_round_trip(&no_arguments_request)?;
        assert_proto_round_trip(&no_arguments_request)?;
        assert_json_round_trip(&result_response)?;
        assert_proto_round_trip(&result_response)?;
        assert_json_round_trip(&error_response)?;
        assert_proto_round_trip(&error_response)?;
        // The two shapes stay distinguishable across a proto round trip: a
        // decoder can tell "no arguments supplied" from any supplied document.
        assert_ne!(request, no_arguments_request);
        Ok(())
    }

    /// #211: an unnamed start is a distinct wire shape from a named one, and
    /// both round-trip across JSON and proto without conflating.
    #[test]
    fn start_workflow_display_name_absent_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let named = ProtoStartWorkflowRequest {
            namespace: String::from("tenant-a"),
            workflow_type: String::from("checkout"),
            input: Some(payload("input")?),
            routing_key: None,
            task_queue: None,
            display_name: Some(String::from("Order 1 checkout")),
        };
        let unnamed = ProtoStartWorkflowRequest {
            display_name: None,
            ..named.clone()
        };

        assert_json_round_trip(&named)?;
        assert_proto_round_trip(&named)?;
        assert_json_round_trip(&unnamed)?;
        assert_proto_round_trip(&unnamed)?;
        // The two shapes stay distinguishable across the wire: a decoder can
        // tell "no name supplied" from any supplied name.
        assert_ne!(named, unnamed);
        Ok(())
    }

    /// #211: the HAND-WRITTEN `display_name` and the GENERATED stub's
    /// `display_name` are the same wire field.
    ///
    /// Both sides declare tag 6, but "both say 6" is two claims, not agreement.
    /// This encodes with one and decodes with the other, in both directions, so
    /// a tag or wire-type drift between the `.proto` and the hand-written prost
    /// derive is a failure here rather than a field that silently vanishes at a
    /// real transport boundary. Compiled only under `generated`, which is the
    /// only posture in which the stubs exist at all.
    #[cfg(feature = "generated")]
    #[test]
    fn start_workflow_display_name_is_the_same_wire_field_as_the_generated_stub()
    -> Result<(), Box<dyn std::error::Error>> {
        const NAME: &str = "Nightly settlement";
        let hand_written = ProtoStartWorkflowRequest {
            namespace: String::from("tenant-a"),
            workflow_type: String::from("checkout"),
            input: None,
            routing_key: None,
            task_queue: None,
            display_name: Some(String::from(NAME)),
        };

        // Hand-written -> generated.
        let mut bytes = Vec::new();
        hand_written.encode(&mut bytes)?;
        let decoded = crate::generated::StartWorkflowRequest::decode(bytes.as_slice())?;
        assert_eq!(
            decoded.display_name.as_deref(),
            Some(NAME),
            "the generated stub must read the hand-written display_name"
        );

        // Generated -> hand-written.
        let mut bytes = Vec::new();
        decoded.encode(&mut bytes)?;
        let round_tripped = ProtoStartWorkflowRequest::decode(bytes.as_slice())?;
        assert_eq!(round_tripped, hand_written);

        // And the field key itself is pinned: tag 6, length-delimited, is
        // (6 << 3) | 2 = 0x32. An unnamed start emits it nowhere.
        let mut bytes = Vec::new();
        ProtoStartWorkflowRequest {
            namespace: String::new(),
            workflow_type: String::new(),
            input: None,
            routing_key: None,
            task_queue: None,
            display_name: Some(String::from("x")),
        }
        .encode(&mut bytes)?;
        assert_eq!(bytes, vec![0x32, 0x01, b'x']);

        let mut bytes = Vec::new();
        ProtoStartWorkflowRequest {
            namespace: String::new(),
            workflow_type: String::new(),
            input: None,
            routing_key: None,
            task_queue: None,
            display_name: None,
        }
        .encode(&mut bytes)?;
        assert!(
            bytes.is_empty(),
            "an unnamed start must put nothing on the wire, got {bytes:?}"
        );
        Ok(())
    }

    /// #211: the hand-written Rename messages agree with the generated stubs,
    /// in both directions.
    #[cfg(feature = "generated")]
    #[test]
    fn rename_messages_agree_with_the_generated_stubs() -> Result<(), Box<dyn std::error::Error>> {
        let request = super::ProtoRenameRequest {
            namespace: String::from("tenant-a"),
            workflow_id: Some(ProtoWorkflowId::from(workflow_id())),
            run_id: Some(ProtoRunId::from(run_id())),
            display_name: String::from("Nightly settlement"),
        };
        let mut bytes = Vec::new();
        request.encode(&mut bytes)?;
        let decoded = crate::generated::RenameRequest::decode(bytes.as_slice())?;
        assert_eq!(decoded.display_name, "Nightly settlement");
        assert_eq!(decoded.namespace, "tenant-a");
        let mut bytes = Vec::new();
        decoded.encode(&mut bytes)?;
        assert_eq!(
            super::ProtoRenameRequest::decode(bytes.as_slice())?,
            request
        );

        let response = super::ProtoRenameResponse {
            run_id: Some(ProtoRunId::from(run_id())),
            display_name: String::from("Nightly settlement"),
        };
        let mut bytes = Vec::new();
        response.encode(&mut bytes)?;
        let decoded = crate::generated::RenameResponse::decode(bytes.as_slice())?;
        assert_eq!(decoded.display_name, "Nightly settlement");
        let mut bytes = Vec::new();
        decoded.encode(&mut bytes)?;
        assert_eq!(
            super::ProtoRenameResponse::decode(bytes.as_slice())?,
            response
        );
        Ok(())
    }

    #[test]
    fn rename_round_trips_json_and_proto() -> Result<(), Box<dyn std::error::Error>> {
        let request = super::ProtoRenameRequest {
            namespace: String::from("tenant-a"),
            workflow_id: Some(ProtoWorkflowId::from(workflow_id())),
            run_id: Some(ProtoRunId::from(run_id())),
            display_name: String::from("Nightly settlement"),
        };
        let response = super::ProtoRenameResponse {
            run_id: Some(ProtoRunId::from(run_id())),
            display_name: String::from("Nightly settlement"),
        };

        assert_json_round_trip(&request)?;
        assert_proto_round_trip(&request)?;
        assert_json_round_trip(&response)?;
        assert_proto_round_trip(&response)?;
        Ok(())
    }

    #[test]
    fn reopen_round_trips_json_and_proto() -> Result<(), Box<dyn std::error::Error>> {
        let request = ProtoReopenRequest {
            namespace: String::from("tenant-a"),
            workflow_id: Some(ProtoWorkflowId::from(workflow_id())),
            run_id: Some(ProtoRunId::from(run_id())),
        };
        let response = ProtoReopenResponse {
            run_id: Some(ProtoRunId::from(run_id())),
            status: crate::convert::ProtoWorkflowStatus::Running as i32,
        };

        assert_json_round_trip(&request)?;
        assert_proto_round_trip(&request)?;
        assert_json_round_trip(&response)?;
        assert_proto_round_trip(&response)?;
        Ok(())
    }
}
