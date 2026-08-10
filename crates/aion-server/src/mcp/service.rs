//! The Aion tool service: the MCP surface's only door into the engine.
//!
//! Every tool here is an IN-PROCESS call onto the same shared handlers the HTTP
//! and gRPC transports use. Nothing loops back through HTTP: a self-call would
//! be a second authorization path, a second serialization, and a second place
//! for the answer to differ from the one the console shows.

use std::time::Duration;

use aion_mcp::tools::service::{
    ToolCall, ToolCatalog, ToolFailure, ToolOutcome, ToolService, ToolTaskDecision,
};
use serde_json::json;

use crate::{CallerIdentity, ServerState};

use super::catalog::aion_tool_catalog;
use super::instructions::instructions;
use super::tools::{authoring, control, describe, history, list, start, transcript};

/// The Aion tool implementation.
pub struct AionToolService {
    state: ServerState,
    catalog: ToolCatalog,
    instructions: String,
    await_poll_interval: Duration,
}

impl AionToolService {
    /// Build the service over the server's shared state.
    ///
    /// # Errors
    ///
    /// [`ServerError::Config`](crate::ServerError::Config) when a published
    /// tool schema fails to compile — a server defect, surfaced at startup
    /// rather than on a caller's first `tools/call`.
    pub fn new(
        state: ServerState,
        await_poll_interval: Duration,
    ) -> Result<Self, crate::ServerError> {
        let catalog = aion_tool_catalog().map_err(|error| crate::ServerError::Config {
            message: format!("the MCP tool catalog is invalid: {error}"),
        })?;
        Ok(Self {
            state,
            catalog,
            instructions: instructions(),
            await_poll_interval,
        })
    }
}

#[async_trait::async_trait]
impl ToolService for AionToolService {
    type Caller = CallerIdentity;

    fn catalog(&self) -> &ToolCatalog {
        &self.catalog
    }

    fn instructions(&self) -> &str {
        &self.instructions
    }

    fn task_decision(&self, call: &ToolCall) -> ToolTaskDecision {
        if start::awaits_completion(call) {
            ToolTaskDecision::Task
        } else {
            ToolTaskDecision::Inline
        }
    }

    async fn call(
        &self,
        caller: &Self::Caller,
        call: ToolCall,
    ) -> Result<ToolOutcome, ToolFailure> {
        match call.name.as_str() {
            "describe_run" => describe::describe_run(&self.state, caller, &call).await,
            "read_transcript" => transcript::read_transcript(&self.state, caller, &call).await,
            "read_history" => history::read_history(&self.state, caller, &call).await,
            "list_runs" => list::list_runs(&self.state, caller, &call).await,
            "query" => control::query(&self.state, caller, &call).await,
            "start_run" => {
                start::start_run(&self.state, caller, &call, self.await_poll_interval).await
            }
            "signal" => control::signal(&self.state, caller, &call).await,
            "cancel" => control::cancel(&self.state, caller, &call).await,
            "list_documents" => authoring::list_documents(&self.state, caller, &call).await,
            "read_document" => authoring::read_document(&self.state, caller, &call).await,
            "check_document" => authoring::check_document(&self.state, caller, &call).await,
            "save_document" => authoring::save_document(&self.state, caller, &call).await,
            "deploy_document" => authoring::deploy_document(&self.state, caller, &call).await,
            // Unreachable through the dispatcher, which refuses an unpublished
            // name with `-32602` before this is called. Handled anyway: a
            // catch-all that cannot be reached is still the difference between
            // a legible refusal and a panic if the catalog and this match ever
            // drift apart.
            other => Err(ToolFailure::new(
                format!("`{other}` is not a tool this server implements"),
                json!({ "code": "unknown_tool", "tool": other }),
            )),
        }
    }
}
