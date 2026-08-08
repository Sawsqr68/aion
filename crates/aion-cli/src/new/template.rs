//! Embedded scaffold templates for `aion new`.
//!
//! Each template is a hand-rolled manifest of `(target path, contents)`
//! pairs embedded with `include_str!` — no templating engine, no extra
//! dependency. Target paths and contents may carry the `{{name}}`
//! placeholder; the worker manifest additionally carries
//! `{{aion_worker_version}}`. Substitution lives in
//! [`crate::new::scaffold`].

use clap::ValueEnum;

use crate::new::agent::{AGENT_ACTIVITIES, AGENT_FILES, AGENT_WORKER_FILES};

/// One scaffolded file: the project-relative target path and the embedded
/// contents, both before placeholder substitution.
pub type ManifestFile = (&'static str, &'static str);

/// Scaffold templates selectable with `aion new --template`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Template {
    /// AWL-first typed workflow with no activities or external worker.
    Awl,
    /// Minimal typed workflow with no activities: start → complete.
    HelloWorld,
    /// Approval signal raced against a durable timeout, with a stage query
    /// and every outcome path.
    ApprovalFlow,
    /// Worker-served payment activity with workflow-driven retries, an
    /// approval race, a status query, and refund compensation.
    Saga,
    /// The durable dev pipeline: an agent develops a brief, a scoped
    /// verify-fix loop, a workspace gate, human review by signal, land —
    /// three composed workflows plus a CLI-shelling worker. Workflow-level
    /// I/O codecs and schemas are generated from the authored types module
    /// by `aion generate`. Requires `--worker rust`.
    DevPipeline,
    /// A durable agent loop: scout → act → verify → signal-gated human
    /// review, parameterised by prompts and a review deadline. The three
    /// agent steps are worker-served activities; the human approval pause is
    /// a durable `workflow.receive` with a timeout. Bundles no agent runtime
    /// — the driver stays worker-side. Workflow I/O codecs and schemas are
    /// generated from the authored types module by `aion generate`. Requires
    /// `--worker rust`.
    Agent,
}

/// Files every template emits. The dev-pipeline template replaces the
/// shared `gleam.toml` with its own (it needs a `gleeunit` dev-dependency
/// for the scaffolded hermetic test suite).
const SHARED_FILES: &[ManifestFile] = &[
    (
        "gleam.toml",
        include_str!("../../templates/shared/gleam.toml"),
    ),
    (
        ".gitignore",
        include_str!("../../templates/shared/gitignore"),
    ),
    (
        "aion.toml",
        include_str!("../../templates/shared/aion.toml"),
    ),
];

/// Shared files minus `gleam.toml`, for templates carrying their own.
const SHARED_FILES_WITHOUT_GLEAM_TOML: &[ManifestFile] = &[
    (
        ".gitignore",
        include_str!("../../templates/shared/gitignore"),
    ),
    (
        "aion.toml",
        include_str!("../../templates/shared/aion.toml"),
    ),
];
const SHARED_AWL_FILES: &[ManifestFile] = SHARED_FILES_WITHOUT_GLEAM_TOML;

const AWL_FILES: &[ManifestFile] = &[
    (
        "{{name}}.awl",
        include_str!("../../templates/awl/project.awl"),
    ),
    ("README.md", include_str!("../../templates/awl/README.md")),
];

const HELLO_WORLD_FILES: &[ManifestFile] = &[
    (
        "workflow.toml",
        include_str!("../../templates/hello_world/workflow.toml"),
    ),
    // Authored source of truth: the boundary types (types-first, ADR-014).
    (
        "src/{{name}}_io.gleam",
        include_str!("../../templates/hello_world/project_io.gleam"),
    ),
    // Pre-generated artifacts, byte-identical to what `aion generate`
    // reproduces from the types module (proven by the scaffold gates), so
    // the fresh scaffold builds without running the toolchain.
    (
        "src/{{name}}_codecs.gleam",
        include_str!("../../templates/hello_world/project_codecs.gleam"),
    ),
    (
        "schemas/input.json",
        include_str!("../../templates/hello_world/schemas/input.json"),
    ),
    (
        "schemas/output.json",
        include_str!("../../templates/hello_world/schemas/output.json"),
    ),
    (
        "src/{{name}}.gleam",
        include_str!("../../templates/hello_world/project.gleam"),
    ),
    (
        "README.md",
        include_str!("../../templates/hello_world/README.md"),
    ),
];

