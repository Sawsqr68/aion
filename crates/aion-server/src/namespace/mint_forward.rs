//! The production [`MintForwarder`]: an already-authorized namespace mint
//! shipped to the registry shard's owner over the R-3 gRPC forwarder.
//!
//! Reuses the forwarder the routing edge already dials signals/queries/cancels
//! and steered starts through, so a forwarded mint gets the SAME caller-metadata
//! copy and the SAME hop stamp (loop prevention) as every other forwarded
//! request — no second transport, no second set of rules.
//!
//! Dormant without a `[store.cluster]` section: with no cluster there is
//! nothing to forward to.

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;

use aion_proto::generated;
use aion_store::NamespaceOrigin;

use super::route::{ForwardMintError, MintCredentials, MintForwarder, encode_mint_origin};
use crate::routing::{ForwardReply, ForwardRequest, RequestForwarder, owner_refusal};

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
/// question about the payload, answered structurally — by
/// [`crate::routing::owner_refusal`], the one place every forwarding surface
/// asks it.
fn classify(status: &tonic::Status) -> ForwardMintError {
    owner_refusal(status).map_or_else(
        || ForwardMintError::Unreachable(status.code().to_string()),
        ForwardMintError::Refused,
    )
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
