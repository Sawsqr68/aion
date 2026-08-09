//! The mutating tools.
//!
//! None of the three carries `readOnlyHint`. Each states its destructive and
//! idempotent character explicitly, because the spec's defaults for an omitted
//! hint are the dangerous readings and a client that trusted them would
//! mis-present every one of these.

use aion_mcp::tools::descriptor::{
    Destructiveness, Idempotence, Tool, ToolAnnotations, WorldScope,
};
use serde_json::json;

use super::shared::{
    namespace_property, object_schema, output_schema, status_values, uuid_property,
};

/// `start_run` — start a workflow execution.
pub(crate) fn start_run() -> Tool {
    Tool {
        name: "start_run".to_owned(),
        title: Some("Start a run".to_owned()),
        description: Some(
            "Start a workflow of a deployed type and get its handles back immediately. Set \
             await_completion to have the server hand you a task instead: the call returns a \
             taskId at once, the run proceeds, and tasks/get carries the outcome when it \
             lands. await_completion requires the tasks extension — without it the call is \
             refused rather than silently turned into a plain start. Cancelling that task \
             stops the WAITING, not the run; use the cancel tool to stop the run."
                .to_owned(),
        ),
        input_schema: object_schema(
            json!({
                "namespace": namespace_property(),
                "workflow_type": {
                    "type": "string",
                    "minLength": 1,
                    "description": "A deployed workflow type. An undeployed type is refused.",
                },
                "input": {
                    "description": "The workflow's input, as plain JSON. Omit for no input.",
                },
                "task_queue": {
                    "type": "string",
                    "description": "Task queue this run's activities dispatch to. Omit to use \
                                    the namespace's default queue.",
                },
                "routing_key": {
                    "type": "string",
                    "description": "Steers placement onto a specific shard in a clustered \
                                    deployment. Omit unless you have been told to set it.",
                },
                "await_completion": {
                    "type": "boolean",
                    "description": "When true the call is answered as a task that settles when \
                                    the run reaches a terminal status.",
                },
            }),
            &["namespace", "workflow_type"],
        ),
        output_schema: Some(output_schema(
            json!({
                "namespace": { "type": "string" },
                "workflow_id": { "type": "string" },
                "run_id": { "type": "string" },
                "workflow_type": { "type": "string" },
                "status": { "type": "string", "enum": status_values() },
                "awaited": {
                    "type": "boolean",
                    "description": "True when this result is the settled outcome of an awaited \
                                    start rather than the immediate handles.",
                },
            }),
            &[
                "namespace",
                "workflow_id",
                "run_id",
                "workflow_type",
                "awaited",
            ],
        )),
        // Starting adds a new execution; it destroys nothing. It is NOT
        // idempotent: two calls start two runs.
        annotations: ToolAnnotations::mutating(
            Destructiveness::Additive,
            Idempotence::Repeating,
            WorldScope::Closed,
        ),
    }
}

/// `signal` — deliver a signal to a run.
pub(crate) fn signal() -> Tool {
    Tool {
        name: "signal".to_owned(),
        title: Some("Signal a run".to_owned()),
        description: Some(
            "Deliver a named signal to a running workflow. The signal is recorded durably in \
             the run's history, so delivering the same signal twice records it twice and the \
             workflow sees both."
                .to_owned(),
        ),
        input_schema: object_schema(
            json!({
                "namespace": namespace_property(),
                "workflow_id": uuid_property("The workflow to signal."),
                "run_id": uuid_property("Which run. Omit for the latest."),
                "signal_name": {
                    "type": "string",
                    "minLength": 1,
                    "description": "A signal name the workflow's code listens for.",
                },
                "payload": { "description": "The signal payload, as plain JSON. Omit for none." },
            }),
            &["namespace", "workflow_id", "signal_name"],
        ),
        output_schema: Some(output_schema(
            json!({
                "workflow_id": { "type": "string" },
                "signal_name": { "type": "string" },
                "delivered": { "type": "boolean" },
            }),
            &["workflow_id", "signal_name", "delivered"],
        )),
        annotations: ToolAnnotations::mutating(
            Destructiveness::Additive,
            Idempotence::Repeating,
            WorldScope::Closed,
        ),
    }
}

/// `cancel` — cancel a run.
pub(crate) fn cancel() -> Tool {
    Tool {
        name: "cancel".to_owned(),
        title: Some("Cancel a run".to_owned()),
        description: Some(
            "Cancel a running workflow. This is destructive: the run stops and records a \
             terminal cancellation it cannot be talked out of. Confirm the run is the one you \
             mean with describe_run first."
                .to_owned(),
        ),
        input_schema: object_schema(
            json!({
                "namespace": namespace_property(),
                "workflow_id": uuid_property("The workflow to cancel."),
                "run_id": uuid_property("Which run. Omit for the latest."),
                "reason": {
                    "type": "string",
                    "description": "Why it is being cancelled. Recorded in history.",
                },
            }),
            &["namespace", "workflow_id"],
        ),
        output_schema: Some(output_schema(
            json!({
                "workflow_id": { "type": "string" },
                "cancelled": { "type": "boolean" },
                "reason": { "type": "string" },
            }),
            &["workflow_id", "cancelled", "reason"],
        )),
        // Destructive, and idempotent: a second cancel of an already-cancelled
        // run changes nothing further.
        annotations: ToolAnnotations::mutating(
            Destructiveness::Destructive,
            Idempotence::Idempotent,
            WorldScope::Closed,
        ),
    }
}