const APPROVAL_FLOW_FILES: &[ManifestFile] = &[
    (
        "workflow.toml",
        include_str!("../../templates/approval_flow/workflow.toml"),
    ),
    (
        "schemas/input.json",
        include_str!("../../templates/approval_flow/schemas/input.json"),
    ),
    (
        "schemas/output.json",
        include_str!("../../templates/approval_flow/schemas/output.json"),
    ),
    (
        "src/{{name}}.gleam",
        include_str!("../../templates/approval_flow/project.gleam"),
    ),
    (
        "README.md",
        include_str!("../../templates/approval_flow/README.md"),
    ),
];

const SAGA_FILES: &[ManifestFile] = &[
    (
        "workflow.toml",
        include_str!("../../templates/saga/workflow.toml"),
    ),
    (
        "schemas/input.json",
        include_str!("../../templates/saga/schemas/input.json"),
    ),
    (
        "schemas/output.json",
        include_str!("../../templates/saga/schemas/output.json"),
    ),
    (
        "src/{{name}}.gleam",
        include_str!("../../templates/saga/project.gleam"),
    ),
    ("README.md", include_str!("../../templates/saga/README.md")),
];

/// Rust worker crate serving the saga template's activities.
///
/// The on-disk template is `Cargo.toml.tmpl`, not `Cargo.toml`: cargo
/// excludes subdirectories containing a `Cargo.toml` when packaging this
/// crate, which would break `include_str!` on the published crate.
const SAGA_WORKER_FILES: &[ManifestFile] = &[
    (
        "worker/Cargo.toml",
        include_str!("../../templates/saga/worker/Cargo.toml.tmpl"),
    ),
    (
        "worker/src/main.rs",
        include_str!("../../templates/saga/worker/main.rs"),
    ),
];

