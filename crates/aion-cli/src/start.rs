//! The `aion start` verb: turning the parsed flags into a start request and
//! the start's outcome into the CLI's JSON.
//!
//! Split out of `main.rs`, which is a thin entry point by house rule and was
//! already well over the 500-code-line limit before this verb grew its `--name`
//! flag (#211). Keeping the start surface here means adding to it does not add
//! to that file.

use aion_client::{Client, StartOptions, StartOutcome, WorkflowHandle};
use aion_core::Payload;
use anyhow::{Context, Result};
use serde_json::Value;

use crate::output::{start_output, to_value};

pub(crate) async fn start_workflow_outcome(
    client: &Client,
    workflow_type: &str,
    input: Payload,
    display_name: Option<String>,
) -> Result<StartOutcome> {
    client
        .start(
            workflow_type.to_owned(),
            input,
            StartOptions {
                display_name,
                ..StartOptions::default()
            },
        )
        .await
        .context("failed to start workflow")
}

pub(crate) async fn start_workflow_handle(
    client: &Client,
    workflow_type: &str,
    input: Payload,
) -> Result<WorkflowHandle> {
    // `aion run` names nothing: the one-motion verb compiles, deploys, starts
    // and awaits a document, and the operator never sees the run listed.
    Ok(start_workflow_outcome(client, workflow_type, input, None)
        .await?
        .handle)
}

pub(crate) async fn start_workflow(
    client: &Client,
    workflow_type: &str,
    input: Payload,
    display_name: Option<String>,
) -> Result<Value> {
    let outcome = start_workflow_outcome(client, workflow_type, input, display_name).await?;
    to_value(start_output(&outcome))
}
