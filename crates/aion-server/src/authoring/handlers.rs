//! Transport-agnostic server-side authoring handler.
//!
//! `compile_and_load` is the authoring loop in one call: authorize (reusing
//! the deploy guard — new code admission is gated exactly like a deploy),
//! refuse during drain, compile and type-check the submitted Gleam source
//! through [`aion_toolchain`] (which only spawns the external `gleam` binary),
//! and on success hot-load the resulting package into the running engine via
//! `engine.load_package`. A type error returns the gleam diagnostics inline.
//!
//! Mounted only when `[authoring].gleam_path` is configured; with it absent
//! the routes do not exist, the server deploys pre-built `.aion` files only,
//! and nothing here is ever reached (CN7).

use std::path::PathBuf;
use std::sync::Arc;

use aion::EngineError;
use aion_awl_package::AwlAssembleOptions;
use aion_package::{ExtractionLimits, Package, PackageBuilder};
use aion_proto::WireError;
use aion_toolchain::{CompileRequest, ToolchainError, compile_source, compile_source_for_entry};
use serde::{Deserialize, Serialize};

use super::error::AuthoringApiError;
use crate::config::{AUTHORING_GLEAM_PATH_EMPTY, AUTHORING_PROJECT_ROOT_REQUIRED};
use crate::{CallerIdentity, ServerState};

/// Request to compile, type-check, and hot-load submitted Gleam source.
///
/// Strict parsing (`deny_unknown_fields`, consistent with the server config
/// surfaces): an unrecognised field is a 400, never silently ignored, so a
/// typo in the submission body fails loudly instead of being dropped.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompileSourceRequest {
    /// The Gleam workflow source written verbatim into a fresh per-submission
    /// working copy of the server's configured authoring project template,
    /// into its single entry-module file before building. The toolchain never
    /// rewrites it.
    pub source: String,
}

/// Response for a successful compile-and-hot-load.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompileSourceResponse {
    /// The workflow type (the manifest entry module) that was loaded.
    pub workflow_type: String,
    /// The content hash of the loaded package version.
    pub content_hash: String,
    /// The deployed (content-hash-namespaced) entry module name.
    pub deployed_entry_module: String,
    /// The entry function spawned for this version.
    pub entry_function: String,
    /// True when this call registered the version; false on idempotent re-load.
    pub freshly_loaded: bool,
    /// True when this call re-pointed the type's route at the version.
    pub route_changed: bool,
}

/// Compiles, type-checks, and hot-loads submitted Gleam source.
///
/// # Errors
///
/// Returns [`AuthoringApiError::Wire`] for authorization denials and
/// misconfiguration, [`AuthoringApiError::Unavailable`] during drain or
/// engine shutdown, [`AuthoringApiError::TypeError`] (carrying the verbatim
/// gleam diagnostics) when the source does not compile, and
/// [`AuthoringApiError::Wire`] for spawn, packaging, or load failures.
pub async fn compile_and_load(
    state: &ServerState,
    caller: &CallerIdentity,
    transport: &'static str,
    request: CompileSourceRequest,
) -> Result<CompileSourceResponse, AuthoringApiError> {
    compile_and_load_with_options(
        state,
        caller,
        transport,
        request,
        AwlAssembleOptions::default(),
    )
    .await
}

/// Compiles and hot-loads submitted Gleam source while applying AWL-native
/// manifest options after the frozen project compiler has packaged it.
///
/// # Errors
///
/// Returns the same failures as [`compile_and_load`], plus a package error if
/// applying the AWL manifest timeout cannot round-trip the built archive.
pub async fn compile_and_load_with_options(
    state: &ServerState,
    caller: &CallerIdentity,
    transport: &'static str,
    request: CompileSourceRequest,
    options: AwlAssembleOptions,
) -> Result<CompileSourceResponse, AuthoringApiError> {
    compile_and_load_inner(state, caller, transport, request, options, None).await
}

/// Compiles and hot-loads an emitted AWL document under the workflow type
/// declared by that document rather than the authoring template's frozen entry.
///
/// `workflow_type` must come from the parsed document header. It becomes the
/// staged Gleam module path, package manifest entry module, engine workflow
/// type, and sole routing target for the load.
///
/// # Errors
///
/// Returns the same failures as [`compile_and_load_with_options`], plus an
/// invalid-project error if `workflow_type` cannot name a Gleam module.
pub async fn compile_and_load_document(
    state: &ServerState,
    caller: &CallerIdentity,
    transport: &'static str,
    request: CompileSourceRequest,
    workflow_type: String,
    options: AwlAssembleOptions,
) -> Result<CompileSourceResponse, AuthoringApiError> {
    compile_and_load_inner(
        state,
        caller,
        transport,
        request,
        options,
        Some(workflow_type),
    )
    .await
}

