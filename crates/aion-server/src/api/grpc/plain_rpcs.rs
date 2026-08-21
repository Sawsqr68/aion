//! The gRPC service's PURE-DELEGATION RPCs.
//!
//! Ten of the `WorkflowService` methods share one shape and carry no routing,
//! no forwarding, and no `cfg` arms: authenticate the caller, decode the wire
//! message, call the shared handler, encode the reply. Their bodies live here
//! as free functions so `mod.rs` keeps only the RPCs that genuinely differ —
//! the forwarded workflow-lifecycle ones, whose cluster routing IS the
//! interesting part — and stays inside the house 500-code-line limit.
//!
//! The trait impl in `mod.rs` still declares every method (a trait impl is one
//! block); each of these ten delegates to its function here in a single line,
//! so the wire surface is still readable in one place.

use aion_proto::generated;
use tonic::{Request, Response, Status};

use crate::ServerState;
use crate::api::{handlers, schedule_handlers};

use super::auth::caller_from_metadata;
use super::convert::{
    decode_count_request, decode_create_schedule_request, decode_describe_request,
    decode_list_request, decode_list_schedules_request, decode_read_history_request,
    decode_schedule_id_request, decode_update_schedule_request, encode_count_response,
    encode_create_schedule_response, encode_delete_schedule_response, encode_describe_response,
    encode_describe_schedule_response, encode_list_response, encode_list_schedules_response,
    encode_pause_schedule_response, encode_read_history_response, encode_resume_schedule_response,
    encode_update_schedule_response,
};
use super::status::status_from_wire_error;

pub(super) async fn list_workflows(
    state: &ServerState,
    request: Request<generated::ListWorkflowsRequest>,
) -> Result<Response<generated::ListWorkflowsResponse>, Status> {
    let caller = caller_from_metadata(request.metadata(), state).await?;
    let response = handlers::list(
        state.namespace_guard(),
        &caller,
        decode_list_request(request.into_inner()),
    )
    .await
    .map_err(status_from_wire_error)?;
    Ok(Response::new(encode_list_response(response)))
}

pub(super) async fn count_workflows(
    state: &ServerState,
    request: Request<generated::CountWorkflowsRequest>,
) -> Result<Response<generated::CountWorkflowsResponse>, Status> {
    let caller = caller_from_metadata(request.metadata(), state).await?;
    let response = handlers::count(
        state.namespace_guard(),
        &caller,
        decode_count_request(request.into_inner()),
    )
    .await
    .map_err(status_from_wire_error)?;
    Ok(Response::new(encode_count_response(response)))
}

pub(super) async fn describe_workflow(
    state: &ServerState,
    request: Request<generated::DescribeWorkflowRequest>,
) -> Result<Response<generated::DescribeWorkflowResponse>, Status> {
    let caller = caller_from_metadata(request.metadata(), state).await?;
    let outcome = handlers::describe(
        state.namespace_guard(),
        &caller,
        decode_describe_request(request.into_inner()),
    )
    .await
    .map_err(status_from_wire_error)?;
    Ok(Response::new(encode_describe_response(outcome.response)))
}

pub(super) async fn read_history(
    state: &ServerState,
    request: Request<generated::ReadHistoryRequest>,
) -> Result<Response<generated::ReadHistoryResponse>, Status> {
    let caller = caller_from_metadata(request.metadata(), state).await?;
    let response = handlers::read_history(
        state.namespace_guard(),
        &caller,
        decode_read_history_request(request.into_inner()),
    )
    .await
    .map_err(status_from_wire_error)?;
    Ok(Response::new(encode_read_history_response(response)))
}

pub(super) async fn create_schedule(
    state: &ServerState,
    request: Request<generated::CreateScheduleRequest>,
) -> Result<Response<generated::CreateScheduleResponse>, Status> {
    let caller = caller_from_metadata(request.metadata(), state).await?;
    let response = schedule_handlers::create_schedule(
        state.namespace_guard(),
        &caller,
        decode_create_schedule_request(request.into_inner()),
    )
    .await
    .map_err(status_from_wire_error)?;
    Ok(Response::new(encode_create_schedule_response(response)))
}

pub(super) async fn update_schedule(
    state: &ServerState,
    request: Request<generated::UpdateScheduleRequest>,
) -> Result<Response<generated::UpdateScheduleResponse>, Status> {
    let caller = caller_from_metadata(request.metadata(), state).await?;
    let response = schedule_handlers::update_schedule(
        state.namespace_guard(),
        &caller,
        decode_update_schedule_request(request.into_inner()),
    )
    .await
    .map_err(status_from_wire_error)?;
    Ok(Response::new(encode_update_schedule_response(response)))
}

pub(super) async fn pause_schedule(
    state: &ServerState,
    request: Request<generated::ScheduleIdRequest>,
) -> Result<Response<generated::PauseScheduleResponse>, Status> {
    let caller = caller_from_metadata(request.metadata(), state).await?;
    let response = schedule_handlers::pause_schedule(
        state.namespace_guard(),
        &caller,
        decode_schedule_id_request(request.into_inner()),
    )
    .await
    .map_err(status_from_wire_error)?;
    Ok(Response::new(encode_pause_schedule_response(response)))
}

pub(super) async fn resume_schedule(
    state: &ServerState,
    request: Request<generated::ScheduleIdRequest>,
) -> Result<Response<generated::ResumeScheduleResponse>, Status> {
    let caller = caller_from_metadata(request.metadata(), state).await?;
    let response = schedule_handlers::resume_schedule(
        state.namespace_guard(),
        &caller,
        decode_schedule_id_request(request.into_inner()),
    )
    .await
    .map_err(status_from_wire_error)?;
    Ok(Response::new(encode_resume_schedule_response(response)))
}

pub(super) async fn delete_schedule(
    state: &ServerState,
    request: Request<generated::ScheduleIdRequest>,
) -> Result<Response<generated::DeleteScheduleResponse>, Status> {
    let caller = caller_from_metadata(request.metadata(), state).await?;
    let response = schedule_handlers::delete_schedule(
        state.namespace_guard(),
        &caller,
        decode_schedule_id_request(request.into_inner()),
    )
    .await
    .map_err(status_from_wire_error)?;
    Ok(Response::new(encode_delete_schedule_response(response)))
}

pub(super) async fn list_schedules(
    state: &ServerState,
    request: Request<generated::ListSchedulesRequest>,
) -> Result<Response<generated::ListSchedulesResponse>, Status> {
    let caller = caller_from_metadata(request.metadata(), state).await?;
    let response = schedule_handlers::list_schedules(
        state.namespace_guard(),
        &caller,
        decode_list_schedules_request(request.into_inner()),
    )
    .await
    .map_err(status_from_wire_error)?;
    Ok(Response::new(encode_list_schedules_response(response)))
}

pub(super) async fn describe_schedule(
    state: &ServerState,
    request: Request<generated::ScheduleIdRequest>,
) -> Result<Response<generated::DescribeScheduleResponse>, Status> {
    let caller = caller_from_metadata(request.metadata(), state).await?;
    let response = schedule_handlers::describe_schedule(
        state.namespace_guard(),
        &caller,
        decode_schedule_id_request(request.into_inner()),
    )
    .await
    .map_err(status_from_wire_error)?;
    Ok(Response::new(encode_describe_schedule_response(response)))
}
