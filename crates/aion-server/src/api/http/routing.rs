//! Shard routing for the HTTP write surface (#211 rename).
//!
//! A rename APPENDS a recorded `SearchAttributesUpdated` event, so it is a
//! write and must reach the workflow shard's OWNER — serving it on a non-owner
//! would append from a node that does not hold the single writer, which is why
//! the gRPC edge forwards it (`crate::routing::ForwardRequest::Rename`).
//!
//! HTTP needed the same treatment because HTTP is the surface the OPS CONSOLE
//! speaks. Rename carries an engine-side residency gate — a non-terminal,
//! non-`Paused` run with no registered handle on this node is refused rather
//! than raced — so a console pointed at a non-owner node did not merely write
//! to the wrong place: it got `registry.get -> None`, status `Running`, and a
//! refusal telling it to "retry once it is resident", a retry that could never
//! succeed from that node. Forwarding turns that dead end into the owner's own
//! answer — its REFUSALS included, relayed verbatim rather than flattened into
//! a wrong-owner retry hint the console would spin on forever.
//!
//! Compiled only under `feature = "haematite-backend"`: without the distributed
//! backend there are no shards, no directory, and no owner but this node.

use std::net::SocketAddr;

use axum::http::HeaderMap;
use tonic::metadata::MetadataMap;

use aion_proto::{ProtoRenameRequest, ProtoRenameResponse, WireError};

use crate::ServerState;
use crate::api::grpc::convert::{decode_rename_response, encode_rename_request};
use crate::routing::{
    ForwardReply, ForwardRequest, RequestForwarder, RouteDecision, ShardDirectory, not_owner_wire,
    owner_refusal, route_mutation,
};

use super::auth::caller_credentials_metadata;

/// Decide where an HTTP rename must execute, forwarding it when this node is
/// not the workflow shard's owner.
///
/// `Ok(None)` means serve it locally — the only outcome for single-node and
/// non-clustered boots, so the default path is unchanged. `Ok(Some(response))`
/// is the owner's own reply, relayed verbatim. `Err` is the owner's own typed
/// REFUSAL when it authored one, the typed retryable `NotOwner` when no owner
/// could be reached at all, or a backend error when the forwarder answers a
/// different request than it was asked.
///
/// A missing or malformed workflow id routes LOCALLY on purpose: routing acts
/// only on a well-formed target, and the handler's own validation is what owes
/// the caller an answer about a malformed one — exactly the rule the gRPC edge
/// applies (`api/grpc/routing_resolve.rs`).
///
/// # Errors
///
/// The owner's own typed [`WireError`] when the owner refused the rename (a
/// blank name, an unknown workflow, a denied namespace, a superseded or
/// non-resident run — every refusal this endpoint exists to produce);
/// [`WireError`] with code `NotOwner` when the owning node cannot be forwarded
/// to at all (no directory address, no forwarder wired, or the dial failed);
/// and a backend error when the forwarder returns a reply for a different
/// request.
pub(crate) async fn forward_rename(
    state: &ServerState,
    headers: &HeaderMap,
    request: &ProtoRenameRequest,
) -> Result<Option<ProtoRenameResponse>, WireError> {
    let Some(cluster_store) = state.cluster_store() else {
        return Ok(None);
    };
    let Some(proto_id) = request.workflow_id.clone() else {
        return Ok(None);
    };
    let Ok(workflow_id) = aion_core::WorkflowId::try_from(proto_id) else {
        return Ok(None);
    };
    let directory = state
        .shard_directory()
        .map(|directory| directory.as_ref() as &dyn ShardDirectory);
    match route_mutation(Some(cluster_store.as_ref()), directory, &workflow_id) {
        RouteDecision::Local => Ok(None),
        RouteDecision::NotOwner { shard } => Err(not_owner_wire(shard)),
        RouteDecision::Forward { owner, shard } => {
            let Some(target) = owner.grpc_addr else {
                return Err(not_owner_wire(shard));
            };
            let Some(forwarder) = state.request_forwarder() else {
                return Err(not_owner_wire(shard));
            };
            // The caller's own credentials ride along, so the owner authorizes
            // the rename exactly as this node did rather than as an anonymous
            // caller. The hop counter is deliberately NOT copied from the
            // inbound headers: an HTTP request is the START of a forward chain
            // by construction (nothing forwards to this server over HTTP), and
            // the forwarder stamps hop 1 on the way out — the owner's own edge
            // then enforces the cap on any further hop.
            let metadata = caller_credentials_metadata(headers)?;
            relay_rename(forwarder.as_ref(), target, metadata, request, shard)
                .await
                .map(Some)
        }
    }
}