async fn compile_and_load_inner(
    state: &ServerState,
    caller: &CallerIdentity,
    transport: &'static str,
    request: CompileSourceRequest,
    options: AwlAssembleOptions,
    workflow_type: Option<String>,
) -> Result<CompileSourceResponse, AuthoringApiError> {
    admit_mutation(state, caller, transport, "authoring.compile")?;
    let (gleam_path, template_root) = authoring_paths(state)?;
    let expected_workflow_type = workflow_type.clone();
    let mut compiled =
        run_compile(gleam_path, template_root, request.source, workflow_type).await?;
    if let Some(expected) = expected_workflow_type {
        validate_document_identity(&compiled.package, &expected)?;
    }
    compiled.package = package_with_options(compiled.package, options)?;
    load_authorized_package(
        state,
        caller,
        transport,
        "authoring.compile",
        compiled.package,
    )
    .await
}

/// Hot-loads a package after the caller has passed [`admit_mutation`].
///
/// Authorization is stable in the request's [`CallerIdentity`] and is not
/// repeated. Drain state is mutable, so it is re-checked immediately before
/// the engine load to close a drain transition during direct compilation.
pub(crate) async fn load_admitted_package(
    state: &ServerState,
    caller: &CallerIdentity,
    transport: &'static str,
    operation: &'static str,
    package: Package,
) -> Result<CompileSourceResponse, AuthoringApiError> {
    ensure_not_draining(state)?;
    load_authorized_package(state, caller, transport, operation, package).await
}

/// Verifies document-owned package identity before any engine load can mutate
/// the catalog or routing table.
pub(crate) fn validate_document_identity(
    package: &Package,
    expected: &str,
) -> Result<(), AuthoringApiError> {
    let actual = &package.manifest().entry_module;
    if actual == expected {
        return Ok(());
    }
    Err(AuthoringApiError::Wire(
        WireError::backend(format!(
            "document compile returned manifest entry module `{actual}` instead of `{expected}`"
        ))
        .with_error_type("Toolchain"),
    ))
}

async fn load_authorized_package(
    state: &ServerState,
    caller: &CallerIdentity,
    transport: &'static str,
    operation: &'static str,
    package: Package,
) -> Result<CompileSourceResponse, AuthoringApiError> {
    let engine = engine_handle(state)?;
    match engine.load_package(package).await {
        Ok(outcome) => {
            let workflow_type = outcome.record.workflow_type().to_owned();
            let content_hash = outcome.record.version().to_string();
            tracing::info!(
                operation,
                subject = caller.subject(),
                grant_source = caller.grant_source().label(),
                transport,
                workflow_type = %workflow_type,
                content_hash = %content_hash,
                outcome = "loaded",
                freshly_loaded = outcome.freshly_loaded,
                route_changed = outcome.route_changed,
                "authoring compile-and-load applied"
            );
            Ok(CompileSourceResponse {
                workflow_type,
                content_hash,
                deployed_entry_module: outcome.record.deployed_entry_module().to_owned(),
                entry_function: outcome.record.entry_function().to_owned(),
                freshly_loaded: outcome.freshly_loaded,
                route_changed: outcome.route_changed,
            })
        }
        Err(error) => Err(map_load_failure(caller, transport, operation, error)),
    }
}

pub(crate) fn package_with_options(
    package: Package,
    options: AwlAssembleOptions,
) -> Result<Package, AuthoringApiError> {
    let Some(timeout) = options.timeout else {
        return Ok(package);
    };
    let mut manifest = package.manifest().clone();
    manifest.timeout = Some(timeout);
    let contract = package.contract().map_err(|error| {
        AuthoringApiError::Wire(WireError::invalid_input(format!(
            "AWL manifest options require a `.v4` package contract: {error}"
        )))
    })?;
    let source = package
        .source()
        .iter()
        .map(|(name, bytes)| (name.clone(), bytes.clone()));
    let bytes = PackageBuilder::with_source(manifest, package.beams().clone(), source)
        .with_contract(contract.clone())
        .write_to_bytes()
        .map_err(|error| package_options_error(&error))?;
    Package::load_from_bytes(bytes, ExtractionLimits::unbounded())
        .map_err(|error| package_options_error(&error))
}

fn package_options_error(error: &aion_package::PackageError) -> AuthoringApiError {
    AuthoringApiError::Wire(
        WireError::invalid_input(format!(
            "AWL manifest options could not be applied: {error}"
        ))
        .with_error_type("Package"),
    )
}

