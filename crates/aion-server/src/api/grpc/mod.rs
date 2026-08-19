//! tonic workflow service adapter.

/// Caller-identity extraction from gRPC request metadata.
mod auth;
/// Codec conversions between the generated wire messages and `aion-proto` types.
/// Proto/generated wire conversions. Crate-visible because the HTTP edge
/// reuses the rename conversions when it forwards to a shard owner (#211).
pub(crate) mod convert;
/// The owner side of namespace-mint routing (`MintNamespace`).
mod mint_resolve;
/// The pure-delegation RPC bodies (no routing, no forwarding, no cfg arms).
mod plain_rpcs;
/// Cluster request routing/resolution (steered start + forward-or-local).
mod routing_resolve;
/// Wire-error-to-tonic-Status mapping.
mod status;

pub(crate) use auth::caller_from_metadata;
pub(crate) use status::{status_from_wire_error, status_with_code};

use aion_proto::generated::{self, workflow_service_server::WorkflowServiceServer};
use tonic::{Request, Response, Status};

use crate::namespace::MintCredentials;
use crate::routing::{ForwardReply, ForwardRequest};
use crate::{CallerIdentity, ServerState, api::handlers};
use convert::decode_workflow_id;
use convert::{
    decode_cancel_request, decode_pause_request, decode_query_request, decode_rename_request,
    decode_reopen_request, decode_resume_request, decode_signal_request, decode_start_request,
    encode_cancel_response, encode_pause_response, encode_query_response, encode_rename_response,
    encode_reopen_response, encode_resume_response, encode_signal_response, encode_start_response,
};
use routing_resolve::{RouteResolution, StartResolution};

/// Cloneable tonic implementation for workflow management.
#[derive(Clone)]
pub struct WorkflowGrpcService {
    state: ServerState,
}

impl WorkflowGrpcService {
    /// Build a tonic workflow service from shared server state.
    #[must_use]
    pub const fn new(state: ServerState) -> Self {
        Self { state }
    }

    async fn caller<T>(&self, request: &Request<T>) -> Result<CallerIdentity, Status> {
        caller_from_metadata(request.metadata(), &self.state).await
    }
}

/// Construct the generated tonic server wrapper.
#[must_use]
pub fn workflow_service(state: ServerState) -> WorkflowServiceServer<WorkflowGrpcService> {
    WorkflowServiceServer::new(WorkflowGrpcService::new(state))
}

#[tonic::async_trait]
impl generated::workflow_service_server::WorkflowService for WorkflowGrpcService {
    async fn start_workflow(
        &self,
        request: Request<generated::StartWorkflowRequest>,
    ) -> Result<Response<generated::StartWorkflowResponse>, Status> {
        if self.state.drain_state().is_draining() {
            return Err(Status::unavailable(
                "server is draining and not accepting new workflow starts",
            ));
        }
        let caller = self.caller(&request).await?;
        // R-4 steered start over R-1 remint: a non-empty routing key steers the
        // start to its shard owner (forwarding when remote); otherwise the R-1
        // remint places it locally. `placement` is `None` for single-node /
        // non-clustered boots (and own-all scope), so the engine mints as usual —
        // default path unchanged.
        let (metadata, _ext, inner) = request.into_parts();
        let placement = match self.resolve_start(&inner, &metadata).await {
            StartResolution::Reject(status) => return Err(status),
            StartResolution::Reply(reply) => return Ok(Response::new(reply)),
            StartResolution::Local(placement) => placement,
        };
        // Minted-on-use safety net (Phase 1 S6): mint/gate the authorized
        // namespace before the engine start, sharing the SAME policy the
        // worker-registration seam applies. Auth-scoped (`guard.scope` runs
        // inside the handler); placement is the orthogonal steered-start id.
        // Carry the caller's own credentials onto any forwarded mint, so
        // the namespace shard's owner authorizes THIS caller rather than an
        // anonymous peer (the same verbatim-metadata discipline the R-3
        // request forwarder already applies to a forwarded start).
        let minter = self
            .state
            .namespace_minter()
            .with_caller_credentials(MintCredentials::from_grpc_metadata(&metadata));
        let response = handlers::start_with_placement(
            self.state.namespace_guard(),
            &caller,
            decode_start_request(inner),
            placement,
            Some(&minter),
        )
        .await
        .map_err(status_from_wire_error)?;
        return Ok(Response::new(encode_start_response(response)));
    }

