//! The authoring tools: the AWL workspace, checked and deployed over MCP.
//!
//! These five tools are the writing hands of the assistant: list and read what
//! the workspace holds, check drafted source, save it, and deploy exactly the
//! saved revision. They mirror the ops console's authoring facade — same
//! seams, same refusals — so a document authored over MCP and one authored in
//! the console are indistinguishable on the server.

use aion_mcp::tools::descriptor::{
    Destructiveness, Idempotence, Tool, ToolAnnotations, WorldScope,
};
use serde_json::{Value, json};

use super::shared::{object_schema, output_schema};

/// The workspace-relative document path property.
fn document_path_property(description: &str) -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "description": format!(
            "{description} Workspace-relative, no `..` components, ending in `.awl` \
             (e.g. `flows/etl.awl`)."
        ),
    })
}

/// The AWL source property.
fn source_property(description: &str) -> Value {
    json!({
        "type": "string",
        "description": description,
    })
}

/// The content-hash property: the revision identity save and read return.
fn content_hash_property(description: &str) -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "description": description,
    })
}

/// `list_documents` — enumerate the authoring workspace.
pub(crate) fn list_documents() -> Tool {
    Tool {
        name: "list_documents".to_owned(),
        title: Some("List AWL documents".to_owned()),
        description: Some(
            "Every AWL document in the server's authoring workspace, sorted by path. Use this \
             to see what exists before reading, saving, or deploying — document paths come \
             from here or from a previous save, never from a guess. An empty list means the \
             workspace holds no documents yet, which is a normal starting state."
                .to_owned(),
        ),
        input_schema: object_schema(json!({}), &[]),
        output_schema: Some(output_schema(
            json!({
                "documents": {
                    "type": "array",
                    "items": object_schema(
                        json!({
                            "path": { "type": "string" },
                            "name": { "type": "string" },
                        }),
                        &["path", "name"],
                    ),
                },
                "count": { "type": "integer" },
            }),
            &["documents", "count"],
        )),
        annotations: ToolAnnotations::read_only(WorldScope::Closed),
    }
}

/// `read_document` — one saved document, with its revision identity.
pub(crate) fn read_document() -> Tool {
    Tool {
        name: "read_document".to_owned(),
        title: Some("Read an AWL document".to_owned()),
        description: Some(
            "The saved source of one workspace document, with the content_hash of exactly \
             those bytes. The hash is the revision identity deploy_document verifies: if you \
             deploy the document as-is, pass this hash; if you edit first, save_document gives \
             you the new one."
                .to_owned(),
        ),
        input_schema: object_schema(
            json!({
                "path": document_path_property("The document to read."),
            }),
            &["path"],
        ),
        output_schema: Some(output_schema(
            json!({
                "path": { "type": "string" },
                "source": { "type": "string" },
                "content_hash": { "type": "string" },
            }),
            &["path", "source", "content_hash"],
        )),
        annotations: ToolAnnotations::read_only(WorldScope::Closed),
    }
}

/// `check_document` — the AWL checker over drafted source.
pub(crate) fn check_document() -> Tool {
    Tool {
        name: "check_document".to_owned(),
        title: Some("Check AWL source".to_owned()),
        description: Some(
            "Run the AWL checker over source you are drafting, WITHOUT saving anything. \
             Returns whether the document parses (`ok`), whether it would be accepted by \
             deploy (`deploys_green`), the step count, and every diagnostic with its line \
             and column. Pass `path` — the workspace path the document lives at or will be \
             saved at — so schema imports resolve against the workspace; omit it only for a \
             document with no imports. Check after every structural edit and treat each \
             diagnostic as a source defect: a document that is not green will be refused by \
             deploy_document."
                .to_owned(),
        ),
        input_schema: object_schema(
            json!({
                "source": source_property("The AWL source to check, in full."),
                "path": document_path_property(
                    "Where this document lives (or will live) in the workspace, so its \
                     schema imports resolve.",
                ),
            }),
            &["source"],
        ),
        output_schema: Some(output_schema(
            json!({
                "ok": { "type": "boolean" },
                "deploys_green": { "type": "boolean" },
                "steps": { "type": ["integer", "null"] },
                "diagnostics": {
                    "type": "array",
                    "items": object_schema(
                        json!({
                            "class": { "type": "string" },
                            "message": { "type": "string" },
                            "line": { "type": "integer" },
                            "column": { "type": "integer" },
                        }),
                        &["class", "message", "line", "column"],
                    ),
                },
            }),
            &["ok", "deploys_green", "steps", "diagnostics"],
        )),
        annotations: ToolAnnotations::read_only(WorldScope::Closed),
    }
}

