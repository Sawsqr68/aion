//! Composing the selected harness and serving it on the derived queue.
//!
//! # Why liminal, and not the global `--endpoint`
//!
//! The agent-harness seam exists only on the LIMINAL worker transport. A composed
//! harness reaches a served worker as an `AgentHarnessConfig` threaded through
//! `aion_worker::serve_with_redial`; the gRPC `Worker`/`WorkerBuilder` path (what
//! `aion worker shell` drives via `--endpoint`) has no parameter for one, and the
//! server's own worker bridge records that its gRPC arm carries no agent binding
//! *because* the harness seam is liminal-only. So this verb dials liminal
//! candidates, and the gRPC endpoint plays no part in it.
//!
//! # The order the preparation happens in, and why
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
//! nothing must not require the configuration of a connection it will not make.
//!
//! # Why the serve loop gets its own thread
//!
//! `serve_with_redial` is SYNCHRONOUS: the liminal push client is thread-based, and
//! the function builds its own current-thread runtime to drive each connection's
//! serve loop. Calling it on a thread already inside the CLI's runtime would nest
//! one runtime inside another. It runs on a dedicated thread — the shape every
//! example worker uses — and this task awaits its outcome.
//!
//! # Build configurations
//!
//! Serving needs both a compiled-in harness (`norn` and/or `acp`) and the transport
//! that can carry one (`liminal-transport`). A build missing either derives and reports
//! the surface exactly as a complete build does, then refuses to serve — naming which
//! piece is absent, and naming what it would otherwise have served, so the operator can
//! see the request was understood and only the build is wrong.

use anyhow::{Context, Result};

use super::args::{self, AgentArgs, ConnectionSettings};
use super::surface::{self, AdvertisedSurface};
use crate::harness::ResolvedHarness;
use crate::settings::Environment;
use crate::worker_surface;

/// Everything a serve needs, resolved — or `None` when `--check` has already reported
/// and there is nothing left to do.
type Prepared = Option<Plan>;

/// The resolved plan for one served worker.
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
}

