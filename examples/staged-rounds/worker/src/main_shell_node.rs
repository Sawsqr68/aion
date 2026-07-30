//! Typed registry and worker-SDK adapters for shell-node activities.
//!
//! Three adapter shapes, because the handlers genuinely differ:
//!
//! - [`blocking`] — a synchronous handler that shells out, moved onto the
//!   blocking pool.
//! - [`blocking_with_progress`] — the same, PLUS the activity's heartbeat
//!   progress channel. The two gate batteries (`gate_item`,
//!   `verify_integration`) run the operator's real `cargo build` /
//!   `cargo clippy` / `cargo test`, which on a whole-project run dominate
//!   wall-clock. Without this an operator would see the activity start and
//!   then nothing at all until it finished many minutes later; with it, every
//!   gate command's start and its exit status arrive as progress detail. The
//!   context is CLONED into the blocking task because the battery reports from
//!   the blocking thread, not from the async adapter.
//! - [`immediate`] — a pure handler, run without consuming a blocking-pool
//!   thread.
//!
//! EVERY handler is registered WITH ITS DECLARED WIRE DESCRIPTOR, taken from the
//! compiled AWL document (see [`staged_rounds_worker::declaration`]). A handler
//! committed with no descriptor advertises nothing, and the server's contract
//! gate then refuses the whole connection with `WORKER_CONTRACT_MISMATCH`
//! naming every action as `<missing>`. The handlers stay TYPED — the descriptor
//! decides only what is advertised, never how the payload is decoded.

use aion_worker::{ActivityContext, ActivityRegistry, HandlerFuture};
use staged_rounds_worker::declaration::Declaration;
use staged_rounds_worker::handlers;
use staged_rounds_worker::handlers::progress::{GateProgress, HeartbeatProgress};
use staged_rounds_worker::shell::Shell;

/// The six shell-node activities served by the binary, in the document's
/// action order, each advertising the schemas the document declares for it.
///
/// # Errors
///
/// A registration failure from the SDK, or the declaration guard's message when
/// the document declares no action of a registered name, or declares one pinned
/// to a node other than the shell connection's.
pub(super) fn shell_registry(
    shell: Shell,
    declaration: &Declaration,
) -> anyhow::Result<ActivityRegistry> {
    let declared = |action: &str| {
        declaration
            .descriptor(action, super::SHELL_NODE)
            .map_err(anyhow::Error::msg)
    };
    Ok(ActivityRegistry::new()
        .register_activity_with_descriptor(
            "provision_item",
            declared("provision_item")?,
            blocking(shell.clone(), handlers::provision_item),
        )?
        .register_activity_with_descriptor(
            "gate_item",
            declared("gate_item")?,
            blocking_with_progress(shell.clone(), handlers::gate_item),
        )?
        .register_activity_with_descriptor(
            "plan_reviews",
            declared("plan_reviews")?,
            immediate(handlers::plan_reviews),
        )?
        .register_activity_with_descriptor(
            "fold_phase",
            declared("fold_phase")?,
            immediate(handlers::fold_phase),
        )?
        .register_activity_with_descriptor(
            "merge_branches",
            declared("merge_branches")?,
            blocking(shell.clone(), handlers::merge_branches),
        )?
        .register_activity_with_descriptor(
            "verify_integration",
            declared("verify_integration")?,
            blocking_with_progress(shell, handlers::verify_integration),
        )?)
}

/// Move a synchronous shell handler onto the blocking thread pool.
fn blocking<Input, Output>(
    shell: Shell,
    body: fn(&Shell, Input) -> Result<Output, aion_worker::ActivityFailure>,
) -> impl for<'context> Fn(Input, &'context ActivityContext) -> HandlerFuture<'context, Output>
+ Send
+ Sync
+ 'static
where
    Input: Send + 'static,
    Output: Send + 'static,
{
    move |input: Input, _context: &ActivityContext| {
        let shell = shell.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || body(&shell, input))
                .await
                .map_err(|join_error| join_failure(&join_error))?
        })
    }
}

/// Move a synchronous shell handler onto the blocking thread pool WITH the
/// activity's heartbeat progress channel, for the long gate batteries.
fn blocking_with_progress<Input, Output>(
    shell: Shell,
    body: fn(&Shell, &dyn GateProgress, Input) -> Result<Output, aion_worker::ActivityFailure>,
) -> impl for<'context> Fn(Input, &'context ActivityContext) -> HandlerFuture<'context, Output>
+ Send
+ Sync
+ 'static
where
    Input: Send + 'static,
    Output: Send + 'static,
{
    move |input: Input, context: &ActivityContext| {
        let shell = shell.clone();
        let progress = HeartbeatProgress::new(context.clone());
        Box::pin(async move {
            tokio::task::spawn_blocking(move || body(&shell, &progress, input))
                .await
                .map_err(|join_error| join_failure(&join_error))?
        })
    }
}

/// Adapt a pure synchronous handler without consuming a blocking-pool thread.
fn immediate<Input, Output>(
    body: fn(Input) -> Result<Output, aion_worker::ActivityFailure>,
) -> impl for<'context> Fn(Input, &'context ActivityContext) -> HandlerFuture<'context, Output>
+ Send
+ Sync
+ 'static
where
    Input: Send + 'static,
    Output: Send + 'static,
{
    move |input: Input, _context: &ActivityContext| Box::pin(async move { body(input) })
}

/// A blocking task that did not complete is an activity failure naming why —
/// never a silently dropped result.
fn join_failure(join_error: &tokio::task::JoinError) -> aion_worker::ActivityFailure {
    aion_worker::ActivityFailure::terminal(format!(
        "activity handler task did not complete: {join_error}"
    ))
}