const DEV_PIPELINE_FILES: &[ManifestFile] = &[
    (
        "gleam.toml",
        include_str!("../../templates/dev_pipeline/gleam.toml"),
    ),
    (
        "workflow.toml",
        include_str!("../../templates/dev_pipeline/workflow.toml"),
    ),
    // Authored source of truth: the boundary types (types-first, ADR-014).
    (
        "src/{{name}}_io.gleam",
        include_str!("../../templates/dev_pipeline/project_io.gleam"),
    ),
    // Pre-generated artifacts, byte-identical to what `aion generate`
    // reproduces from the types module (proven by the scaffold gates).
    (
        "src/{{name}}_codecs.gleam",
        include_str!("../../templates/dev_pipeline/project_codecs.gleam"),
    ),
    (
        "schemas/input.json",
        include_str!("../../templates/dev_pipeline/schemas/input.json"),
    ),
    (
        "schemas/input_placement.json",
        include_str!("../../templates/dev_pipeline/schemas/input_placement.json"),
    ),
    (
        "schemas/input_isolation.json",
        include_str!("../../templates/dev_pipeline/schemas/input_isolation.json"),
    ),
    (
        "schemas/output.json",
        include_str!("../../templates/dev_pipeline/schemas/output.json"),
    ),
    (
        "schemas/output_build_warm.json",
        include_str!("../../templates/dev_pipeline/schemas/output_build_warm.json"),
    ),
    (
        "schemas/dev_input.json",
        include_str!("../../templates/dev_pipeline/schemas/dev_input.json"),
    ),
    (
        "schemas/dev_input_workspace.json",
        include_str!("../../templates/dev_pipeline/schemas/dev_input_workspace.json"),
    ),
    (
        "schemas/dev_input_workspace_placement.json",
        include_str!("../../templates/dev_pipeline/schemas/dev_input_workspace_placement.json"),
    ),
    (
        "schemas/dev_input_workspace_isolation.json",
        include_str!("../../templates/dev_pipeline/schemas/dev_input_workspace_isolation.json"),
    ),
    (
        "schemas/dev_output.json",
        include_str!("../../templates/dev_pipeline/schemas/dev_output.json"),
    ),
    (
        "schemas/dev_output_dev_result.json",
        include_str!("../../templates/dev_pipeline/schemas/dev_output_dev_result.json"),
    ),
    (
        "schemas/dev_output_build_warm.json",
        include_str!("../../templates/dev_pipeline/schemas/dev_output_build_warm.json"),
    ),
    (
        "schemas/gate_input.json",
        include_str!("../../templates/dev_pipeline/schemas/gate_input.json"),
    ),
    (
        "schemas/gate_input_workspace.json",
        include_str!("../../templates/dev_pipeline/schemas/gate_input_workspace.json"),
    ),
    (
        "schemas/gate_input_workspace_placement.json",
        include_str!("../../templates/dev_pipeline/schemas/gate_input_workspace_placement.json"),
    ),
    (
        "schemas/gate_input_workspace_isolation.json",
        include_str!("../../templates/dev_pipeline/schemas/gate_input_workspace_isolation.json"),
    ),
    (
        "schemas/gate_input_scope.json",
        include_str!("../../templates/dev_pipeline/schemas/gate_input_scope.json"),
    ),
    (
        "schemas/gate_input_scope_kind.json",
        include_str!("../../templates/dev_pipeline/schemas/gate_input_scope_kind.json"),
    ),
    (
        "schemas/gate_output.json",
        include_str!("../../templates/dev_pipeline/schemas/gate_output.json"),
    ),
    (
        "schemas/gate_output_verdict.json",
        include_str!("../../templates/dev_pipeline/schemas/gate_output_verdict.json"),
    ),
    (
        "schemas/gate_output_verdict_outcome.json",
        include_str!("../../templates/dev_pipeline/schemas/gate_output_verdict_outcome.json"),
    ),
    (
        "src/{{name}}.gleam",
        include_str!("../../templates/dev_pipeline/project.gleam"),
    ),
    (
        "src/{{name}}_dev.gleam",
        include_str!("../../templates/dev_pipeline/project_dev.gleam"),
    ),
    (
        "src/{{name}}_gate.gleam",
        include_str!("../../templates/dev_pipeline/project_gate.gleam"),
    ),
    (
        "src/{{name}}_cli_ffi.erl",
        include_str!("../../templates/dev_pipeline/cli_ffi.erl"),
    ),
    (
        "src/{{name}}/types.gleam",
        include_str!("../../templates/dev_pipeline/support/types.gleam"),
    ),
    (
        "src/{{name}}/codecs_core.gleam",
        include_str!("../../templates/dev_pipeline/support/codecs_core.gleam"),
    ),
    (
        "src/{{name}}/codecs_flow.gleam",
        include_str!("../../templates/dev_pipeline/support/codecs_flow.gleam"),
    ),
    (
        "src/{{name}}/codecs_workflows.gleam",
        include_str!("../../templates/dev_pipeline/support/codecs_workflows.gleam"),
    ),
    (
        "src/{{name}}/io_convert.gleam",
        include_str!("../../templates/dev_pipeline/support/io_convert.gleam"),
    ),
    (
        "src/{{name}}/activities.gleam",
        include_str!("../../templates/dev_pipeline/support/activities.gleam"),
    ),
    (
        "src/{{name}}/locals.gleam",
        include_str!("../../templates/dev_pipeline/support/locals.gleam"),
    ),
    (
        "src/{{name}}/cli.gleam",
        include_str!("../../templates/dev_pipeline/support/cli.gleam"),
    ),
    (
        "src/{{name}}/errors.gleam",
        include_str!("../../templates/dev_pipeline/support/errors.gleam"),
    ),
    (
        "test/{{name}}_test.gleam",
        include_str!("../../templates/dev_pipeline/test/project_test.gleam"),
    ),
    (
        "test/{{name}}_test_ffi.erl",
        include_str!("../../templates/dev_pipeline/test/test_ffi.erl"),
    ),
    (
        "test/support/shims.gleam",
        include_str!("../../templates/dev_pipeline/test/shims.gleam"),
    ),
    (
        "README.md",
        include_str!("../../templates/dev_pipeline/README.md"),
    ),
];

/// Rust worker crate serving the dev-pipeline template's eight activities
/// (required: the pipeline is meaningless without a worker serving them).
const DEV_PIPELINE_WORKER_FILES: &[ManifestFile] = &[
    (
        "worker/Cargo.toml",
        include_str!("../../templates/dev_pipeline/worker/Cargo.toml.tmpl"),
    ),
    (
        "worker/src/main.rs",
        include_str!("../../templates/dev_pipeline/worker/main.rs"),
    ),
    (
        "worker/src/lib.rs",
        include_str!("../../templates/dev_pipeline/worker/lib.rs"),
    ),
    (
        "worker/src/types.rs",
        include_str!("../../templates/dev_pipeline/worker/types.rs"),
    ),
    (
        "worker/src/handlers.rs",
        include_str!("../../templates/dev_pipeline/worker/handlers.rs"),
    ),
    (
        "worker/src/shell.rs",
        include_str!("../../templates/dev_pipeline/worker/shell.rs"),
    ),
    (
        "worker/tests/wire_compat.rs",
        include_str!("../../templates/dev_pipeline/worker/tests/wire_compat.rs"),
    ),
    (
        "worker/tests/handlers_shims.rs",
        include_str!("../../templates/dev_pipeline/worker/tests/handlers_shims.rs"),
    ),
];