/// Derives the action surface, resolves the harness, honours `--check`, and resolves
/// every remaining setting.
///
/// See the module documentation for why the steps happen in this order.
///
/// The derivation runs on a blocking thread because compiling an `.awl` document is
/// real CPU work — parse, typecheck, schema derivation, lowering — and belongs off
/// the async runtime's workers.
///
/// # Errors
///
/// Returns an error when the surface cannot be derived, when the derivation task
/// does not complete, when the harness selection is ambiguous or names a harness this
/// build does not carry, when a harness flag configures a harness that is not the
/// resolved one, or when a setting has no source.
async fn prepare(args: &AgentArgs) -> Result<Prepared> {
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
pub(super) fn no_harness_section(surface: &AdvertisedSurface, document: &std::path::Path) -> anyhow::Error {
    anyhow::anyhow!(
        "queue `{queue}` in {path} declares no `harness` section, so this document does not say \
         how to launch an agent for it.\n\nAn agent worker's launch — the harness kind, the \
         concurrency, the three reconnect settings, the agent's command, working directory, \
         permission policy and shutdown grace, and the environment variables the agent \
         subprocess is given — is declared in the document and nowhere else. There is no flag \
         for any of it.\n\nAdd a `harness` section under `worker {queue}` and run `aion awl \
         check {path}`: the checker names every setting the section is missing, scoped to the \
         kind it declares.",
        queue = surface.task_queue,
        path = document.display()
    )
}

/// Serves the selected agent harness on the queue `args.awl` declares.
///
/// # Errors
///
/// Returns an error when the surface cannot be derived, when the harness cannot be
/// resolved, when a setting has no source, when the composed harness would advertise
/// something other than the document's surface, or when the serve loop ends for any
/// reason other than a shutdown request.
#[cfg(all(any(feature = "norn", feature = "acp"), feature = "liminal-transport"))]
pub(crate) async fn run(args: &AgentArgs, namespace: &str) -> Result<()> {
    let Some(plan) = prepare(args).await? else {
        return Ok(());
    };
    // A build that compiles a harness in can still be handed `harness: None` — that is
    // the shape of a build with NO adapter, and it cannot happen here, because the
    // selection derives or refuses over a non-empty catalogue. Reported rather than
    // unwrapped so the impossible case names itself instead of panicking.
    let Some(harness) = plan.harness else {
        anyhow::bail!(
            "this `aion` build compiles an agent harness in, but none was resolved for queue \
             `{}`; the harness selection must resolve before a worker can serve",
            plan.surface.task_queue
        );
    };
    serve(
        &plan.surface,
        &plan.connection,
        &plan.launch,
        &harness,
        namespace,
    )
    .await
}

/// Derives and reports the surface exactly as a complete build does, then refuses
/// to serve it, naming the missing piece of the build.
///
/// # Errors
///
/// Returns an error when the surface cannot be derived or a setting has no source, and
/// always returns one when there is a surface to serve: this build cannot serve an
/// agent worker.
#[cfg(not(all(any(feature = "norn", feature = "acp"), feature = "liminal-transport")))]
pub(crate) async fn run(args: &AgentArgs, namespace: &str) -> Result<()> {
    let Some(plan) = prepare(args).await? else {
        return Ok(());
    };
    Err(unservable(
        &plan.surface,
        &plan.connection,
        plan.harness.as_ref(),
        namespace,
    ))
}

/// `WorkerConfigBuilder::build` requires a gRPC `endpoint`, and the liminal
/// transport never reads it: the address a connection dials is the liminal candidate
/// the redial driver hands `LiminalActivityWorker::connect_advertising`, not this
/// field. Accepting a flag for a value that could not affect anything would be worse
/// than naming the gap, so the gap is named in the value itself — where any
/// diagnostic that prints the config repeats it.
#[cfg(all(any(feature = "norn", feature = "acp"), feature = "liminal-transport"))]
const UNUSED_GRPC_ENDPOINT: &str =
    "unused: an agent worker dials liminal candidates, not a gRPC endpoint";

/// Composes the configured harness over the document's surface and checks that what
/// would actually be REGISTERED is exactly that surface.
///
/// The set the server sees is the config's UNION of its activity types and its
/// descriptor names. Checking it here closes the gap between what was derived and
/// what gets advertised: a worker registered for a type it will not route, or routing
/// one it never announced, is caught before it dials instead of by a dispatch that
/// finds no owner.
///
/// The advertised intervention capabilities come from the harness that was actually
/// resolved, not from the build: a binary carrying two adapters would otherwise
/// advertise the wrong controls for whichever one it is serving.
#[cfg(all(any(feature = "norn", feature = "acp"), feature = "liminal-transport"))]
fn compose(
    surface: &AdvertisedSurface,
    harness: &ResolvedHarness,
) -> Result<aion_worker::AgentHarnessConfig> {
    let advertised = surface.names();
    let agent = aion_worker::AgentHarnessConfig::new(
        crate::harness::compose::build(&harness.config),
        advertised.iter().cloned(),
        crate::harness::registration_capabilities(Some(harness.kind)),
    )
    .with_activity_descriptors(surface.descriptors.clone());
    if agent.agent_activity_types() != &advertised {
        anyhow::bail!(
            "the composed harness would advertise {:?} but queue `{}` requires {:?}; \
             refusing to register a surface that does not match the document",
            agent.agent_activity_types(),
            surface.task_queue,
            advertised
        );
    }
    Ok(agent)
}

/// Builds the worker's registration config from the derived queue and the resolved
/// settings.
///
/// `node` is the one setting left to the SDK when the operator supplies none: the
/// hostname default is the library's own documented behaviour, not a value invented
/// here.
#[cfg(all(any(feature = "norn", feature = "acp"), feature = "liminal-transport"))]
fn worker_config(
    surface: &AdvertisedSurface,
    settings: &ConnectionSettings,
    launch: &aion_awl::CompiledHarness,
    namespace: &str,
) -> Result<aion_worker::WorkerConfig> {
    let mut builder = aion_worker::WorkerConfig::builder()
        .endpoint(UNUSED_GRPC_ENDPOINT)
        .namespace(namespace)
        .task_queue(&surface.task_queue)
        .identity(&settings.identity)
        .max_concurrency(launch.concurrency)
        .reconnect_initial_backoff(launch.reconnect_initial_backoff)
        .reconnect_max_backoff(launch.reconnect_max_backoff)
        .reconnect_max_attempts(launch.reconnect_max_attempts);
    if let Some(node) = &settings.node {
        builder = builder.node(node.clone());
    }
    builder.build().map_err(Into::into)
}

/// Composes the harness, registers the derived surface, and serves until a shutdown
/// signal arrives.
#[cfg(all(any(feature = "norn", feature = "acp"), feature = "liminal-transport"))]
async fn serve(
    surface: &AdvertisedSurface,
    settings: &ConnectionSettings,
    launch: &aion_awl::CompiledHarness,
    harness: &ResolvedHarness,
    namespace: &str,
) -> Result<()> {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use anyhow::bail;

    // Install the product's own tracing subscriber (same `AION_LOG`/`RUST_LOG`
    // policy the server uses) BEFORE anything can log. The worker SDK's redial
    // driver reports a refused dial — a contract mismatch, a wrong address, a closed
    // port — through tracing, and that is the one signal telling an operator why a
    // worker that looks asleep is actually being turned away. With no subscriber
    // installed it would be dropped on the floor.
    aion_server::observability::tracing::init()?;

    let advertised = surface.names();
    let agent = compose(surface, harness)?;
    let config = worker_config(surface, settings, launch, namespace)?;
    // The harness owns every action on this queue, so the typed registry is EMPTY:
    // an agent activity is executed by driving the harness, never by a registered
    // handler. The advertisement is the only thing that makes the worker selectable.
    let registry = Arc::new(aion_worker::ActivityRegistry::new());

    let queue = surface.task_queue.clone();
    let ready_queue = queue.clone();
    let served = advertised.iter().cloned().collect::<Vec<_>>().join(", ");
    let candidates = settings.addresses.join(", ");
    let addresses = settings.addresses.clone();
    let timing = aion_worker::RedialTiming::new(
        launch.reconnect_initial_backoff,
        launch.reconnect_max_backoff,
    );
    let stop = Arc::new(AtomicBool::new(false));
    let serve_stop = Arc::clone(&stop);
    let (finished, ended) = tokio::sync::oneshot::channel();
    let thread = std::thread::Builder::new()
        .name("aion-worker-agent".to_owned())
        .spawn(move || {
            let outcome = aion_worker::serve_with_redial(
                addresses,
                &config,
                &registry,
                timing,
                serve_stop.as_ref(),
                Some(&agent),
                || {
                    tracing::info!(
                        task_queue = %ready_queue,
                        actions = %served,
                        "agent worker registered; serving through the composed agent harness"
                    );
                },
            );
            if let Err(unreported) = finished.send(outcome) {
                // The awaiting task is gone (the process is already unwinding), so
                // there is nobody left to hand this to. Log the whole outcome rather
                // than let a serve-loop result vanish because its reader left first.
                tracing::error!(
                    outcome = ?unreported,
                    "agent worker serve loop finished after its reporter left, so its \
                     outcome could not be returned"
                );
            }
        })
        .context("failed to spawn the agent worker serve thread")?;

    tracing::info!(
        candidates = %candidates,
        task_queue = %queue,
        identity = %settings.identity,
        namespace = %namespace,
        harness = harness.kind.name(),
        "agent worker dialing liminal candidates"
    );
    // A shutdown signal flips the shared stop flag; the serve loop checks it between
    // pushes and inside each connection, so a drain is prompt even on a quiet link.
    let signal_stop = Arc::clone(&stop);
    let signals = tokio::spawn(async move {
        match shutdown_signal().await {
            Ok(()) => {
                tracing::info!(
                    "shutdown signal received; finishing in-flight agent runs before exit"
                );
                signal_stop.store(true, Ordering::SeqCst);
            }
            // A worker that cannot install a signal listener still serves; it just
            // cannot be drained gracefully, which the operator needs to know.
            Err(error) => tracing::error!(
                %error,
                "cannot listen for a shutdown signal, so a termination will not drain"
            ),
        }
    });
    let reported = ended
        .await
        .context("the agent worker serve thread ended without reporting an outcome");
    signals.abort();
    let joined = thread.join();
    let outcome = reported?;
    if joined.is_err() {
        bail!("the agent worker serve thread terminated unexpectedly");
    }
    match outcome {
        Ok(()) if stop.load(Ordering::SeqCst) => Ok(()),
        // `serve_with_redial` returns cleanly only when the stop flag is set, so a
        // clean return without one is a loop that gave up — never a successful run.
        Ok(()) => bail!(
            "the agent worker serve loop for queue `{queue}` ended without a shutdown request"
        ),
        Err(error) => Err(anyhow::Error::new(error)
            .context(format!("agent worker on queue `{queue}` stopped serving"))),
    }
}

/// Awaits SIGTERM or SIGINT (Ctrl-C off Unix), mirroring the server's own shutdown
/// listener so a worker and a server drain on the same signals.
#[cfg(all(any(feature = "norn", feature = "acp"), feature = "liminal-transport"))]
async fn shutdown_signal() -> Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut terminate =
            signal(SignalKind::terminate()).context("failed to listen for SIGTERM")?;
        let mut interrupt =
            signal(SignalKind::interrupt()).context("failed to listen for SIGINT")?;
        tokio::select! {
            _ = terminate.recv() => Ok(()),
            _ = interrupt.recv() => Ok(()),
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .context("failed to listen for a shutdown signal")
    }
}