/// Authorization plus drain gate, reusing the deploy guard: hot-loading new
/// code is new-work admission, gated exactly like a deploy mutation (ADR-002:
/// no second authorization mechanism).
pub(crate) fn admit_mutation(
    state: &ServerState,
    caller: &CallerIdentity,
    transport: &'static str,
    operation: &'static str,
) -> Result<(), AuthoringApiError> {
    let guard = state.deploy_guard();
    if let Err(error) = guard.authorize(caller) {
        let wire = error.to_wire_error();
        tracing::warn!(
            operation,
            subject = caller.subject(),
            grant_source = caller.grant_source().label(),
            transport,
            reason = %wire.message,
            "authoring operation denied"
        );
        return Err(AuthoringApiError::Wire(wire));
    }
    ensure_not_draining(state)
}

fn ensure_not_draining(state: &ServerState) -> Result<(), AuthoringApiError> {
    if state.drain_state().is_draining() {
        return Err(AuthoringApiError::Unavailable(WireError::backend(
            "server is draining and not accepting authoring submissions",
        )));
    }
    Ok(())
}

/// Resolves the operator-configured authoring paths, failing loudly if the
/// surface was mounted without them (a wiring bug, never a caller error).
fn authoring_paths(state: &ServerState) -> Result<(PathBuf, PathBuf), AuthoringApiError> {
    let authoring = &state.runtime_config().authoring;
    let Some(gleam_path) = authoring.gleam_path.clone() else {
        return Err(AuthoringApiError::Wire(WireError::backend(
            AUTHORING_GLEAM_PATH_EMPTY,
        )));
    };
    let Some(project_root) = authoring.project_root.clone() else {
        return Err(AuthoringApiError::Wire(WireError::backend(
            AUTHORING_PROJECT_ROOT_REQUIRED,
        )));
    };
    Ok((gleam_path, project_root))
}

/// Runs the synchronous, multi-second compile-and-package off the async
/// runtime in a blocking task, then maps the toolchain outcome onto the
/// authoring wire classes.
///
/// The toolchain stages its own per-submission working copy of the read-only
/// `template_root`, so concurrent blocking tasks never collide on the template.
async fn run_compile(
    gleam_path: PathBuf,
    template_root: PathBuf,
    source: String,
    workflow_type: Option<String>,
) -> Result<aion_toolchain::CompiledWorkflow, AuthoringApiError> {
    let join = tokio::task::spawn_blocking(move || {
        let request = CompileRequest {
            template_root: &template_root,
            gleam_path: &gleam_path,
            source: &source,
        };
        workflow_type.map_or_else(
            || compile_source(&request),
            |entry_module| compile_source_for_entry(&request, &entry_module),
        )
    })
    .await;
    match join {
        Ok(Ok(compiled)) => Ok(compiled),
        Ok(Err(error)) => Err(map_toolchain_error(error)),
        Err(join_error) => Err(AuthoringApiError::Wire(WireError::backend(format!(
            "authoring compile task failed to run: {join_error}"
        )))),
    }
}

/// Maps a toolchain failure onto the authoring wire classes.
///
/// A type error is the inline 400; a dependency-layer failure is the retryable
/// 503, because the source was never compiled and nothing about it is in
/// question; a spawn failure or packaging fault is a backend/invalid-input wire
/// error naming the cause.
fn map_toolchain_error(error: ToolchainError) -> AuthoringApiError {
    match error {
        ToolchainError::TypeCheck { diagnostics } => AuthoringApiError::TypeError(diagnostics),
        ToolchainError::DependencyLayer { .. } => {
            // 🔴 NOT a 400, and not a type error (#125). `gleam build` resolves
            // and fetches dependencies before it compiles anything, so a
            // registry outage exits non-zero having never reached the compiler.
            // Routing that to `TypeError` told the author two untrue things at
            // once: that type-checking produced these diagnostics, and — by the
            // 400 — that their submission was the thing at fault. Nothing was
            // established about the source. This is the server's environment
            // failing and it is transient, so it is the retryable 503.
            AuthoringApiError::Unavailable(
                WireError::backend(error.to_string()).with_error_type("GleamRegistry"),
            )
        }
        ToolchainError::GleamSpawn { .. } | ToolchainError::Io { .. } => {
            // Operator-side faults (binary unspawnable, project filesystem
            // unwritable): backend errors, not caller-correctable input.
            AuthoringApiError::Wire(
                WireError::backend(error.to_string()).with_error_type("Toolchain"),
            )
        }
        ToolchainError::Packaging(_) | ToolchainError::InvalidProject { .. } => {
            // The source compiled but the project could not be assembled, or
            // the project layout is unusable: a configuration/input problem.
            AuthoringApiError::Wire(
                WireError::invalid_input(error.to_string()).with_error_type("Toolchain"),
            )
        }
    }
}