/// Ship the rename to `target` and answer the caller with what the OWNER said.
///
/// A refusal the owner authored is relayed VERBATIM — code, message, and error
/// type — because that refusal is the whole reason the request was forwarded:
/// a blank name, an unknown workflow, a denied namespace, a superseded run, a
/// run that is not resident there. Flattening those into `NotOwner` would tell
/// the console to re-resolve and retry a request that can never succeed, and
/// would discard the only explanation anyone has.
///
/// `NotOwner` is answered ONLY when the forward carried no authored answer at
/// all — a stale or unreachable target (it may have just died, or not yet have
/// adopted the shard) — so the caller re-resolves, matching the gRPC edge's
/// §2.5 discipline. That case is logged: a forward that failed in the transport
/// is the one outcome whose cause exists nowhere else.
async fn relay_rename(
    forwarder: &dyn RequestForwarder,
    target: SocketAddr,
    metadata: MetadataMap,
    request: &ProtoRenameRequest,
    shard: usize,
) -> Result<ProtoRenameResponse, WireError> {
    match forwarder
        .forward(
            target,
            metadata,
            ForwardRequest::Rename(encode_rename_request(request.clone())),
        )
        .await
    {
        Ok(ForwardReply::Rename(reply)) => Ok(decode_rename_response(reply)),
        Ok(_) => Err(WireError::backend(
            "the forwarder returned a reply for a different request",
        )),
        Err(status) => Err(owner_refusal(&status).unwrap_or_else(|| {
            tracing::warn!(
                shard,
                %target,
                grpc_code = %status.code(),
                grpc_message = status.message(),
                "forwarded rename never reached the shard owner; answering NotOwner so the \
                 caller re-resolves"
            );
            not_owner_wire(shard)
        })),
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::sync::Arc;

    use aion::EngineBuilder;
    use aion_proto::{ProtoRenameRequest, ProtoWorkflowId, WireError};
    use aion_store::{EventStore, InMemoryStore};
    use axum::http::HeaderMap;
    use tonic::metadata::MetadataMap;

    use super::super::test_support::{NAMESPACE, runtime_config, server_state};
    use super::{ForwardReply, ForwardRequest, RequestForwarder, forward_rename, relay_rename};
    use crate::config::NamespaceMode;
    use crate::namespace::{StaticScheduleNamespaces, StaticWorkflowNamespaces};
    use crate::{NamespaceResolver, ServerState};

    async fn non_clustered_state() -> Result<ServerState, Box<dyn std::error::Error>> {
        let store: Arc<dyn EventStore> = Arc::new(InMemoryStore::default());
        let engine = Arc::new(
            EngineBuilder::new()
                .store_arc(store)
                .in_memory_visibility()
                .scheduler_threads(1)
                .build()
                .await?,
        );
        let resolver = NamespaceResolver::from_parts(
            NamespaceMode::SharedEngine,
            Some(engine),
            Arc::new(StaticWorkflowNamespaces::default()),
            Arc::new(StaticScheduleNamespaces::default()),
        );
        server_state(resolver, runtime_config()).await
    }

    fn rename_request(workflow_id: Option<ProtoWorkflowId>) -> ProtoRenameRequest {
        ProtoRenameRequest {
            namespace: NAMESPACE.to_owned(),
            workflow_id,
            run_id: None,
            display_name: String::from("Nightly settlement"),
        }
    }

    /// A boot with no cluster store routes every rename LOCALLY — the single-node
    /// default path is unchanged by the forward existing at all.
    #[tokio::test]
    async fn a_non_clustered_boot_always_serves_the_rename_locally()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = non_clustered_state().await?;
        let workflow_id = ProtoWorkflowId::from(aion_core::WorkflowId::new_v4());

        let routed = forward_rename(
            &state,
            &HeaderMap::new(),
            &rename_request(Some(workflow_id)),
        )
        .await?;

        assert!(
            routed.is_none(),
            "with no cluster store there is no owner but this node, so nothing may be forwarded"
        );
        state.shutdown()?;
        Ok(())
    }

    /// A malformed or absent target routes locally rather than being refused
    /// here: routing acts only on a well-formed target, and the handler's own
    /// validation is what owes the caller an answer about a bad one.
    #[tokio::test]
    async fn a_target_routing_cannot_read_is_left_to_the_handler()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = non_clustered_state().await?;

        assert!(
            forward_rename(&state, &HeaderMap::new(), &rename_request(None))
                .await?
                .is_none()
        );
        assert!(
            forward_rename(
                &state,
                &HeaderMap::new(),
                &rename_request(Some(ProtoWorkflowId {
                    uuid: String::from("not-a-uuid"),
                })),
            )
            .await?
            .is_none()
        );

        state.shutdown()?;
        Ok(())
    }

    /// A forwarder scripted with one answer, so the relay can be driven without
    /// a live owner: the answer it returns is the one the OWNER would have
    /// produced, encoded by the production `status_from_wire_error` path.
    struct ScriptedForwarder(Result<ForwardReply, tonic::Status>);

    #[async_trait::async_trait]
    impl RequestForwarder for ScriptedForwarder {
        async fn forward(
            &self,
            _target: SocketAddr,
            _metadata: MetadataMap,
            _request: ForwardRequest,
        ) -> Result<ForwardReply, tonic::Status> {
            match &self.0 {
                Ok(reply) => Ok(reply.clone()),
                Err(status) => Err(tonic::Status::with_details(
                    status.code(),
                    status.message(),
                    status.details().to_vec().into(),
                )),
            }
        }
    }

    fn target() -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], 7233))
    }

    const SHARD: usize = 7;

    async fn relayed(answer: Result<ForwardReply, tonic::Status>) -> Result<(), WireError> {
        relay_rename(
            &ScriptedForwarder(answer),
            target(),
            MetadataMap::new(),
            &rename_request(Some(ProtoWorkflowId::from(aion_core::WorkflowId::new_v4()))),
            SHARD,
        )
        .await
        .map(|_response| ())
    }

    /// THE RELAY, PINNED: every refusal the owner authors arrives at the caller
    /// as the OWNER'S OWN typed error — not as `NotOwner`.
    ///
    /// These four are the refusals a forwarded rename actually produces, and
    /// forwarding runs BEFORE the local blank-name check, so on a clustered
    /// node this is the normal path for all of them. Flattened into `NotOwner`
    /// they would read to the console as "some other node owns this shard",
    /// which is retryable — so the console would retry forever a request that
    /// can never succeed, with the cause nowhere in the answer.
    #[tokio::test]
    async fn a_forwarded_refusal_reaches_the_caller_as_the_owners_own_error()
    -> Result<(), Box<dyn std::error::Error>> {
        let refusals = [
            WireError::invalid_input("display_name must not be blank"),
            WireError::not_found("workflow 7 has no recorded history"),
            WireError::namespace_denied("tenant-b is not visible to this caller"),
            WireError::invalid_state(
                "run is Running but is not resident on this node; retry once it is resident",
            ),
        ];
        for refusal in refusals {
            let status = crate::api::grpc::status_from_wire_error(refusal.clone());
            let relayed = relayed(Err(status))
                .await
                .err()
                .ok_or("a refused forward must not be reported as success")?;
            assert_eq!(
                relayed.code, refusal.code,
                "the owner's code must reach the caller, not NotOwner: {relayed:?}"
            );
            assert_eq!(relayed.message, refusal.message);
            assert_ne!(
                relayed.code,
                aion_proto::WireErrorCode::NotOwner,
                "flattening the owner's answer is the defect this pins"
            );
        }
        Ok(())
    }

    /// The ONLY case that may answer `NotOwner`: a transport failure, which
    /// carries no answer the owner authored, so there is nothing to relay and
    /// re-resolving is the honest instruction.
    #[tokio::test]
    async fn a_forward_that_never_reached_the_owner_is_not_owner()
    -> Result<(), Box<dyn std::error::Error>> {
        let relayed = relayed(Err(tonic::Status::unavailable("forward dial failed")))
            .await
            .err()
            .ok_or("a failed dial must not be reported as success")?;
        assert_eq!(relayed.code, aion_proto::WireErrorCode::NotOwner);
        assert_eq!(relayed.error_type.as_deref(), Some("NotOwner"));
        assert!(
            relayed.message.contains(&format!("shard {SHARD}")),
            "the refusal names the shard to re-resolve: {}",
            relayed.message
        );
        Ok(())
    }

    /// The owner's SUCCESS is relayed verbatim too — the recorded name comes
    /// back from the node that recorded it.
    #[tokio::test]
    async fn the_owners_reply_is_relayed_verbatim() -> Result<(), Box<dyn std::error::Error>> {
        let run_id = aion_core::RunId::new_v4();
        let reply = ForwardReply::Rename(aion_proto::generated::RenameResponse {
            run_id: Some(aion_proto::generated::RunId {
                uuid: run_id.to_string(),
            }),
            display_name: String::from("Nightly settlement"),
        });
        let response = relay_rename(
            &ScriptedForwarder(Ok(reply)),
            target(),
            MetadataMap::new(),
            &rename_request(Some(ProtoWorkflowId::from(aion_core::WorkflowId::new_v4()))),
            SHARD,
        )
        .await?;
        assert_eq!(response.display_name, "Nightly settlement");
        assert_eq!(
            response.run_id.map(|run| run.uuid),
            Some(run_id.to_string())
        );
        Ok(())
    }

    /// A forwarder answering a DIFFERENT request is a wiring bug, not a routing
    /// one: it is reported as a backend error rather than dressed up as a
    /// retryable wrong-owner.
    #[tokio::test]
    async fn a_mismatched_reply_is_a_backend_error() -> Result<(), Box<dyn std::error::Error>> {
        let reply = ForwardReply::Cancel(aion_proto::generated::CancelResponse {});
        let relayed = relayed(Ok(reply))
            .await
            .err()
            .ok_or("a mismatched reply must not be reported as success")?;
        assert_eq!(relayed.code, aion_proto::WireErrorCode::Backend);
        Ok(())
    }
}