impl Template {
    /// The kebab-case template name used by `--template`, `--help`, and the
    /// JSON output.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Awl => "awl",
            Self::HelloWorld => "hello-world",
            Self::ApprovalFlow => "approval-flow",
            Self::Saga => "saga",
            Self::DevPipeline => "dev-pipeline",
            Self::Agent => "agent",
        }
    }

    /// Every project file this template emits, shared files first.
    #[must_use]
    pub fn files(self) -> Vec<ManifestFile> {
        let (shared, own): (&[ManifestFile], &[ManifestFile]) = match self {
            Self::Awl => (SHARED_AWL_FILES, AWL_FILES),
            Self::HelloWorld => (SHARED_FILES, HELLO_WORLD_FILES),
            Self::ApprovalFlow => (SHARED_FILES, APPROVAL_FLOW_FILES),
            Self::Saga => (SHARED_FILES, SAGA_FILES),
            // The dev pipeline carries its own gleam.toml (gleeunit
            // dev-dependency for the scaffolded test suite).
            Self::DevPipeline => (SHARED_FILES_WITHOUT_GLEAM_TOML, DEV_PIPELINE_FILES),
            Self::Agent => (SHARED_FILES, AGENT_FILES),
        };
        let mut files = shared.to_vec();
        files.extend_from_slice(own);
        files
    }

    /// The activities this template's workflows dispatch to a worker.
    #[must_use]
    pub fn activities(self) -> &'static [&'static str] {
        match self {
            Self::Awl | Self::HelloWorld | Self::ApprovalFlow => &[],
            Self::Saga => &["charge_payment", "refund_payment"],
            Self::Agent => AGENT_ACTIVITIES,
            Self::DevPipeline => &[
                "provision_workspace",
                "warm_build",
                "dev",
                "scoped_checks",
                "dev_resume",
                "full_checks",
                "request_review",
                "land",
            ],
        }
    }

    /// Additional files emitted with `--worker rust`. Empty for templates
    /// whose workflows dispatch no activities — there is nothing for a
    /// worker to serve, and `aion new` refuses rather than inventing one.
    #[must_use]
    pub fn worker_files(self) -> &'static [ManifestFile] {
        match self {
            Self::Awl | Self::HelloWorld | Self::ApprovalFlow => &[],
            Self::Saga => SAGA_WORKER_FILES,
            Self::Agent => AGENT_WORKER_FILES,
            Self::DevPipeline => DEV_PIPELINE_WORKER_FILES,
        }
    }

    /// The reason a worker-required template cannot be scaffolded without one,
    /// surfaced verbatim in the `aion new` refusal, and the single source of
    /// truth for whether `--worker` is mandatory (`Some` ⟺ required). A
    /// worker-required template's activities are all worker-served in a live
    /// deployment, so scaffolding it without the worker would emit a project
    /// that cannot run; `aion new` refuses instead. Returns `None` for
    /// templates that do not require a worker.
    #[must_use]
    pub fn worker_requirement_reason(self) -> Option<&'static str> {
        match self {
            Self::Awl | Self::HelloWorld | Self::ApprovalFlow | Self::Saga => None,
            Self::DevPipeline => Some(
                "all eight of its activities (provision, warm build, dev agent, checks, gate, \
                 review request, land) are served by the standalone worker crate in a live \
                 deployment",
            ),
            Self::Agent => Some(
                "its three agent steps (scout, act, verify) are served by the worker, where the \
                 agent driver lives — the scaffold bundles no runtime of its own",
            ),
        }
    }

    /// All templates, for manifest-completeness tests.
    #[cfg(test)]
    pub fn all() -> &'static [Self] {
        &[
            Self::Awl,
            Self::HelloWorld,
            Self::ApprovalFlow,
            Self::Saga,
            Self::DevPipeline,
            Self::Agent,
        ]
    }
}

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
