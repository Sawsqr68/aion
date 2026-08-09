//! Required-field and envelope encode/decode helpers for the shared handlers.

use aion_core::{Payload, WorkflowId};
use aion_proto::{
    WireError,
    convert::{ProtoPayload, decode_core_value, encode_event},
};

pub(super) fn required_workflow_id(
    id: Option<aion_proto::ProtoWorkflowId>,
) -> Result<WorkflowId, WireError> {
    id.ok_or_else(|| WireError::backend("workflow id is missing"))?
        .try_into()
}

pub(super) fn required_payload(payload: Option<ProtoPayload>) -> Result<Payload, WireError> {
    payload
        .ok_or_else(|| WireError::backend("payload is missing"))?
        .try_into()
}

/// Decode an optional wire payload, substituting the canonical "nothing
/// supplied" document when the field is absent.
///
/// Used by surfaces whose payload is genuinely optional — query arguments —
/// so an omitted field becomes one well-formed JSON document rather than an
/// empty byte string no decoder can read.
pub(super) fn optional_payload(payload: Option<ProtoPayload>) -> Result<Payload, WireError> {
    payload.map_or_else(|| Ok(Payload::json_null()), TryInto::try_into)
}

pub(super) fn decode_visibility_filter(
    filter: Option<&aion_proto::WireEnvelope>,
) -> Result<aion_store::visibility::ListWorkflowsFilter, WireError> {
    filter.map_or_else(
        || Ok(aion_store::visibility::ListWorkflowsFilter::default()),
        decode_core_value,
    )
}

pub(super) fn encode_history(
    include_history: bool,
    namespace: &str,
    history: &[aion_core::Event],
) -> Result<Vec<aion_proto::WireEnvelope>, WireError> {
    if include_history {
        history
            .iter()
            .map(|event| encode_event(namespace.to_owned(), None, event))
            .collect()
    } else {
        Ok(Vec::new())
    }
}
