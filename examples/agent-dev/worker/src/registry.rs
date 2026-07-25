//! Plain activity registry composition for the agent-dev worker.

use std::future::Future;
use std::path::Path;
use std::sync::Arc;

use aion_worker::{ActivityContext, ActivityRegistry, HandlerFuture};

use crate::handlers;
use crate::shell::Shell;
use crate::types::{AssistantProvisionInput, GateInput, LandInput, ProvisionInput};

/// Adapt a handler that owns its cloned [`ActivityContext`] to the registry's
/// borrowed-context signature. The clone shares the cancellation state while
/// making the returned future independent of the borrow lifetime.
fn with_owned_context<Input, Output, Body, BodyFuture>(
    body: Body,
) -> impl for<'context> Fn(Input, &'context ActivityContext) -> HandlerFuture<'context, Output>
+ Send
+ Sync
+ 'static
where
    Input: Send + 'static,
    Output: Send + 'static,
    Body: Fn(Input, ActivityContext) -> BodyFuture + Clone + Send + Sync + 'static,
    BodyFuture: Future<Output = Result<Output, aion_worker::ActivityFailure>> + Send + 'static,
{
    move |input, context| Box::pin(body.clone()(input, context.clone()))
}

/// Build the cancellable registry for all plain worker activities.
///
/// # Errors
///
/// Returns an error if an activity name is registered more than once.
pub fn build_registry(
    shell: &Shell,
    workspace_root: &Path,
) -> anyhow::Result<Arc<ActivityRegistry>> {
    let provision = {
        let shell = shell.clone();
        let root = workspace_root.to_path_buf();
        move |input: ProvisionInput, context: ActivityContext| {
            let shell = shell.clone();
            let root = root.clone();
            async move { handlers::provision(&shell, &root, input, context).await }
        }
    };
    let assistant_provision = {
        let shell = shell.clone();
        let root = workspace_root.to_path_buf();
        move |input: AssistantProvisionInput, context: ActivityContext| {
            let shell = shell.clone();
            let root = root.clone();
            async move { handlers::assistant_provision(&shell, &root, input, context).await }
        }
    };
    let gate = {
        let shell = shell.clone();
        move |input: GateInput, context: ActivityContext| {
            let shell = shell.clone();
            async move { handlers::gate(&shell, input, context).await }
        }
    };
    let land = {
        let shell = shell.clone();
        move |input: LandInput, context: ActivityContext| {
            let shell = shell.clone();
            async move { handlers::land(&shell, input, context).await }
        }
    };
    let registry = ActivityRegistry::new()
        .register_activity("provision", with_owned_context(provision))?
        .register_activity(
            "assistant_provision",
            with_owned_context(assistant_provision),
        )?
        .register_activity("gate", with_owned_context(gate))?
        .register_activity("land", with_owned_context(land))?;
    Ok(Arc::new(registry))
}