    async fn signal(
        &self,
        request: Request<generated::SignalRequest>,
    ) -> Result<Response<generated::SignalResponse>, Status> {
        let caller = self.caller(&request).await?;
        let (metadata, _ext, inner) = request.into_parts();
        let workflow_id = inner.workflow_id.clone().map(decode_workflow_id);
        match self
            .resolve_route(
                workflow_id,
                &metadata,
                ForwardRequest::Signal(inner.clone()),
            )
            .await
        {
            RouteResolution::Reject(status) => return Err(status),
            RouteResolution::Reply(ForwardReply::Signal(reply)) => {
                return Ok(Response::new(reply));
            }
            RouteResolution::Reply(_) => {
                return Err(Status::internal("forwarder returned a mismatched reply"));
            }
            RouteResolution::Local => {
                let response = handlers::signal(
                    self.state.namespace_guard(),
                    &caller,
                    decode_signal_request(inner),
                )
                .await
                .map_err(status_from_wire_error)?;
                return Ok(Response::new(encode_signal_response(response)));
            }
        }
    }

    async fn query(
        &self,
        request: Request<generated::QueryRequest>,
    ) -> Result<Response<generated::QueryResponse>, Status> {
        let caller = self.caller(&request).await?;
        let (metadata, _ext, inner) = request.into_parts();
        let workflow_id = inner.workflow_id.clone().map(decode_workflow_id);
        match self
            .resolve_route(workflow_id, &metadata, ForwardRequest::Query(inner.clone()))
            .await
        {
            RouteResolution::Reject(status) => return Err(status),
            RouteResolution::Reply(ForwardReply::Query(reply)) => {
                return Ok(Response::new(reply));
            }
            RouteResolution::Reply(_) => {
                return Err(Status::internal("forwarder returned a mismatched reply"));
            }
            RouteResolution::Local => {
                let response = handlers::query(
                    self.state.namespace_guard(),
                    &caller,
                    decode_query_request(inner),
                )
                .await
                .map_err(status_from_wire_error)?;
                return Ok(Response::new(encode_query_response(response)));
            }
        }
    }

    async fn cancel(
        &self,
        request: Request<generated::CancelRequest>,
    ) -> Result<Response<generated::CancelResponse>, Status> {
        let caller = self.caller(&request).await?;
        let (metadata, _ext, inner) = request.into_parts();
        let workflow_id = inner.workflow_id.clone().map(decode_workflow_id);
        match self
            .resolve_route(
                workflow_id,
                &metadata,
                ForwardRequest::Cancel(inner.clone()),
            )
            .await
        {
            RouteResolution::Reject(status) => return Err(status),
            RouteResolution::Reply(ForwardReply::Cancel(reply)) => {
                return Ok(Response::new(reply));
            }
            RouteResolution::Reply(_) => {
                return Err(Status::internal("forwarder returned a mismatched reply"));
            }
            RouteResolution::Local => {
                let response = handlers::cancel(
                    &self.state,
                    self.state.namespace_guard(),
                    &caller,
                    decode_cancel_request(inner),
                )
                .await
                .map_err(status_from_wire_error)?;
                return Ok(Response::new(encode_cancel_response(response)));
            }
        }
    }

    async fn reopen(
        &self,
        request: Request<generated::ReopenRequest>,
    ) -> Result<Response<generated::ReopenResponse>, Status> {
        let caller = self.caller(&request).await?;
        let (metadata, _ext, inner) = request.into_parts();
        let workflow_id = inner.workflow_id.clone().map(decode_workflow_id);
        match self
            .resolve_route(
                workflow_id,
                &metadata,
                ForwardRequest::Reopen(inner.clone()),
            )
            .await
        {
            RouteResolution::Reject(status) => return Err(status),
            RouteResolution::Reply(ForwardReply::Reopen(reply)) => {
                return Ok(Response::new(reply));
            }
            RouteResolution::Reply(_) => {
                return Err(Status::internal("forwarder returned a mismatched reply"));
            }
            RouteResolution::Local => {
                let response = handlers::reopen(
                    self.state.namespace_guard(),
                    &caller,
                    decode_reopen_request(inner),
                )
                .await
                .map_err(status_from_wire_error)?;
                return Ok(Response::new(encode_reopen_response(response)));
            }
        }
    }

