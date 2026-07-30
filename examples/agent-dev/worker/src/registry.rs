//! Plain activity registry composition for the agent-dev worker.

use std::future::Future;
use std::path::Path;
use std::sync::Arc;

use aion_package::ActivityDescriptor;
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
/// `plain_descriptors` are the typed schemas the deployed contract declares for
/// the registry-served actions, derived from the shipped `.awl` document (see
/// [`crate::descriptors`]). An action WITH a descriptor is registered through
/// the descriptor-carrying path, which is what makes this worker visible to
/// contract admission at all; an action without one is still registered so it
/// can be dispatched, but is not advertised.
///
/// # Errors
///
/// Returns an error if an activity name is registered more than once, if a
/// descriptor names an action this worker has no handler for — advertising an
/// action nothing can execute is how a queue comes to dispatch into a void —
/// or if a descriptor's name disagrees with the handler it is attached to.
pub fn build_registry(
    shell: &Shell,
    workspace_root: &Path,
    plain_descriptors: &[ActivityDescriptor],
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
    // Every handler this binary carries, by the name a dispatch arrives under.
    // The descriptor list is reconciled against THIS set below, so a document
    // that declares an action the worker cannot run is a startup failure rather
    // than a dispatch into a void.
    let served = ["provision", "assistant_provision", "gate", "land"];
    for descriptor in plain_descriptors {
        anyhow::ensure!(
            served.contains(&descriptor.name.as_str()),
            "the deployed contract declares registry action `{}`, which this worker has no \
             handler for; advertising it would admit the worker and then dispatch an action \
             nothing can execute. This worker serves {served:?}",
            descriptor.name
        );
    }
    let descriptor_for = |name: &str| {
        plain_descriptors
            .iter()
            .find(|descriptor| descriptor.name == name)
            .cloned()
    };

    let mut registry = ActivityRegistry::new();
    registry = register(
        registry,
        "provision",
        descriptor_for("provision"),
        provision,
    )?;
    registry = register(
        registry,
        "assistant_provision",
        descriptor_for("assistant_provision"),
        assistant_provision,
    )?;
    registry = register(registry, "gate", descriptor_for("gate"), gate)?;
    registry = register(registry, "land", descriptor_for("land"), land)?;
    Ok(Arc::new(registry))
}

/// Register one handler, WITH its typed descriptor when the contract declares
/// one. The descriptor-carrying path is the only one that makes an action
/// visible to contract admission; the plain path is for a handler no deployed
/// contract declares, which is dispatchable but never advertised.
fn register<Input, Output, Body, BodyFuture>(
    registry: ActivityRegistry,
    activity_type: &str,
    descriptor: Option<ActivityDescriptor>,
    body: Body,
) -> anyhow::Result<ActivityRegistry>
where
    Input: serde::Serialize + serde::de::DeserializeOwned + Send + Sync + 'static,
    Output: serde::Serialize + Send + Sync + 'static,
    Body: Fn(Input, ActivityContext) -> BodyFuture + Clone + Send + Sync + 'static,
    BodyFuture: Future<Output = Result<Output, aion_worker::ActivityFailure>> + Send + 'static,
{
    let registered = match descriptor {
        Some(descriptor) => registry.register_activity_with_descriptor(
            activity_type,
            descriptor,
            with_owned_context(body),
        )?,
        None => registry.register_activity(activity_type, with_owned_context(body))?,
    };
    Ok(registered)
}