/// `save_document` — write one document into the workspace.
pub(crate) fn save_document() -> Tool {
    Tool {
        name: "save_document".to_owned(),
        title: Some("Save an AWL document".to_owned()),
        description: Some(
            "Write the full source of one document into the authoring workspace, creating or \
             replacing it, and get back the content_hash of what was saved. Every save also \
             stores a revision keyed by that hash, so no previously saved state is lost. The \
             returned content_hash is what deploy_document requires — deploy immediately \
             after saving, with the hash you were just handed. Requires the deploy grant."
                .to_owned(),
        ),
        input_schema: object_schema(
            json!({
                "path": document_path_property("Where to save the document."),
                "source": source_property("The complete AWL source to save."),
            }),
            &["path", "source"],
        ),
        output_schema: Some(output_schema(
            json!({
                "path": { "type": "string" },
                "source": { "type": "string" },
                "content_hash": {
                    "type": "string",
                    "description": "The revision identity of the saved bytes. Pass it to \
                                    deploy_document unchanged.",
                },
            }),
            &["path", "source", "content_hash"],
        )),
        // Non-destructive: every save stores a content-addressed revision, so
        // the prior state remains recoverable. Idempotent: the same source
        // saves to the same bytes and the same hash.
        annotations: ToolAnnotations::mutating(
            Destructiveness::Additive,
            Idempotence::Idempotent,
            WorldScope::Closed,
        ),
    }
}

/// `deploy_document` — compile and load exactly the saved revision.
pub(crate) fn deploy_document() -> Tool {
    Tool {
        name: "deploy_document".to_owned(),
        title: Some("Deploy an AWL document".to_owned()),
        description: Some(
            "Compile the SAVED document at `path` and load it into the engine, making its \
             workflow type startable with start_run. `content_hash` must be the hash of the \
             currently saved document — take it from the save_document (or read_document) \
             result and pass it unchanged. A hash that does not match what is saved is \
             refused rather than deployed: the document changed since you last held it, so \
             re-read or re-save and deploy the returned hash. There is deliberately no way \
             to deploy unsaved source. Requires the deploy grant."
                .to_owned(),
        ),
        input_schema: object_schema(
            json!({
                "path": document_path_property("The saved document to deploy."),
                "content_hash": content_hash_property(
                    "The content hash save_document or read_document returned for the \
                     document as it is saved right now. Never construct one.",
                ),
            }),
            &["path", "content_hash"],
        ),
        output_schema: Some(output_schema(
            json!({
                "deployment": object_schema(
                    json!({
                        "deployment_id": { "type": "string" },
                        "document_path": { "type": "string" },
                        "content_hash": { "type": "string" },
                        "package_id": { "type": "string" },
                        "workflow_type": {
                            "type": "string",
                            "description": "The workflow type this deployment made startable \
                                            — pass it to start_run.",
                        },
                        "task_queue": { "type": "string" },
                        "workflow_id": { "type": ["string", "null"] },
                        "run_id": { "type": ["string", "null"] },
                    }),
                    &[
                        "deployment_id",
                        "document_path",
                        "content_hash",
                        "package_id",
                        "workflow_type",
                        "task_queue",
                        "workflow_id",
                        "run_id",
                    ],
                ),
                "steps": {
                    "type": "array",
                    "description": "What the guided deploy did, step by step: check, \
                                    compile, package, deploy.",
                    "items": object_schema(
                        json!({
                            "step": { "type": "string" },
                            "detail": { "type": "string" },
                        }),
                        &["step", "detail"],
                    ),
                },
            }),
            &["deployment", "steps"],
        )),
        // Non-destructive: each package version is a distinct content-hash
        // module, so deploying never overwrites deployed code. NOT idempotent:
        // although re-deploying the same saved revision loads the same
        // content-hash package, every call also appends a NEW deployment
        // record with a fresh deployment_id — repeating the call grows the
        // deployment ledger, and an honest hint says so.
        annotations: ToolAnnotations::mutating(
            Destructiveness::Additive,
            Idempotence::Repeating,
            WorldScope::Closed,
        ),
    }
}
