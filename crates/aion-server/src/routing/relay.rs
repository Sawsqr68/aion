//! Relaying a FORWARDED request's answer back to the original caller.
//!
//! Forwarding exists so a caller that reached a non-owner still gets the
//! OWNER'S answer (DISTRIBUTED-ROUTING-DESIGN §2.2). That promise is only kept
//! if the owner's refusals travel too: the owner encodes its typed
//! [`WireError`] into the status details on every refusal it authors
//! (`api/grpc/status.rs`), so `not_found`, `invalid_input`, `namespace_denied`
//! and `invalid_state` are all present on the wire. Flattening them into
//! `NotOwner` tells the caller to retry a request that will never succeed, and
//! loses the cause entirely.
//!
//! So the edge asks ONE structural question — "did the owner author this?" —
//! and this module is where it is asked, once, for every forwarding surface.

use aion_proto::{ProtoWireError, WireError};
use prost::Message as _;
use tonic::Status;

/// The owner's own typed refusal, decoded out of a forwarded request's failure
/// status, or [`None`] when the status carries no authored detail.
///
/// [`None`] means the forward itself failed — a dial that never reached the
/// owner, or a status synthesised by the transport — because a server that
/// authored a refusal always attaches the encoded [`ProtoWireError`] detail.
/// The question is answered STRUCTURALLY, from the payload, never by reading
/// message text.
///
/// A detail that decodes but carries no recognised code is still the owner's
/// answer, not a transport failure: [`WireError::try_from`] renders that as a
/// `backend` error and it is relayed as such, exactly as the namespace mint
/// forwarder has always treated it.
#[must_use]
pub fn owner_refusal(status: &Status) -> Option<WireError> {
    // proto3 decodes an EMPTY buffer into an all-default message, so "details
    // present" must be checked BEFORE decoding or a bare transport status would
    // masquerade as an authored refusal.
    if status.details().is_empty() {
        return None;
    }
    let proto = ProtoWireError::decode(status.details()).ok()?;
    match WireError::try_from(proto) {
        Ok(wire) | Err(wire) => Some(wire),
    }
}

/// The typed retryable wrong-owner refusal for `shard`.
///
/// One builder for every surface, so a console retrying a mutation sees one
/// answer whichever transport it arrived on and the two edges cannot drift in
/// code, error type, or wording.
#[must_use]
pub fn not_owner_wire(shard: usize) -> WireError {
    WireError::not_owner(format!(
        "workflow shard {shard} is owned by another cluster node"
    ))
    .with_error_type("NotOwner")
}

#[cfg(test)]
mod tests {
    use aion_proto::{WireError, WireErrorCode};
    use tonic::{Code, Status};

    use super::{not_owner_wire, owner_refusal};

    /// The status is built by the PRODUCTION encoder the owner uses, so this
    /// pins the encode/decode pair rather than a hand-rolled fixture that could
    /// agree with itself while disagreeing with the server.
    fn owner_status(error: WireError) -> Status {
        crate::api::grpc::status_from_wire_error(error)
    }

    /// Every refusal the owner authors survives the hop with its code, message,
    /// and error type intact — including the ones a rename exists to produce.
    #[test]
    fn an_authored_refusal_decodes_back_to_the_owners_own_error() {
        let cases = [
            WireError::not_found("workflow 7 has no recorded history"),
            WireError::invalid_input("display_name must not be blank"),
            WireError::namespace_denied("tenant-b is not visible to this caller"),
            WireError::invalid_state("run is not resident on this node")
                .with_error_type("Resident"),
        ];
        for expected in cases {
            let decoded = owner_refusal(&owner_status(expected.clone()))
                .unwrap_or_else(|| WireError::backend("no detail decoded"));
            assert_eq!(decoded.code, expected.code, "the owner's code must survive");
            assert_eq!(decoded.message, expected.message);
            assert_eq!(decoded.error_type, expected.error_type);
        }
    }

    /// A transport failure carries no authored detail, so it is NOT mistaken
    /// for an answer — this is the only case the edges may answer `NotOwner`
    /// for.
    #[test]
    fn a_transport_status_carries_no_owner_answer() {
        assert!(owner_refusal(&Status::unavailable("forward dial failed")).is_none());
        assert!(owner_refusal(&Status::internal("connection reset")).is_none());
    }

    /// Details that are not a `ProtoWireError` at all are treated as no answer
    /// rather than decoded into a fabricated one.
    #[test]
    fn undecodable_details_are_not_an_owner_answer() {
        // Field 1 declared as a length-delimited value that runs off the end of
        // the buffer: not a decodable `ProtoWireError`.
        let details: Vec<u8> = vec![0x0a, 0x7f];
        let status = Status::with_details(Code::Aborted, "opaque", details.into());
        assert!(owner_refusal(&status).is_none());
    }

    /// The shared wrong-owner refusal is the typed retryable one, and names the
    /// shard the caller must re-resolve.
    #[test]
    fn the_shared_not_owner_refusal_is_typed_and_names_the_shard() {
        let wire = not_owner_wire(7);
        assert_eq!(wire.code, WireErrorCode::NotOwner);
        assert_eq!(wire.error_type.as_deref(), Some("NotOwner"));
        assert!(wire.message.contains("shard 7"), "{}", wire.message);
    }
}
