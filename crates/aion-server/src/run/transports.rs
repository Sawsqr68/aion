//! The transport legs of the run loop: the gRPC and HTTP serve futures over
//! pre-bound listeners, the shutdown-watch adapter both use, the OS signal
//! listener that requests the drain, and the typed errors each leg wears.
//!
//! Split from `run.rs` so the run module stays a composition root; the door
//! choreography that spawns these legs lives in [`super::doors`].

use std::net::SocketAddr;

use tokio::net::TcpListener;
use tonic::transport::Server as TonicServer;

use crate::{ServerError, ServerState, api};

pub(super) fn transport_result(
    transport: &'static str,
    result: Result<Result<(), ServerError>, tokio::task::JoinError>,
) -> Result<(), ServerError> {
    match result {
        Ok(transport_outcome) => transport_outcome,
        Err(join_error) => Err(ServerError::Transport {
            transport,
            message: join_error.to_string(),
        }),
    }
}

pub(super) async fn serve_grpc(
    state: ServerState,
    listener: TcpListener,
    address: SocketAddr,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), ServerError> {
    let workflow = api::grpc::workflow_service(state.clone());
    let worker = api::worker_grpc::worker_service(state.clone());
    let mut router = TonicServer::builder()
        .add_service(workflow)
        .add_service(worker);
    // Dark by default: the deploy service joins the listener only when the
    // operator commissioned it; otherwise the surface answers Unimplemented.
    if state.runtime_config().deploy.enabled {
        router = router.add_service(api::deploy_grpc::deploy_service(state)?);
    }
    // The pre-bound listener rides in as a `TcpIncoming`, NOT a bare
    // `TcpListenerStream`: tonic applies its socket options (`tcp_nodelay`
    // defaults to true) only in `bind_incoming`, and documents that a raw
    // incoming stream ignores them — a bare stream would silently re-enable
    // Nagle's algorithm on every worker-protocol and client connection.
    let incoming = tonic::transport::server::TcpIncoming::from(listener).with_nodelay(Some(true));
    router
        .serve_with_incoming_shutdown(incoming, shutdown_requested(shutdown))
        .await
        .map_err(|source| transport_bind("grpc", address, source))?;
    Ok(())
}

pub(super) async fn serve_http(
    state: ServerState,
    listener: TcpListener,
    address: SocketAddr,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), ServerError> {
    axum::serve(listener, api::http::http_router(state)?)
        .with_graceful_shutdown(shutdown_requested(shutdown))
        .await
        .map_err(|source| transport_bind("http", address, source))?;
    Ok(())
}

async fn shutdown_requested(mut shutdown: tokio::sync::watch::Receiver<bool>) {
    while !*shutdown.borrow_and_update() {
        if shutdown.changed().await.is_err() {
            break;
        }
    }
}

pub(super) async fn shutdown_signal() -> Result<(), ServerError> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut terminate = signal(SignalKind::terminate())
            .map_err(|source| signal_listener("SIGTERM", &source))?;
        let mut interrupt =
            signal(SignalKind::interrupt()).map_err(|source| signal_listener("SIGINT", &source))?;
        tokio::select! {
            _ = terminate.recv() => Ok(()),
            _ = interrupt.recv() => Ok(()),
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .map_err(|source| signal_listener("shutdown signal", &source))
    }
}

fn signal_listener(listener: &'static str, source: &std::io::Error) -> ServerError {
    ServerError::SignalListener {
        listener,
        message: source.to_string(),
    }
}

pub(super) fn transport_bind<E>(
    transport: &'static str,
    address: SocketAddr,
    source: E,
) -> ServerError
where
    E: std::error::Error,
{
    ServerError::TransportBind {
        transport,
        address,
        message: source.to_string(),
    }
}
