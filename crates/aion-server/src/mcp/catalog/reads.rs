//! The read tools.

use aion_mcp::tools::descriptor::{Tool, ToolAnnotations, WorldScope};
use serde_json::{Value, json};

use super::shared::{
    cursor_property, limit_property, namespace_property, object_schema, output_schema,
    status_values, uuid_property,
};

/// `describe_run` — the join an agent should make first.
pub(crate) fn describe_run() -> Tool {
    Tool {
        name: "describe_run".to_owned(),
        title: Some("Describe a run".to_owned()),
        description: Some(
            "The first call to make about any run. One read answers: what the run is, what \
             status history projects it to, which step it is on right now, which of its \
             dispatched activities nothing can currently serve, and the exact handles for \
             reading each step's agent transcript. Every handle it returns is complete and \
             usable as-is — pass them back verbatim to read_transcript or read_history rather \
             than assembling your own."
                .to_owned(),
        ),
        input_schema: object_schema(
            json!({
                "namespace": namespace_property(),
                "workflow_id": uuid_property("The workflow to describe."),
                "run_id": uuid_property(
                    "Which generation of the workflow to describe. Omit for the latest run of \
                     the chain — the usual choice. A workflow that continued-as-new has several \
                     runs and they do NOT share step numbering.",
                ),
            }),
            &["namespace", "workflow_id"],
        ),
        output_schema: Some(describe_run_output()),
        annotations: ToolAnnotations::read_only(WorldScope::Closed),
    }
}

/// The `describe_run` result contract.
///
/// Split out from the tool so neither the argument contract nor the result
/// contract has to be read past the other.
fn describe_run_output() -> Value {
    output_schema(
        json!({
            "namespace": { "type": "string" },
            "workflow_id": { "type": "string" },
            "run_id": {
                "type": "string",
                "description": "The run this description is about — resolved for you when \
                                you omitted it. Carry it into every transcript read.",
            },
            "workflow_type": { "type": "string" },
            "status": { "type": "string", "enum": status_values() },
            "started_at": { "type": "string" },
            "ended_at": { "type": ["string", "null"] },
            "failed_step": { "type": ["string", "null"] },
            "failure_reason": { "type": ["string", "null"] },
            "history_head_seq": { "type": "integer" },
            "current_step": {
                "type": ["object", "null"],
                "description": "The activity this run dispatched most recently that has \
                                recorded no terminal event. Null when nothing is in flight.",
                "properties": {
                    "activity_id": { "type": "integer" },
                    "activity_type": { "type": "string" },
                    "attempt": { "type": "integer" },
                    "task_queue": { "type": "string" },
                    "node": { "type": ["string", "null"] },
                    "dispatched_at": { "type": "string" },
                },
                "required": ["activity_id", "activity_type", "attempt", "dispatched_at"],
                "additionalProperties": false,
            },
            "unserved": {
                "type": "array",
                "description": "Activities this run dispatched that NO live worker can take. \
                                An empty list is the healthy answer. A non-empty one is the \
                                difference between 'a worker is working on it' and 'it is \
                                parked with nobody to take it' — which the status alone \
                                cannot express, because both project Running.",
                "items": { "type": "object" },
            },
            "transcripts": {
                "type": "array",
                "description": "One entry per retained agent-transcript stream of this run. \
                                Each carries the COMPLETE read_transcript handle, run_id \
                                included.",
                "items": transcript_handle_schema(),
            },
        }),
        &[
            "namespace",
            "workflow_id",
            "run_id",
            "workflow_type",
            "status",
            "started_at",
            "history_head_seq",
            "unserved",
            "transcripts",
        ],
    )
}

/// One emitted `read_transcript` handle.
///
/// `run_id` is in `required`, not merely present: a handle that comes back out
/// of `describe_run` must be complete enough to hand straight to
/// `read_transcript`, and a handle without a run names a pile of streams fused
/// across generations rather than one conversation.
fn transcript_handle_schema() -> Value {
    object_schema(
        json!({
            "workflow_id": { "type": "string" },
            "run_id": { "type": "string" },
            "activity_id": { "type": "integer" },
            "attempt": { "type": "integer" },
            "head_seq": { "type": "integer" },
            "transcript_run_ambiguous": { "type": "boolean" },
            "colliding_runs": { "type": "array", "items": { "type": "string" } },
        }),
        &[
            "workflow_id",
            "run_id",
            "activity_id",
            "attempt",
            "head_seq",
            "transcript_run_ambiguous",
            "colliding_runs",
        ],
    )
}

