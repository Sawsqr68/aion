//! The production [`MintForwarder`]: an already-authorized namespace mint
//! shipped to the registry shard's owner over the R-3 gRPC forwarder.
//!
//! Reuses the forwarder the routing edge already dials signals/queries/cancels
//! and steered starts through, so a forwarded mint gets the SAME caller-metadata
//! copy and the SAME hop stamp (loop prevention) as every other forwarded
//! request — no second transport, no second set of rules.
//!
//! Compiled only under `feature = "haematite-backend"`: without the distributed
//! backend there is no cluster to forward to.

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use prost::Message as _;

use aion_proto::generated;
use aion_proto::{ProtoWireError, WireError};
use aion_store::NamespaceOrigin;

use super::route::{ForwardMintError, MintCredentials, MintForwarder, encode_mint_origin};
use crate::routing::{ForwardReply, ForwardRequest, RequestForwarder};

/// Ships mints over the R-3 gRPC request forwarder.
#[derive(Clone)]
pub struct GrpcMintForwarder {
    forwarder: Arc<dyn RequestForwarder>,
}

impl GrpcMintForwarder {
    /// Wrap the boot's shared request forwarder.
    #[must_use]
    pub const fn new(forwarder: Arc<dyn RequestForwarder>) -> Self {
        Self { forwarder }
    }
}

/// Classify a forward failure WITHOUT reading message text.
///
/// The server encodes its typed `WireError` into the status details on every
/// refusal it authors; a transport failure the forwarder itself synthesises (a
/// failed dial) carries no details. So "did the owner answer?" is a structural
/// question about the payload, answered structurally.
fn classify(status: &tonic::Status) -> ForwardMintError {
    // proto3 decodes an EMPTY buffer into an all-default message, so "details
    // present" must be checked before decoding or a bare transport status would
    // masquerade as an authored refusal.
    if status.details().is_empty() {
        return ForwardMintError::Unreachable(status.code().to_string());
    }
    let Ok(proto) = ProtoWireError::decode(status.details()) else {
        return ForwardMintError::Unreachable(status.code().to_string());
    };
    match WireError::try_from(proto) {
        Ok(wire) | Err(wire) => ForwardMintError::Refused(wire),
    }
}

#[async_trait]
impl MintForwarder for GrpcMintForwarder {
    async fn forward_mint(
        &self,
        target: SocketAddr,
        credentials: &MintCredentials,
        namespaces: &[String],
        origin: NamespaceOrigin,
    ) -> Result<(), ForwardMintError> {
        let request = generated::MintNamespaceRequest {
            namespaces: namespaces.to_vec(),
            origin: encode_mint_origin(origin),
        };
        match self
            .forwarder
            .forward(
                target,
                credentials.to_grpc_metadata(),
                ForwardRequest::MintNamespace(request),
            )
            .await
        {
            Ok(ForwardReply::MintNamespace(_)) => Ok(()),
            Ok(_) => Err(ForwardMintError::Unreachable(
                "the forwarder returned a reply for a different request".to_owned(),
            )),
            Err(status) => Err(classify(&status)),
        }
    }
}
