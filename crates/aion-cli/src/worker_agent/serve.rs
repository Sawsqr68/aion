//! Composing the harness and serving it on the derived queue.
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
//! Serving needs both a compiled-in harness (`norn`) and the transport that can
//! carry one (`liminal-transport`). A build missing either derives and reports the
//! surface exactly as a complete build does, then refuses to serve — naming which
//! piece is absent, and naming what it would otherwise have served, so the operator
//! can see the request was understood and only the build is wrong.

use anyhow::{Context, Result};

use super::args::{self, AgentArgs, Environment, Settings};
use super::surface::{self, AdvertisedSurface};

/// The surface to serve and the settings to serve it with, or `None` when `--check`
/// has already reported and there is nothing left to do.
type Prepared = Option<(AdvertisedSurface, Settings)>;

/// Derives the action surface, honours `--check`, and resolves every connection
/// setting.
///
/// The surface comes FIRST, before any setting is demanded: `--check` must work on a
/// build that could not serve, and a document fault must never hide behind a
/// missing-configuration complaint.
///
/// The derivation runs on a blocking thread because compiling an `.awl` document is
/// real CPU work — parse, typecheck, schema derivation, lowering — and belongs off
/// the async runtime's workers.
///
/// # Errors
///
/// Returns an error when the surface cannot be derived, when the derivation task
/// does not complete, or when a connection setting has no source.
async fn prepare(args: &AgentArgs) -> Result<Prepared> {
    let environment = Environment::from_process();
    let document = args.awl.clone();
    let requested = args.requested_queue(&environment);
    let surface =
        tokio::task::spawn_blocking(move || surface::derive(&document, requested.as_deref()))
            .await
            .context("the action-surface derivation task did not complete")??;
    if args.check {
        let mut stdout = std::io::stdout().lock();
        surface::print(&mut stdout, &surface)?;
        return Ok(None);
    }
    let settings = args::resolve(args, &environment)?;
    Ok(Some((surface, settings)))
}

/// Serves the compiled-in agent harness on the queue `args.awl` declares.
///
/// # Errors
///
/// Returns an error when the surface cannot be derived, when a connection setting
/// has no source, when the composed harness would advertise something other than
/// the document's surface, or when the serve loop ends for any reason other than a
/// shutdown request.
#[cfg(all(feature = "norn", feature = "liminal-transport"))]
pub(crate) async fn run(args: &AgentArgs, namespace: &str) -> Result<()> {
    let Some((surface, settings)) = prepare(args).await? else {
        return Ok(());
    };
    serve(&surface, &settings, namespace).await
}

/// Derives and reports the surface exactly as a complete build does, then refuses
/// to serve it, naming the missing piece of the build.
///
/// # Errors
///
/// Returns an error when the surface cannot be derived or a connection setting has
/// no source, and always returns one when there is a surface to serve: this build
/// cannot serve an agent worker.
#[cfg(not(all(feature = "norn", feature = "liminal-transport")))]
pub(crate) async fn run(args: &AgentArgs, namespace: &str) -> Result<()> {
    let Some((surface, settings)) = prepare(args).await? else {
        return Ok(());
    };
    Err(unservable(&surface, &settings, namespace))
}

/// `WorkerConfigBuilder::build` requires a gRPC `endpoint`, and the liminal
/// transport never reads it: the address a connection dials is the liminal candidate
/// the redial driver hands `LiminalActivityWorker::connect_advertising`, not this
/// field. Accepting a flag for a value that could not affect anything would be worse
/// than naming the gap, so the gap is named in the value itself — where any
/// diagnostic that prints the config repeats it.
#[cfg(all(feature = "norn", feature = "liminal-transport"))]
const UNUSED_GRPC_ENDPOINT: &str =
    "unused: an agent worker dials liminal candidates, not a gRPC endpoint";

