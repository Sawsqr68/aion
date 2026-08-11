"""Rewrite the agent-worker serve path for document-declared launches."""

import pathlib

p = pathlib.Path("crates/aion-cli/src/worker_agent/serve.rs")
s = p.read_text()

s = s.replace(
    """//! # The order the preparation happens in, and why
//!
//! 1. **The action surface**, from the document. It comes first because `--check` must
//!    work on a build that could not serve, and a document fault must never hide behind
//!    a missing-configuration complaint.
//! 2. **The harness selection**, over the adapters compiled into this binary — plus the
//!    refusal of any harness flag belonging to a harness other than the resolved one.
//!    Both are part of the `--check` report: which harness would drive the surface is
//!    exactly as load-bearing as the surface itself, and a flag that cannot take effect
//!    should be refused before anything runs rather than at the next deploy.
//! 3. **Every remaining setting** — the connection settings and the resolved harness's
//!    own configuration, resolved together so one refusal names every missing one.
//!
//! `--check` stops after (2), demanding no settings at all. That is the same line the
//! verb already drew for the connection settings: a report that connects to nothing must
//! not require the configuration of a connection it will not make.""",
    """//! # The order the preparation happens in, and why
//!
//! 1. **The action surface**, from the document. It comes first because `--check` must
//!    work on a build that could not serve, and a document fault must never hide behind
//!    a missing-configuration complaint.
//! 2. **The declared harness**, read from the same document's `harness` section and
//!    checked against the adapters compiled into this binary. It is part of the
//!    `--check` report: which harness would drive the surface is exactly as load-bearing
//!    as the surface itself, and a document naming a harness this build does not carry
//!    should be refused before anything runs rather than at the next deploy.
//! 3. **The connection settings** — where this worker dials, who it says it is, and
//!    where it sits. Those are the only settings that are still flags, because they are
//!    properties of this process on this box rather than of the queue.
//!
//! `--check` stops after (2), demanding no connection at all: a report that connects to
//! nothing must not require the configuration of a connection it will not make.""",
)

s = s.replace(
    """use super::args::{self, AgentArgs, ConnectionSettings};
use super::surface::{self, AdvertisedSurface};
use crate::harness::ResolvedHarness;
use crate::settings::Environment;""",
    """use super::args::{self, AgentArgs, ConnectionSettings};
use super::surface::{self, AdvertisedSurface};
use crate::harness::ResolvedHarness;
use crate::settings::Environment;
use crate::worker_surface;""",
)

s = s.replace(
    """/// The resolved plan for one served worker.
struct Plan {
    /// The typed action surface to advertise.
    surface: AdvertisedSurface,
    /// The connection settings to register with.
    connection: ConnectionSettings,
    /// The selected and configured harness, `None` when this build compiles none in.
    harness: Option<ResolvedHarness>,
}""",
    """/// The resolved plan for one served worker.
struct Plan {
    /// The typed action surface to advertise.
    surface: AdvertisedSurface,
    /// The connection settings to register with — the only half still supplied by flags.
    connection: ConnectionSettings,
    /// The launch the document declares: concurrency, the reconnect trio, the
    /// environment pass-through list, and the per-kind settings.
    launch: aion_awl::CompiledHarness,
    /// The declared and configured harness, `None` when this build compiles none in.
    harness: Option<ResolvedHarness>,
}""",
)

