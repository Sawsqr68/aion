//! Registering a declared command as a servable activity.
//!
//! [`ShellAction`] knows how to run a declared command; this is how a worker
//! offers one to the server under an activity-type name, so an action whose
//! body was written in an `.awl` document is served by exactly the same
//! dispatch path as a hand-written Rust activity.

use std::collections::BTreeMap;

use crate::activity::{ActivityRegistry, HandlerFuture};
use crate::error::WorkerError;

use super::action::{ShellAction, ShellOutcome};

/// The arguments a declared action is called with.
///
/// A declared action has no Rust input type to deserialize into: its
/// parameters are named in the `.awl` document, so the call arrives as a
/// JSON object and the command's own parameter references decide which
/// members are used. A value that no reference names is simply unused; a
/// reference with no value is refused by name at render time rather than
/// silently rendering empty.
pub type DeclaredArguments = BTreeMap<String, serde_json::Value>;

/// Offering declared commands as activities.
pub trait DeclaredCommands: Sized {
    /// Register `action` to be served under `activity_type`.
    ///
    /// # Errors
    ///
    /// Returns [`WorkerError::Registration`] when `activity_type` already has
    /// a registered handler. A declared action never silently displaces a
    /// hand-written one: a name collision is a deployment mistake worth
    /// refusing, because the wrong body would otherwise run under the right
    /// name.
    fn register_declared_command(
        self,
        activity_type: impl Into<String>,
        action: ShellAction,
    ) -> Result<Self, WorkerError>;
}

impl DeclaredCommands for ActivityRegistry {
    fn register_declared_command(
        self,
        activity_type: impl Into<String>,
        action: ShellAction,
    ) -> Result<Self, WorkerError> {
        self.register_activity(
            activity_type,
            move |arguments: DeclaredArguments, context| -> HandlerFuture<'_, ShellOutcome> {
                // The action is cloned per call rather than shared behind a
                // lock: it is an immutable parsed command, so a clone is a
                // few allocations and every call stays independent of every
                // other. Nothing about one run can affect another.
                let action = action.clone();
                Box::pin(async move { action.run(&arguments, context).await })
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{DeclaredArguments, DeclaredCommands};
    use crate::activity::ActivityRegistry;
    use crate::runtime::{ActivityDispatcher, DispatchOutcome, decode_payload, encode_payload};
    use crate::shell::action::{FailureMode, ShellAction, ShellOutcome};
    use aion_core::{ActivityErrorKind, ActivityId, ContentType, WorkflowId};

    /// What a test returns. Every fallible step is carried rather than
    /// unwrapped, because the workspace denies panicking accessors in test
    /// code as firmly as in library code.
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn arguments(pairs: &[(&str, serde_json::Value)]) -> DeclaredArguments {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect()
    }

    /// Dispatch `activity_type` through `registry` exactly as the serving
    /// loop would, so these tests exercise the real registered path rather
    /// than calling the action directly.
    async fn dispatch(
        registry: &ActivityRegistry,
        activity_type: &str,
        call: &DeclaredArguments,
    ) -> Result<DispatchOutcome, Box<dyn std::error::Error>> {
        let task = crate::protocol::ActivityTask {
            workflow_id: WorkflowId::new_v4(),
            activity_id: ActivityId::from_sequence_position(1),
            run_id: None,
            activity_type: activity_type.to_owned(),
            attempt: 1,
            completion_token: "declared-command-test".to_owned(),
            idempotency_key: "declared-command-test".to_owned(),
            input: encode_payload(call)?,
            labels: std::collections::BTreeMap::new(),
        };
        let (context, cancellation) =
            crate::ActivityContext::new(ActivityId::from_sequence_position(1), 1);
        drop(cancellation);
        Ok(registry.dispatch(task, context).await?)
    }

    #[tokio::test]
    async fn a_declared_command_is_served_through_the_normal_dispatch_path() -> TestResult {
        let registry = ActivityRegistry::new()
            .register_declared_command("greet", ShellAction::new("echo $greeting")?)?;

        let outcome = dispatch(
            &registry,
            "greet",
            &arguments(&[("greeting", serde_json::json!("hello there"))]),
        )
        .await?;

        let DispatchOutcome::Completed { output } = outcome else {
            return Err("a succeeding declared command must complete".into());
        };
        assert_eq!(output.content_type(), &ContentType::Json);
        let result: ShellOutcome = decode_payload(&output)?;
        assert_eq!(result.stdout, "hello there");
        assert_eq!(result.exit_code, 0);
        Ok(())
    }

    #[tokio::test]
    async fn a_failing_declared_command_reports_its_classification() -> TestResult {
        let registry = ActivityRegistry::new().register_declared_command(
            "fails",
            ShellAction::new("sh -c 'exit 4'")?.with_failure_mode(FailureMode::Terminal),
        )?;

        let outcome = dispatch(&registry, "fails", &arguments(&[])).await?;

        let DispatchOutcome::Failed { failure } = outcome else {
            return Err("a non-zero exit must fail the activity".into());
        };
        assert_eq!(failure.kind, ActivityErrorKind::Terminal);
        assert!(
            !failure.is_retryable(),
            "a terminal declared-command failure must not be retryable"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_declared_command_cannot_displace_a_registered_activity() -> TestResult {
        let registry = ActivityRegistry::new()
            .register_declared_command("shared", ShellAction::new("echo first")?)?;

        let Err(error) =
            registry.register_declared_command("shared", ShellAction::new("echo second")?)
        else {
            return Err("a duplicate activity type must be refused, not silently replaced".into());
        };
        assert!(
            error.to_string().contains("shared"),
            "the refusal must name the colliding activity type: {error}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn an_unnamed_argument_is_ignored_rather_than_smuggled_into_the_command() -> TestResult {
        // A caller can send anything; only what the command names is used.
        let registry = ActivityRegistry::new()
            .register_declared_command("echo", ShellAction::new("echo $wanted")?)?;

        let outcome = dispatch(
            &registry,
            "echo",
            &arguments(&[
                ("wanted", serde_json::json!("kept")),
                ("ignored", serde_json::json!("; echo PWNED")),
            ]),
        )
        .await?;

        let DispatchOutcome::Completed { output } = outcome else {
            return Err("the command must complete".into());
        };
        let result: ShellOutcome = decode_payload(&output)?;
        assert_eq!(result.stdout, "kept");
        Ok(())
    }
}
