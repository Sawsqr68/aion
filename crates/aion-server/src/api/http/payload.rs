//! HTTP body/payload encode-decode shapes and conversions.

use aion_core::DescribeWorkflowResponse;
use aion_proto::{ProtoDescribeWorkflowResponse, WireEnvelope, WireError};
use serde_json::Value;

use super::error::HttpWireError;

pub(crate) const JSON_CONTENT_TYPE: &str = "application/json";

/// Normalise a start/signal `input` into the proto payload the engine takes.
///
/// An ordinary JSON value is wrapped. An explicit envelope is decoded as
/// [`aion_core::Payload`] — **the same shape this API emits** on history and
/// describe — so a payload read off a run can be handed straight back. The
/// envelope previously decoded as a `ProtoPayload`, whose `content_type` is
/// the gRPC wire string; that made the read form (`"Json"`) unusable as input
/// and let this layer accept content types the next one refuses.
pub(super) fn http_input_payload(
    input: Value,
) -> Result<aion_proto::convert::ProtoPayload, WireError> {
    if is_payload_envelope(&input) {
        serde_json::from_value::<aion_core::Payload>(input)
            .map(aion_proto::convert::ProtoPayload::from)
            .map_err(|_error| invalid_start_input())
    } else {
        serde_json::to_vec(&input)
            .map(|bytes| aion_proto::convert::ProtoPayload {
                content_type: JSON_CONTENT_TYPE.to_owned(),
                bytes,
            })
            .map_err(|_error| invalid_start_input())
    }
}

fn is_payload_envelope(input: &Value) -> bool {
    input
        .as_object()
        .is_some_and(|object| object.contains_key("content_type") && object.contains_key("bytes"))
}

fn invalid_start_input() -> WireError {
    WireError::invalid_input(
        "start workflow request must be JSON shaped like \
         {\"namespace\":\"tenant-a\",\"workflow_type\":\"example\",\"input\":{\"name\":\"Ada\"}} \
         or {\"namespace\":\"tenant-a\",\"workflow_type\":\"example\",\"input\":{\"content_type\":\"Json\",\"bytes\":[123,125]}} \
         — the envelope form is the one this API returns for payloads, so a value read off a run can be resubmitted unchanged",
    )
}

/// Convert the proto describe response into the ops console-facing
/// [`DescribeWorkflowResponse`]: the summary is decoded into the generated
/// [`aion_core::WorkflowSummary`] shape and each history envelope is decoded
/// into a plain [`aion_core::Event`], so the wire matches the generated
/// TypeScript bindings field-for-field (no protobuf-derived `{content_type,
/// data}` payload wrappers).
pub(crate) fn describe_response_to_ops_console(
    response: &ProtoDescribeWorkflowResponse,
) -> Result<DescribeWorkflowResponse, HttpWireError> {
    let summary = response
        .summary
        .as_ref()
        .map(decode_summary_envelope)
        .transpose()?;
    let history = response
        .history
        .iter()
        .map(decode_event_envelope)
        .collect::<Result<Vec<_>, _>>()?;
    // The unserved verdict is a LIVE-fleet read, not a decode of the wire
    // response, so it is attached by the handler that holds the server state.
    // `generations` is likewise projected by that handler, from the run's full
    // decoded history rather than from this response's (possibly omitted) one.
    Ok(DescribeWorkflowResponse {
        summary,
        history,
        unserved: Vec::new(),
        generations: Vec::new(),
    })
}

fn decode_summary_envelope(
    envelope: &WireEnvelope,
) -> Result<aion_core::WorkflowSummary, HttpWireError> {
    aion_proto::decode_core_value::<aion_core::WorkflowSummary>(envelope).map_err(HttpWireError)
}

fn decode_event_envelope(envelope: &WireEnvelope) -> Result<aion_core::Event, HttpWireError> {
    aion_proto::decode_event(envelope).map_err(HttpWireError)
}

#[cfg(test)]
mod tests {
    use aion_proto::WireErrorCode;
    use serde_json::json;

    use super::*;

    #[test]
    fn http_start_input_normalization_accepts_plain_json_and_an_envelope()
    -> Result<(), Box<dyn std::error::Error>> {
        let plain = http_input_payload(json!({ "name": "Ada" }))?;
        assert_eq!(plain.content_type, JSON_CONTENT_TYPE);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&plain.bytes)?,
            json!({ "name": "Ada" })
        );

        let malformed =
            http_input_payload(json!({ "content_type": "Json", "bytes": "not-a-byte-array" }));
        assert!(matches!(malformed, Err(error) if error.code == WireErrorCode::InvalidInput));

        Ok(())
    }

    /// The whole point of the envelope form: a payload READ off a run must be
    /// resubmittable unchanged.
    ///
    /// The oracle is the emitted form itself, not a literal I chose — the
    /// envelope is serialised from an `aion_core::Payload` exactly as history
    /// and describe serialise it, so this cannot pass by my writing the same
    /// spelling on both sides of the comparison.
    #[test]
    fn a_payload_serialised_the_way_this_api_emits_it_is_accepted_as_input()
    -> Result<(), Box<dyn std::error::Error>> {
        let emitted = aion_core::Payload::from_json(&json!({ "name": "Ada" }))?;
        let as_this_api_returns_it = serde_json::to_value(&emitted)?;

        // Guard the guard: if the emitted spelling ever stops being `Json`,
        // this test is no longer exercising the read form and must be
        // revisited rather than quietly passing on something else.
        assert_eq!(
            as_this_api_returns_it.get("content_type"),
            Some(&json!("Json")),
            "the emitted content type moved; this round-trip no longer tests the read form"
        );

        let resubmitted = http_input_payload(as_this_api_returns_it)?;
        assert_eq!(resubmitted.content_type, JSON_CONTENT_TYPE);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&resubmitted.bytes)?,
            json!({ "name": "Ada" })
        );
        Ok(())
    }

    /// The gRPC wire spelling is NOT the HTTP envelope form. Accepting it here
    /// is what let this layer take content types the next layer refuses.
    #[test]
    fn the_grpc_wire_spelling_is_refused_as_an_http_envelope() {
        let refused = http_input_payload(json!({
            "content_type": "application/json",
            "bytes": [123, 125],
        }));
        assert!(matches!(refused, Err(error) if error.code == WireErrorCode::InvalidInput));
    }
}