/// `read_transcript` — a cursor page of one step's agent transcript.
pub(crate) fn read_transcript() -> Tool {
    Tool {
        name: "read_transcript".to_owned(),
        title: Some("Read a step transcript".to_owned()),
        description: Some(
            "A page of what the agent running one activity attempt actually said and did. \
             Returns immediately with whatever exists now — it never waits for more. To follow \
             a live step, call again with from_seq = the next_from_seq you were given. \
             run_id is REQUIRED and is not decoration: the durable transcript key carries no \
             run, so a handle without one names a pile of streams fused across generations \
             rather than one conversation."
                .to_owned(),
        ),
        input_schema: object_schema(
            json!({
                "namespace": namespace_property(),
                "workflow_id": uuid_property("The workflow that owns the step."),
                "run_id": uuid_property(
                    "The run that dispatched this attempt. REQUIRED. Take it from describe_run; \
                     a run that never dispatched this activity/attempt is refused.",
                ),
                "activity_id": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "The activity ordinal within the run.",
                },
                "attempt": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "The one-based delivery attempt. Each attempt is its own \
                                    transcript.",
                },
                "from_seq": cursor_property(
                    "Read records with store_seq >= this. Omit to read from the start.",
                ),
                "limit": limit_property("Maximum records to return in this page."),
            }),
            &[
                "namespace",
                "workflow_id",
                "run_id",
                "activity_id",
                "attempt",
            ],
        ),
        output_schema: Some(output_schema(
            json!({
                "workflow_id": { "type": "string" },
                "run_id": { "type": "string" },
                "activity_id": { "type": "integer" },
                "attempt": { "type": "integer" },
                "events": { "type": "array", "items": { "type": "object" } },
                "next_from_seq": {
                    "type": ["integer", "null"],
                    "description": "Pass back as from_seq for the next page. Null means this \
                                    page reached the end of what is retained.",
                },
                "head_seq": {
                    "type": "integer",
                    "description": "The stream head at read time: the next store_seq that will \
                                    be written.",
                },
                "transcript_run_ambiguous": {
                    "type": "boolean",
                    "description": "TRUE means another run of this workflow dispatched the same \
                                    activity ordinal and attempt, so the durable stream is a \
                                    fusion of both generations. Do not read an ambiguous \
                                    transcript as one agent's conversation.",
                },
                "transcript_run_ambiguity": { "type": ["string", "null"] },
                "colliding_runs": { "type": "array", "items": { "type": "string" } },
            }),
            &[
                "workflow_id",
                "run_id",
                "activity_id",
                "attempt",
                "events",
                "head_seq",
                "transcript_run_ambiguous",
                "colliding_runs",
            ],
        )),
        annotations: ToolAnnotations::read_only(WorldScope::Closed),
    }
}

/// `read_history` — a cursor page of the durable event history.
pub(crate) fn read_history() -> Tool {
    Tool {
        name: "read_history".to_owned(),
        title: Some("Read run history".to_owned()),
        description: Some(
            "A page of the workflow's authoritative event history — the record everything else \
             is projected from. Large payloads are elided to a size marker rather than \
             truncated silently. Returns immediately; page with next_from_seq."
                .to_owned(),
        ),
        input_schema: object_schema(
            json!({
                "namespace": namespace_property(),
                "workflow_id": uuid_property("The workflow whose history to read."),
                "from_seq": cursor_property("Read events with seq >= this. Omit to start at 0."),
                "limit": limit_property("Maximum events to return in this page."),
                "payload_limit_bytes": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Elide any payload larger than this many bytes to a size \
                                    marker. 0 disables elision and returns payloads whole.",
                },
            }),
            &["namespace", "workflow_id"],
        ),
        output_schema: Some(output_schema(
            json!({
                "workflow_id": { "type": "string" },
                "events": { "type": "array", "items": { "type": "object" } },
                "next_from_seq": { "type": ["integer", "null"] },
                "head_seq": { "type": "integer" },
                "page_is_immutable": {
                    "type": "boolean",
                    "description": "TRUE when every event on this page sits strictly below the \
                                    head at read time. History is append-only, so such a page \
                                    can never change and may be cached indefinitely. FALSE \
                                    means the page touches the head and more may follow.",
                },
            }),
            &["workflow_id", "events", "head_seq", "page_is_immutable"],
        )),
        annotations: ToolAnnotations::read_only(WorldScope::Closed),
    }
}

/// `list_runs` — filtered enumeration.
pub(crate) fn list_runs() -> Tool {
    Tool {
        name: "list_runs".to_owned(),
        title: Some("List runs".to_owned()),
        description: Some(
            "Find runs in a namespace by type, status, or start window. Use this to locate a \
             workflow_id you do not already hold — never invent one."
                .to_owned(),
        ),
        input_schema: object_schema(
            json!({
                "namespace": namespace_property(),
                "workflow_type": { "type": "string", "description": "Exact workflow type." },
                "status": { "type": "string", "enum": status_values() },
                "started_after": {
                    "type": "string",
                    "description": "RFC 3339 instant; only runs started at or after it.",
                },
                "started_before": {
                    "type": "string",
                    "description": "RFC 3339 instant; only runs started at or before it.",
                },
                "limit": limit_property("Maximum rows to return."),
                "offset": cursor_property("Rows to skip before returning."),
            }),
            &["namespace"],
        ),
        output_schema: Some(output_schema(
            json!({
                "namespace": { "type": "string" },
                "runs": { "type": "array", "items": { "type": "object" } },
                "count": { "type": "integer" },
            }),
            &["namespace", "runs", "count"],
        )),
        annotations: ToolAnnotations::read_only(WorldScope::Closed),
    }
}

/// `query` — a mid-run read of workflow-defined state.
pub(crate) fn query() -> Tool {
    Tool {
        name: "query".to_owned(),
        title: Some("Query a running workflow".to_owned()),
        description: Some(
            "Ask a running workflow one of the queries its own code registered. Records \
             nothing and changes nothing: a query runs against replayed state. An unknown \
             query name is answered as a failure, never as an empty result."
                .to_owned(),
        ),
        input_schema: object_schema(
            json!({
                "namespace": namespace_property(),
                "workflow_id": uuid_property("The workflow to query."),
                "run_id": uuid_property("Which run. Omit for the latest."),
                "query_name": {
                    "type": "string",
                    "minLength": 1,
                    "description": "A query name the workflow registered. Not a free-text \
                                    question.",
                },
            }),
            &["namespace", "workflow_id", "query_name"],
        ),
        output_schema: Some(output_schema(
            json!({
                "workflow_id": { "type": "string" },
                "query_name": { "type": "string" },
                "result": {
                    "description": "The query's own result value, whatever shape the workflow \
                                    returned.",
                },
            }),
            &["workflow_id", "query_name", "result"],
        )),
        annotations: ToolAnnotations::read_only(WorldScope::Closed),
    }
}