s = s.replace(
    """async fn prepare(args: &AgentArgs) -> Result<Prepared> {
    let environment = Environment::from_process();
    let document = args.awl.clone();
    let requested = args.requested_queue(&environment);
    let surface =
        tokio::task::spawn_blocking(move || surface::derive(&document, requested.as_deref()))
            .await
            .context("the action-surface derivation task did not complete")??;
    let harness = args.harness.select(&environment)?;
    if args.check {
        let mut stdout = std::io::stdout().lock();
        surface::print(&mut stdout, &surface, harness)?;
        return Ok(None);
    }
    let (connection, harness) = args::resolve(args, &environment, harness)?;
    Ok(Some(Plan {
        surface,
        connection,
        harness,
    }))
}""",
    """async fn prepare(args: &AgentArgs) -> Result<Prepared> {
    let environment = Environment::from_process();
    let document = args.document()?.to_path_buf();
    let requested = args.requested_queue(&environment);
    let derivation = document.clone();
    let surface =
        tokio::task::spawn_blocking(move || surface::derive(&derivation, requested.as_deref()))
            .await
            .context("the action-surface derivation task did not complete")??;
    let declared = worker_surface::compile_harness(&document, &surface.task_queue)?;
    let harness = match &declared {
        Some(declared) => crate::harness::resolve_declared(declared, &document)?,
        None => None,
    };
    if args.check {
        let mut stdout = std::io::stdout().lock();
        surface::print(
            &mut stdout,
            &surface,
            harness.as_ref().map(|resolved| resolved.kind),
        )?;
        return Ok(None);
    }
    let Some(launch) = declared else {
        return Err(no_harness_section(&surface, &document));
    };
    let connection = args::resolve(args, &environment)?;
    Ok(Some(Plan {
        surface,
        connection,
        launch,
        harness,
    }))
}

/// The refusal for a document that declares a queue but no launch for it.
///
/// Loud on purpose, and it names the whole shape rather than one missing field: an
/// operator meeting this has almost certainly arrived with the deleted flags in their
/// fingers, and the useful answer is where those settings live now — not that one of
/// them is absent.
fn no_harness_section(surface: &AdvertisedSurface, document: &std::path::Path) -> anyhow::Error {
    anyhow::anyhow!(
        "queue `{queue}` in {path} declares no `harness` section, so this document does not say \\
         how to launch an agent for it.\\n\\nAn agent worker's launch — the harness kind, the \\
         concurrency, the three reconnect settings, the agent's command, working directory, \\
         permission policy and shutdown grace, and the environment variables the agent \\
         subprocess is given — is declared in the document and nowhere else. There is no flag \\
         for any of it.\\n\\nAdd a `harness` section under `worker {queue}` and run `aion awl \\
         check {path}`: the checker names every setting the section is missing, scoped to the \\
         kind it declares.",
        queue = surface.task_queue,
        path = document.display()
    )
}""",
)

s = s.replace(
    """    let mut builder = aion_worker::WorkerConfig::builder()
        .endpoint(UNUSED_GRPC_ENDPOINT)
        .namespace(namespace)
        .task_queue(&surface.task_queue)
        .identity(&settings.identity)
        .max_concurrency(settings.concurrency)
        .reconnect_initial_backoff(settings.initial_backoff)
        .reconnect_max_backoff(settings.max_backoff)
        .reconnect_max_attempts(settings.max_attempts);""",
    """    let mut builder = aion_worker::WorkerConfig::builder()
        .endpoint(UNUSED_GRPC_ENDPOINT)
        .namespace(namespace)
        .task_queue(&surface.task_queue)
        .identity(&settings.identity)
        .max_concurrency(launch.concurrency)
        .reconnect_initial_backoff(launch.reconnect_initial_backoff)
        .reconnect_max_backoff(launch.reconnect_max_backoff)
        .reconnect_max_attempts(launch.reconnect_max_attempts);""",
)

s = s.replace(
    """fn worker_config(
    surface: &AdvertisedSurface,
    settings: &ConnectionSettings,
    namespace: &str,
) -> Result<aion_worker::WorkerConfig> {""",
    """fn worker_config(
    surface: &AdvertisedSurface,
    settings: &ConnectionSettings,
    launch: &aion_awl::CompiledHarness,
    namespace: &str,
) -> Result<aion_worker::WorkerConfig> {""",
)

s = s.replace(
    """    serve(&plan.surface, &plan.connection, &harness, namespace).await""",
    """    serve(
        &plan.surface,
        &plan.connection,
        &plan.launch,
        &harness,
        namespace,
    )
    .await""",
)

s = s.replace(
    """async fn serve(
    surface: &AdvertisedSurface,
    settings: &ConnectionSettings,
    harness: &ResolvedHarness,
    namespace: &str,
) -> Result<()> {""",
    """async fn serve(
    surface: &AdvertisedSurface,
    settings: &ConnectionSettings,
    launch: &aion_awl::CompiledHarness,
    harness: &ResolvedHarness,
    namespace: &str,
) -> Result<()> {""",
)

s = s.replace(
    """    let config = worker_config(surface, settings, namespace)?;""",
    """    let config = worker_config(surface, settings, launch, namespace)?;""",
)

s = s.replace(
    """    let timing = aion_worker::RedialTiming::new(settings.initial_backoff, settings.max_backoff);""",
    """    let timing = aion_worker::RedialTiming::new(
        launch.reconnect_initial_backoff,
        launch.reconnect_max_backoff,
    );""",
)

s = s.replace(
    """    Err(unservable(
        &plan.surface,
        &plan.connection,
        plan.harness.as_ref(),
        namespace,
    ))""",
    """    Err(unservable(
        &plan.surface,
        &plan.connection,
        plan.harness.as_ref(),
        namespace,
    ))""",
)
p.write_text(s)
print("launch wired:", "launch.concurrency" in s)
print("document():", "args.document()?" in s)