/// Composes the compiled-in harness over the document's surface and checks that what
/// would actually be REGISTERED is exactly that surface.
///
/// The set the server sees is the config's UNION of its activity types and its
/// descriptor names. Checking it here closes the gap between what was derived and
/// what gets advertised: a worker registered for a type it will not route, or routing
/// one it never announced, is caught before it dials instead of by a dispatch that
/// finds no owner.
#[cfg(all(feature = "norn", feature = "liminal-transport"))]
fn compose(surface: &AdvertisedSurface) -> Result<aion_worker::AgentHarnessConfig> {
    let advertised = surface.names();
    let agent = aion_worker::AgentHarnessConfig::new(
        crate::harness::default_dyn_agent_harness(),
        advertised.iter().cloned(),
        crate::harness::compiled_harness_capabilities(),
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
#[cfg(all(feature = "norn", feature = "liminal-transport"))]
fn worker_config(
    surface: &AdvertisedSurface,
    settings: &Settings,
    namespace: &str,
) -> Result<aion_worker::WorkerConfig> {
    let mut builder = aion_worker::WorkerConfig::builder()
        .endpoint(UNUSED_GRPC_ENDPOINT)
        .namespace(namespace)
        .task_queue(&surface.task_queue)
        .identity(&settings.identity)
        .max_concurrency(settings.concurrency)
        .reconnect_initial_backoff(settings.initial_backoff)
        .reconnect_max_backoff(settings.max_backoff)
        .reconnect_max_attempts(settings.max_attempts);
    if let Some(node) = &settings.node {
        builder = builder.node(node.clone());
    }
    builder.build().map_err(Into::into)
}

/// Composes the harness, registers the derived surface, and serves until a shutdown
/// signal arrives.
#[cfg(all(feature = "norn", feature = "liminal-transport"))]
async fn serve(surface: &AdvertisedSurface, settings: &Settings, namespace: &str) -> Result<()> {
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
    let agent = compose(surface)?;
    let config = worker_config(surface, settings, namespace)?;
    // The harness owns every action on this queue, so the typed registry is EMPTY:
    // an agent activity is executed by driving the harness, never by a registered
    // handler. The advertisement is the only thing that makes the worker selectable.
    let registry = Arc::new(aion_worker::ActivityRegistry::new());

    let queue = surface.task_queue.clone();
    let ready_queue = queue.clone();
    let served = advertised.iter().cloned().collect::<Vec<_>>().join(", ");
    let candidates = settings.addresses.join(", ");
    let addresses = settings.addresses.clone();
    let timing = aion_worker::RedialTiming::new(settings.initial_backoff, settings.max_backoff);
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
        harness = crate::harness::compiled_harness_name(),
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
#[cfg(all(feature = "norn", feature = "liminal-transport"))]
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
#[cfg(not(feature = "norn"))]
fn unservable(surface: &AdvertisedSurface, settings: &Settings, namespace: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "this `aion` build has NO agent harness compiled in (--no-default-features without \
         `norn`), so it cannot serve queue `{}` ({}) as `{}` in namespace `{namespace}` on {}. \
         Rebuild with the default features, or with `--features norn`.",
        surface.task_queue,
        describe_actions(surface),
        settings.identity,
        settings.addresses.join(", ")
    )
}

/// The refusal for a build that composes a harness but carries no transport able to
/// carry one — the agent-harness seam exists only on the liminal worker transport.
#[cfg(all(feature = "norn", not(feature = "liminal-transport")))]
fn unservable(surface: &AdvertisedSurface, settings: &Settings, namespace: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "this `aion` build composes an agent harness but has NO liminal worker transport \
         compiled in, and the agent-harness seam exists only on that transport — so queue `{}` \
         ({}) cannot be served as `{}` in namespace `{namespace}` on {}. Rebuild with the \
         default features, or add `--features liminal-transport`.",
        surface.task_queue,
        describe_actions(surface),
        settings.identity,
        settings.addresses.join(", ")
    )
}

/// The action surface a refusal names, so an operator sees the work that was
/// understood and not merely the missing feature.
#[cfg(not(all(feature = "norn", feature = "liminal-transport")))]
fn describe_actions(surface: &AdvertisedSurface) -> String {
    format!(
        "{} action(s): {}",
        surface.descriptors.len(),
        surface.names().into_iter().collect::<Vec<_>>().join(", ")
    )
}