    async fn pause(
        &self,
        request: Request<generated::PauseRequest>,
    ) -> Result<Response<generated::PauseResponse>, Status> {
        let caller = self.caller(&request).await?;
        let (metadata, _ext, inner) = request.into_parts();
        let workflow_id = inner.workflow_id.clone().map(decode_workflow_id);
        match self
            .resolve_route(workflow_id, &metadata, ForwardRequest::Pause(inner.clone()))
            .await
        {
            RouteResolution::Reject(status) => return Err(status),
            RouteResolution::Reply(ForwardReply::Pause(reply)) => {
                return Ok(Response::new(reply));
            }
            RouteResolution::Reply(_) => {
                return Err(Status::internal("forwarder returned a mismatched reply"));
            }
            RouteResolution::Local => {
                let response = handlers::pause(
                    self.state.namespace_guard(),
                    &caller,
                    decode_pause_request(inner),
                )
                .await
                .map_err(status_from_wire_error)?;
                return Ok(Response::new(encode_pause_response(response)));
            }
        }
    }

    async fn resume(
        &self,
        request: Request<generated::ResumeRequest>,
    ) -> Result<Response<generated::ResumeResponse>, Status> {
        let caller = self.caller(&request).await?;
        let (metadata, _ext, inner) = request.into_parts();
        let workflow_id = inner.workflow_id.clone().map(decode_workflow_id);
        match self
            .resolve_route(
                workflow_id,
                &metadata,
                ForwardRequest::Resume(inner.clone()),
            )
            .await
        {
            RouteResolution::Reject(status) => return Err(status),
            RouteResolution::Reply(ForwardReply::Resume(reply)) => {
                return Ok(Response::new(reply));
            }
            RouteResolution::Reply(_) => {
                return Err(Status::internal("forwarder returned a mismatched reply"));
            }
            RouteResolution::Local => {
                let response = handlers::resume(
                    self.state.namespace_guard(),
                    &caller,
                    decode_resume_request(inner),
                )
                .await
                .map_err(status_from_wire_error)?;
                return Ok(Response::new(encode_resume_response(response)));
            }
        }
    }

    async fn rename(
        &self,
        request: Request<generated::RenameRequest>,
    ) -> Result<Response<generated::RenameResponse>, Status> {
        let caller = self.caller(&request).await?;
        let (metadata, _ext, inner) = request.into_parts();
        let workflow_id = inner.workflow_id.clone().map(decode_workflow_id);
        match self
            .resolve_route(
                workflow_id,
                &metadata,
                ForwardRequest::Rename(inner.clone()),
            )
            .await
        {
            RouteResolution::Reject(status) => return Err(status),
            RouteResolution::Reply(ForwardReply::Rename(reply)) => {
                return Ok(Response::new(reply));
            }
            RouteResolution::Reply(_) => {
                return Err(Status::internal("forwarder returned a mismatched reply"));
            }
            RouteResolution::Local => {
                let response = handlers::rename(
                    self.state.namespace_guard(),
                    &caller,
                    decode_rename_request(inner),
                )
                .await
                .map_err(status_from_wire_error)?;
                return Ok(Response::new(encode_rename_response(response)));
            }
        }
    }

    async fn list_workflows(
        &self,
        request: Request<generated::ListWorkflowsRequest>,
    ) -> Result<Response<generated::ListWorkflowsResponse>, Status> {
        plain_rpcs::list_workflows(&self.state, request).await
    }

    async fn count_workflows(
        &self,
        request: Request<generated::CountWorkflowsRequest>,
    ) -> Result<Response<generated::CountWorkflowsResponse>, Status> {
        plain_rpcs::count_workflows(&self.state, request).await
    }

    async fn describe_workflow(
        &self,
        request: Request<generated::DescribeWorkflowRequest>,
    ) -> Result<Response<generated::DescribeWorkflowResponse>, Status> {
        plain_rpcs::describe_workflow(&self.state, request).await
    }

    async fn create_schedule(
        &self,
        request: Request<generated::CreateScheduleRequest>,
    ) -> Result<Response<generated::CreateScheduleResponse>, Status> {
        plain_rpcs::create_schedule(&self.state, request).await
    }

    async fn update_schedule(
        &self,
        request: Request<generated::UpdateScheduleRequest>,
    ) -> Result<Response<generated::UpdateScheduleResponse>, Status> {
        plain_rpcs::update_schedule(&self.state, request).await
    }

    async fn pause_schedule(
        &self,
        request: Request<generated::ScheduleIdRequest>,
    ) -> Result<Response<generated::PauseScheduleResponse>, Status> {
        plain_rpcs::pause_schedule(&self.state, request).await
    }