/// Maps an engine load failure onto the authoring wire classes, mirroring the
/// deploy load mapping.
fn map_load_failure(
    caller: &CallerIdentity,
    transport: &'static str,
    operation: &'static str,
    error: EngineError,
) -> AuthoringApiError {
    let mapped = match error {
        EngineError::ShuttingDown => AuthoringApiError::Unavailable(
            WireError::backend(error.to_string()).with_error_type("ShuttingDown"),
        ),
        EngineError::Load { .. } => AuthoringApiError::Wire(
            WireError::invalid_input(error.to_string()).with_error_type("Load"),
        ),
        EngineError::Package(_) => AuthoringApiError::Wire(
            WireError::invalid_input(error.to_string()).with_error_type("Package"),
        ),
        other => AuthoringApiError::Wire(crate::ServerError::from(other).to_wire_error()),
    };
    tracing::info!(
        operation,
        subject = caller.subject(),
        grant_source = caller.grant_source().label(),
        transport,
        outcome = mapped.outcome(),
        "authoring compile-and-load refused at hot-load"
    );
    mapped
}

/// Borrows the engine handle for the authorized authoring operation, reusing
/// the deploy guard's engine accessor.
fn engine_handle(state: &ServerState) -> Result<Arc<aion::Engine>, AuthoringApiError> {
    state
        .deploy_guard()
        .engine()
        .map(Arc::clone)
        .map_err(|error| AuthoringApiError::Wire(error.to_wire_error()))
}

#[cfg(test)]
mod tests {
    use aion_toolchain::error::ToolchainError;

    use super::{AuthoringApiError, map_toolchain_error};

    /// The exact `gleam` output that opened #125, from run 3 of the 2026-08-01
    /// workspace battery.
    const HEX_OUTAGE: &str = "  Resolving versions\nerror: HTTP error\n\nA HTTP request \
         failed.\n\n    error sending request for url \
         (https://hex.pm/api/packages/gleam_stdlib/releases/1.0.3)\n";

    /// 🔴 THE INVERTED CONTROL FOR #125. Before the fix, a registry outage
    /// arrived here as `ToolchainError::TypeCheck` and left as
    /// `AuthoringApiError::TypeError` — an inline **400** telling the author
    /// their submission was at fault, for a build that never reached the
    /// compiler. Two untrue statements in one response: that type-checking
    /// produced these diagnostics, and that the caller's input was wrong.
    #[test]
    fn a_registry_outage_is_never_reported_to_the_author_as_a_type_error() {
        let mapped = map_toolchain_error(ToolchainError::DependencyLayer {
            diagnostics: HEX_OUTAGE.to_owned(),
        });
        assert!(
            !matches!(mapped, AuthoringApiError::TypeError(_)),
            "a registry outage must never be an inline 400 type error: the source was never \
             compiled, so nothing about it has been established"
        );
        assert!(
            matches!(mapped, AuthoringApiError::Unavailable(_)),
            "a registry outage is a transient environment failure — the retryable 503"
        );
        assert_eq!(mapped.outcome(), "unavailable");
    }

    /// The positive arm, under the same mapping. Without it, the assertion
    /// above would still pass if `map_toolchain_error` stopped producing
    /// `TypeError` for anything at all — a vacuous absence check.
    #[test]
    fn a_real_type_error_is_still_the_inline_400() {
        let mapped = map_toolchain_error(ToolchainError::TypeCheck {
            diagnostics: "error: Type mismatch\n  expected Int, got String".to_owned(),
        });
        assert!(
            matches!(
                &mapped,
                AuthoringApiError::TypeError(diagnostics) if diagnostics.contains("Type mismatch")
            ),
            "a genuine type error must stay the inline 400 carrying its diagnostics, got {mapped:?}"
        );
    }

    /// The registry diagnostics must survive the mapping: an operator debugging
    /// a 503 needs `gleam`'s own words, and a class label alone would send them
    /// hunting.
    #[test]
    fn the_registry_failure_carries_gleams_own_output_to_the_operator() {
        let mapped = map_toolchain_error(ToolchainError::DependencyLayer {
            diagnostics: HEX_OUTAGE.to_owned(),
        });
        // Three separate assertions rather than one compound: each failure mode
        // reports distinctly, so a red says WHICH guarantee broke.
        assert!(
            matches!(&mapped, AuthoringApiError::Unavailable(_)),
            "a registry outage must be the retryable class, got {mapped:?}"
        );
        assert!(
            matches!(&mapped, AuthoringApiError::Unavailable(wire)
                if wire.message.contains("hex.pm")),
            "the registry endpoint must survive into the wire error: {mapped:?}"
        );
        assert!(
            matches!(&mapped, AuthoringApiError::Unavailable(wire)
                if wire.message.contains("never compiled")),
            "the message must say the source was never compiled: {mapped:?}"
        );
    }
}