/// The refusal for a build with NO agent harness compiled in, naming exactly what it
/// would otherwise have served.
#[cfg(not(any(feature = "norn", feature = "acp")))]
fn unservable(
    surface: &AdvertisedSurface,
    settings: &ConnectionSettings,
    harness: Option<&ResolvedHarness>,
    namespace: &str,
) -> anyhow::Error {
    anyhow::anyhow!(
        "this `aion` build has NO agent harness compiled in (--no-default-features without \
         `norn`/`acp`), so it cannot serve queue `{}`{} ({}) as `{}` in namespace `{namespace}` \
         on {}. Rebuild with the default features, or with `--features norn` / \
         `--features acp`.",
        surface.task_queue,
        describe_harness(harness),
        describe_actions(surface),
        settings.identity,
        settings.addresses.join(", ")
    )
}

/// The refusal for a build that composes a harness but carries no transport able to
/// carry one — the agent-harness seam exists only on the liminal worker transport.
#[cfg(all(
    any(feature = "norn", feature = "acp"),
    not(feature = "liminal-transport")
))]
fn unservable(
    surface: &AdvertisedSurface,
    settings: &ConnectionSettings,
    harness: Option<&ResolvedHarness>,
    namespace: &str,
) -> anyhow::Error {
    anyhow::anyhow!(
        "this `aion` build composes an agent harness but has NO liminal worker transport \
         compiled in, and the agent-harness seam exists only on that transport — so queue `{}`{} \
         ({}) cannot be served as `{}` in namespace `{namespace}` on {}. Rebuild with the \
         default features, or add `--features liminal-transport`.",
        surface.task_queue,
        describe_harness(harness),
        describe_actions(surface),
        settings.identity,
        settings.addresses.join(", ")
    )
}

/// The harness clause a refusal carries: which harness the invocation resolved and
/// configured, when it resolved one.
///
/// Empty in a build with NO adapter compiled in, where [`ResolvedHarness`] is uninhabited
/// and there is nothing to name. Non-empty in a build that composes an adapter but
/// carries no transport able to drive it — which is the difference between "this build
/// cannot serve an agent worker" and "this build cannot serve THAT harness", and the
/// second is what tells an operator their configuration was understood before the build
/// was blamed.
#[cfg(not(all(any(feature = "norn", feature = "acp"), feature = "liminal-transport")))]
fn describe_harness(harness: Option<&ResolvedHarness>) -> String {
    harness.map_or_else(String::new, |resolved| {
        format!(" with the `{}` harness", resolved.kind.name())
    })
}

/// The action surface a refusal names, so an operator sees the work that was
/// understood and not merely the missing feature.
#[cfg(not(all(any(feature = "norn", feature = "acp"), feature = "liminal-transport")))]
fn describe_actions(surface: &AdvertisedSurface) -> String {
    format!(
        "{} action(s): {}",
        surface.descriptors.len(),
        surface.names().into_iter().collect::<Vec<_>>().join(", ")
    )
}