    async fn resume_schedule(
        &self,
        request: Request<generated::ScheduleIdRequest>,
    ) -> Result<Response<generated::ResumeScheduleResponse>, Status> {
        plain_rpcs::resume_schedule(&self.state, request).await
    }

    async fn delete_schedule(
        &self,
        request: Request<generated::ScheduleIdRequest>,
    ) -> Result<Response<generated::DeleteScheduleResponse>, Status> {
        plain_rpcs::delete_schedule(&self.state, request).await
    }

    async fn list_schedules(
        &self,
        request: Request<generated::ListSchedulesRequest>,
    ) -> Result<Response<generated::ListSchedulesResponse>, Status> {
        plain_rpcs::list_schedules(&self.state, request).await
    }

    async fn describe_schedule(
        &self,
        request: Request<generated::ScheduleIdRequest>,
    ) -> Result<Response<generated::DescribeScheduleResponse>, Status> {
        plain_rpcs::describe_schedule(&self.state, request).await
    }

    /// Mint (or gate) an already-authorized namespace set on THIS node — the
    /// owner side of namespace-mint routing. See
    /// [`WorkflowGrpcService::mint_namespace_here`] for the authorization and
    /// policy-parity contract this delegates to.
    async fn mint_namespace(
        &self,
        request: Request<generated::MintNamespaceRequest>,
    ) -> Result<Response<generated::MintNamespaceResponse>, Status> {
        self.mint_namespace_here(request).await
    }
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, sync::Arc};

    use aion::EngineBuilder;
    use aion_core::{Event, EventEnvelope, Payload, WorkflowId, WorkflowStatus};
    use aion_proto::{
        ProtoWireError, WireError, WireErrorCode,
        convert::{decode_core_value, encode_core_value},
        generated::workflow_service_server::WorkflowService,
    };
    use aion_store::{
        EventStore, InMemoryStore, WriteToken,
        visibility::{VisibilityRecord, VisibilityStore},
    };
    use chrono::Utc;
    use prost::Message;
    use serde_json::json;
    use tonic::{Code, Request};

    use super::convert::{decode_envelope, encode_envelope, encode_payload};
    use super::*;
    use crate::{
        NamespaceResolver,
        config::{
            AuthConfig, AuthoringConfig, DeployConfig, ListenConfig, MetricsConfig,
            NamespaceConfig, NamespaceMode, OpsConsoleAssetSource, OpsConsoleConfig, RuntimeConfig,
            WebSocketConfig, WorkerConfig,
        },
    };

    const NAMESPACE: &str = "tenant-a";
    const TOKEN: &str = "test-token";

    /// Server state whose bearer validation matches the compiled auth path:
    /// under `feature = "auth"` a real [`crate::auth::JwksCache`] is fetched
    /// from a live fixture JWKS endpoint; otherwise the development token path
    /// needs no cache.
    async fn server_state(
        resolver: NamespaceResolver,
        runtime: RuntimeConfig,
    ) -> Result<ServerState, Box<dyn std::error::Error>> {
        #[cfg(feature = "auth")]
        {
            let url = crate::auth::test_support::serve_jwks()?;
            let refresh = std::time::Duration::from_secs(runtime.auth.jwks_refresh_seconds);
            let cache = crate::auth::JwksCache::new(url, refresh).await?;
            Ok(ServerState::from_parts_with_jwks(resolver, runtime, cache))
        }
        #[cfg(not(feature = "auth"))]
        {
            // Yield to preserve the async signature the auth arm requires.
            tokio::task::yield_now().await;
            Ok(ServerState::from_parts(resolver, runtime))
        }
    }

    #[tokio::test]
    async fn in_process_tonic_start_and_list_use_shared_handlers()
    -> Result<(), Box<dyn std::error::Error>> {
        let backing = Arc::new(InMemoryStore::default());
        let store: Arc<dyn EventStore> = backing.clone();
        let visibility_store: Arc<dyn VisibilityStore> = backing;
        let engine = Arc::new(
            EngineBuilder::new()
                .store_arc(Arc::clone(&store))
                .visibility_store_arc(Arc::clone(&visibility_store))
                .scheduler_threads(1)
                .build()
                .await?,
        );
        store
            .append(
                WriteToken::recorder(),
                &workflow_id(),
                &[started_event()?],
                0,
            )
            .await?;
        visibility_store
            .record_visibility(VisibilityRecord {
                workflow_id: workflow_id(),
                run_id: aion_core::RunId::new(uuid::Uuid::from_u128(2)),
                workflow_type: String::from("fixture"),
                status: WorkflowStatus::Running,
                start_time: Utc::now(),
                close_time: None,
                failed_step: None,
                failure_reason: None,
                search_attributes: std::collections::HashMap::from([(
                    crate::namespace::NAMESPACE_ATTRIBUTE.to_owned(),
                    aion_core::SearchAttributeValue::String(NAMESPACE.to_owned()),
                )]),
            })
            .await?;
        let resolver = NamespaceResolver::from_config(
            crate::config::NamespaceConfig {
                mode: NamespaceMode::SharedEngine,
            },
            engine,
        );
        let state = server_state(resolver.clone(), runtime_config()).await?;
        let service = WorkflowGrpcService::new(state);

        let mut start = Request::new(generated::StartWorkflowRequest {
            namespace: NAMESPACE.to_owned(),
            workflow_type: "missing-workflow".to_owned(),
            input: Some(encode_payload(proto_payload()?)),
            routing_key: None,
            task_queue: None,
            display_name: None,
        });
        apply_metadata(start.metadata_mut())?;
        let start_error = service.start_workflow(start).await;
        let status = start_error
            .err()
            .ok_or_else(|| WireError::backend("expected error"))?;
        assert_eq!(status.code(), Code::NotFound);
        let detail = ProtoWireError::decode(status.details())?;
        assert_eq!(detail.error_type.as_deref(), Some("WorkflowTypeNotFound"));
        assert!(detail.message.contains("missing-workflow"));

        let list_filter = encode_core_value(
            NAMESPACE,
            None,
            &aion_store::visibility::ListWorkflowsFilter {
                workflow_type: Some(String::from("fixture")),
                status: Some(WorkflowStatus::Running),
                ..aion_store::visibility::ListWorkflowsFilter::default()
            },
        )?;
        let mut list = Request::new(generated::ListWorkflowsRequest {
            namespace: NAMESPACE.to_owned(),
            filter: Some(encode_envelope(list_filter)),
        });
        apply_metadata(list.metadata_mut())?;
        let response = service.list_workflows(list).await?.into_inner();

        assert_eq!(response.summaries.len(), 1);
        let summary = response
            .summaries
            .into_iter()
            .next()
            .map(decode_envelope)
            .map(|envelope| decode_core_value::<aion_store::visibility::WorkflowSummary>(&envelope))
            .transpose()?
            .ok_or_else(|| WireError::backend("summary missing"))?;
        assert_eq!(summary.workflow_id, workflow_id());
        // The seeded history records no namespace attribute, so durable
        // ownership verification must reject targeted access with NotFound:
        // a missing ownership attribute is indistinguishable from a
        // nonexistent workflow (anti-existence-leak), and NamespaceDenied is
        // reserved for callers without a grant for the requested namespace.
        assert_eq!(
            resolver
                .verify_workflow_ownership(NAMESPACE, &workflow_id())
                .await
                .err()
                .map(|error| error.to_wire_error().code),
            Some(WireErrorCode::NotFound)
        );
        Ok(())
    }

    /// A gRPC service over a TERMINAL run that records its own namespace.
    ///
    /// Terminal is the durable posture in which no live recorder can exist, so
    /// the engine's non-resident append path is legitimately open (a `Running`
    /// run with no registered handle is refused instead — see the engine's
    /// residency gate).
    ///
    /// `register_display_name` selects whether the engine's search-attribute
    /// schema knows `aion.display_name`; the blank-name arm does not need it,
    /// because the name is refused before any append is attempted.
    async fn rename_fixture(
        register_display_name: bool,
    ) -> Result<(WorkflowGrpcService, Arc<dyn EventStore>), Box<dyn std::error::Error>> {
        rename_fixture_with_terminal(register_display_name, true).await
    }

    /// The same fixture, with the run's terminality selectable, so a test can
    /// also exercise the engine's residency refusal for a durably-`Running`
    /// run that no registry handle owns.
    async fn rename_fixture_with_terminal(
        register_display_name: bool,
        terminal: bool,
    ) -> Result<(WorkflowGrpcService, Arc<dyn EventStore>), Box<dyn std::error::Error>> {
        let backing = Arc::new(InMemoryStore::default());
        let store: Arc<dyn EventStore> = backing.clone();
        let visibility_store: Arc<dyn VisibilityStore> = backing;
        let mut schema = aion_core::SearchAttributeSchema::new();
        schema.register(
            crate::namespace::NAMESPACE_ATTRIBUTE,
            aion_core::SearchAttributeType::String,
        )?;
        if register_display_name {
            schema.register(
                crate::namespace::DISPLAY_NAME_ATTRIBUTE,
                aion_core::SearchAttributeType::String,
            )?;
        }
        let engine = Arc::new(
            EngineBuilder::new()
                .store_arc(Arc::clone(&store))
                .visibility_store_arc(Arc::clone(&visibility_store))
                .search_attribute_schema(schema)
                .scheduler_threads(1)
                .build()
                .await?,
        );
        let namespace_event = |seq: u64| Event::SearchAttributesUpdated {
            envelope: EventEnvelope {
                seq,
                recorded_at: Utc::now(),
                workflow_id: workflow_id(),
            },
            workflow_id: workflow_id(),
            attributes: std::collections::HashMap::from([(
                crate::namespace::NAMESPACE_ATTRIBUTE.to_owned(),
                aion_core::SearchAttributeValue::String(NAMESPACE.to_owned()),
            )]),
        };
        let mut events = vec![started_event()?, namespace_event(2)];
        if terminal {
            events.push(Event::WorkflowCompleted {
                envelope: EventEnvelope {
                    seq: 3,
                    recorded_at: Utc::now(),
                    workflow_id: workflow_id(),
                },
                result: payload()?,
            });
        }
        store
            .append(WriteToken::recorder(), &workflow_id(), &events, 0)
            .await?;
        let resolver = NamespaceResolver::from_config(
            crate::config::NamespaceConfig {
                mode: NamespaceMode::SharedEngine,
            },
            engine,
        );
        let state = server_state(resolver, runtime_config()).await?;
        Ok((WorkflowGrpcService::new(state), store))
    }

    /// #211: a rename over the gRPC service records the name durably and
    /// returns it as recorded (trimmed), and a SECOND rename supersedes it
    /// while history keeps both — the whole wire path, guard included.
    #[tokio::test]
    async fn in_process_tonic_rename_records_the_name_and_supersedes_it()
    -> Result<(), Box<dyn std::error::Error>> {
        let (service, store) = rename_fixture(true).await?;

        let rename = |name: &str| {
            let mut request = Request::new(generated::RenameRequest {
                namespace: NAMESPACE.to_owned(),
                workflow_id: Some(generated::WorkflowId {
                    uuid: workflow_id().to_string(),
                }),
                // Absent run id: the rename lands on the LATEST run.
                run_id: None,
                display_name: name.to_owned(),
            });
            apply_metadata(request.metadata_mut()).map(|()| request)
        };

        // Surrounding whitespace is trimmed, and the response says so.
        let response = service.rename(rename("  Nightly settlement  ")?).await?;
        let response = response.into_inner();
        assert_eq!(response.display_name, "Nightly settlement");
        assert_eq!(
            response.run_id.map(|id| id.uuid),
            Some(aion_core::RunId::new(uuid::Uuid::from_u128(1)).to_string())
        );

        service
            .rename(rename("Nightly settlement (rerun)")?)
            .await?;

        let history = store.read_history(&workflow_id()).await?;
        assert_eq!(
            aion_core::display_name(&history).as_deref(),
            Some("Nightly settlement (rerun)"),
            "the latest recorded name wins"
        );
        let names: Vec<_> = history
            .iter()
            .filter_map(|event| match event {
                Event::SearchAttributesUpdated { attributes, .. } => attributes
                    .get(aion_core::DISPLAY_NAME_ATTRIBUTE)
                    .and_then(|value| match value {
                        aion_core::SearchAttributeValue::String(name) => Some(name.clone()),
                        _ => None,
                    }),
                _ => None,
            })
            .collect();
        assert_eq!(
            names,
            vec![
                String::from("Nightly settlement"),
                String::from("Nightly settlement (rerun)")
            ],
            "history keeps every name the run has worn"
        );
        Ok(())
    }

    /// #211: a rename carrying a blank name is refused as tonic
    /// `InvalidArgument` before anything is appended.
    #[tokio::test]
    async fn in_process_tonic_rename_refuses_a_blank_name() -> Result<(), Box<dyn std::error::Error>>
    {
        let (service, store) = rename_fixture(false).await?;
        let before = store.read_history(&workflow_id()).await?.len();

        let mut request = Request::new(generated::RenameRequest {
            namespace: NAMESPACE.to_owned(),
            workflow_id: Some(generated::WorkflowId {
                uuid: workflow_id().to_string(),
            }),
            run_id: None,
            display_name: String::from("   "),
        });
        apply_metadata(request.metadata_mut())?;
        let status = service
            .rename(request)
            .await
            .err()
            .ok_or_else(|| WireError::backend("expected a blank-name refusal"))?;
        assert_eq!(status.code(), Code::InvalidArgument);
        assert_eq!(
            store.read_history(&workflow_id()).await?.len(),
            before,
            "a refused rename appends nothing"
        );
        Ok(())
    }

    /// #211 single writer, over the wire: renaming a durably-`Running` run that
    /// no registry handle owns is refused as tonic `FailedPrecondition`
    /// carrying the typed `InvalidState` detail — never served through a second
    /// writer appending behind the run's live recorder. This is the wire
    /// contract `docs/operations/API.md` states for the route.
    #[tokio::test]
    async fn in_process_tonic_rename_of_a_non_resident_running_run_is_failed_precondition()
    -> Result<(), Box<dyn std::error::Error>> {
        let (service, store) = rename_fixture_with_terminal(true, false).await?;
        let before = store.read_history(&workflow_id()).await?.len();

        let mut request = Request::new(generated::RenameRequest {
            namespace: NAMESPACE.to_owned(),
            workflow_id: Some(generated::WorkflowId {
                uuid: workflow_id().to_string(),
            }),
            run_id: None,
            display_name: String::from("Nightly settlement"),
        });
        apply_metadata(request.metadata_mut())?;
        let status = service
            .rename(request)
            .await
            .err()
            .ok_or_else(|| WireError::backend("expected a residency refusal"))?;

        assert_eq!(status.code(), Code::FailedPrecondition);
        let detail = ProtoWireError::decode(status.details())?;
        assert_eq!(detail.error_type.as_deref(), Some("InvalidState"));
        assert!(
            detail.message.contains("not resident"),
            "the refusal must name the reason: {}",
            detail.message
        );
        assert_eq!(
            store.read_history(&workflow_id()).await?.len(),
            before,
            "a refused rename appends nothing"
        );
        Ok(())
    }

    /// Reopening a terminal-Completed workflow over the gRPC service surfaces
    /// the engine's `InvalidState` precondition as tonic `FailedPrecondition`
    /// carrying the typed `InvalidState` detail (AO-007 C35/C38).
    #[tokio::test]
    async fn in_process_tonic_reopen_completed_is_failed_precondition_invalid_state()
    -> Result<(), Box<dyn std::error::Error>> {
        let backing = Arc::new(InMemoryStore::default());
        let store: Arc<dyn EventStore> = backing.clone();
        let visibility_store: Arc<dyn VisibilityStore> = backing;
        let engine = Arc::new(
            EngineBuilder::new()
                .store_arc(Arc::clone(&store))
                .visibility_store_arc(Arc::clone(&visibility_store))
                .scheduler_threads(1)
                .build()
                .await?,
        );
        // A terminal-Completed run whose history records its namespace, so the
        // guard's durable-ownership verification passes and the request reaches
        // the engine reopen op (which rejects Completed with InvalidState).
        store
            .append(
                WriteToken::recorder(),
                &workflow_id(),
                &[
                    started_event()?,
                    Event::SearchAttributesUpdated {
                        envelope: EventEnvelope {
                            seq: 2,
                            recorded_at: Utc::now(),
                            workflow_id: workflow_id(),
                        },
                        workflow_id: workflow_id(),
                        attributes: std::collections::HashMap::from([(
                            crate::namespace::NAMESPACE_ATTRIBUTE.to_owned(),
                            aion_core::SearchAttributeValue::String(NAMESPACE.to_owned()),
                        )]),
                    },
                    Event::WorkflowCompleted {
                        envelope: EventEnvelope {
                            seq: 3,
                            recorded_at: Utc::now(),
                            workflow_id: workflow_id(),
                        },
                        result: payload()?,
                    },
                ],
                0,
            )
            .await?;
        let resolver = NamespaceResolver::from_config(
            crate::config::NamespaceConfig {
                mode: NamespaceMode::SharedEngine,
            },
            engine,
        );
        let state = server_state(resolver, runtime_config()).await?;
        let service = WorkflowGrpcService::new(state);

        let mut reopen = Request::new(generated::ReopenRequest {
            namespace: NAMESPACE.to_owned(),
            workflow_id: Some(generated::WorkflowId {
                uuid: workflow_id().to_string(),
            }),
            run_id: None,
        });
        apply_metadata(reopen.metadata_mut())?;
        let status = service
            .reopen(reopen)
            .await
            .err()
            .ok_or_else(|| WireError::backend("expected a reopen precondition error"))?;
        assert_eq!(status.code(), Code::FailedPrecondition);
        let detail = ProtoWireError::decode(status.details())?;
        assert_eq!(detail.error_type.as_deref(), Some("InvalidState"));
        assert_eq!(
            detail.code,
            aion_proto::ProtoWireErrorCode::InvalidState as i32
        );
        Ok(())
    }

    fn apply_metadata(
        metadata: &mut tonic::metadata::MetadataMap,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Bearer credential accepted by the compiled authentication path: a
        // JWT minted against the fixture JWKS under `feature = "auth"`, the
        // development shared-secret token otherwise.
        #[cfg(feature = "auth")]
        let bearer = crate::auth::test_support::mint_token("alice", NAMESPACE)?;
        #[cfg(not(feature = "auth"))]
        let bearer = TOKEN.to_owned();
        metadata.insert("authorization", format!("Bearer {bearer}").parse()?);
        metadata.insert("x-aion-subject", "alice".parse()?);
        metadata.insert("x-aion-namespaces", NAMESPACE.parse()?);
        Ok(())
    }

    /// Test runtime settings with authentication enabled; under
    /// `feature = "auth"` validation runs against the [`server_state`]-injected
    /// JWKS cache, so the configured dev-secret `jwks_url` is never fetched.
    fn runtime_config() -> RuntimeConfig {
        RuntimeConfig {
            listen: ListenConfig {
                grpc: SocketAddr::from(([127, 0, 0, 1], 50051)),
                http: SocketAddr::from(([127, 0, 0, 1], 8080)),
            },
            tls: None,
            auth: AuthConfig {
                enabled: true,
                jwks_url: Some(TOKEN.to_owned()),
                jwks_refresh_seconds: 300,
            },
            ops_console: OpsConsoleConfig {
                source: OpsConsoleAssetSource::Embedded,
            },
            namespace: NamespaceConfig {
                mode: NamespaceMode::SharedEngine,
            },
            worker: WorkerConfig {
                heartbeat_window: std::time::Duration::from_secs(30),
                ..WorkerConfig::default()
            },
            websocket: WebSocketConfig {
                outbound_buffer_bound: 32,
                event_broadcast_capacity: Some(64),
                cluster_broadcast_capacity: Some(64),
            },
            workflow_packages: Vec::new(),
            deploy: DeployConfig::default(),
            authoring: AuthoringConfig::default(),
            dev: crate::config::DevConfig::default(),
            outbox: crate::config::OutboxConfig::default(),
            observability: crate::config::ObservabilityConfig::with_flush_policy(64, 0),
            mcp: crate::config::ResolvedMcpConfig::default(),
            scheduler_threads: 1,
            jit_threshold: None,
            query_timeout: Some(std::time::Duration::from_secs(10)),
            default_namespace: "default".to_owned(),
            auto_create: crate::config::AutoCreate::Open,
            max_in_flight_activities: crate::config::DEFAULT_MAX_IN_FLIGHT_ACTIVITIES,
            drain_timeout: std::time::Duration::from_secs(30),
            metrics: MetricsConfig { enabled: true },
            owned_shards: Vec::new(),
            cors_allowed_origins: Vec::new(),
        }
    }

    fn started_event() -> Result<Event, aion_core::PayloadError> {
        Ok(Event::WorkflowStarted {
            envelope: EventEnvelope {
                seq: 1,
                recorded_at: Utc::now(),
                workflow_id: workflow_id(),
            },
            workflow_type: "fixture".to_owned(),
            input: payload()?,
            run_id: aion_core::RunId::new(uuid::Uuid::from_u128(1)),
            parent_run_id: None,
            package_version: aion_core::PackageVersion::new("a".repeat(64)),
        })
    }

    fn proto_payload() -> Result<aion_proto::ProtoPayload, aion_core::PayloadError> {
        Ok(payload()?.into())
    }

    fn payload() -> Result<Payload, aion_core::PayloadError> {
        Payload::from_json(&json!({ "fixture": "input" }))
    }

    fn workflow_id() -> WorkflowId {
        WorkflowId::new(uuid::Uuid::from_u128(1))
    }
}
