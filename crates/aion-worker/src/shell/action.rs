//! A declared command executed as an activity.
//!
//! [`ShellAction`] turns a command declared in an `.awl` worker block into a
//! runnable activity, so an action can do real work without a hand-written
//! Rust crate. The command is parsed once at construction and executed as a
//! direct `execve` with no shell interposed — see [`super::template`] for why
//! that is the security property rather than a convenience.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use super::template::{CommandTemplate, SubstitutionError, TemplateError};
use crate::activity::ActivityFailure;
use crate::context::ActivityContext;
use crate::process::{CancellableCommandOutput, ProcessGroupError, run_cancellable_command};

/// What a declared action produces when its command succeeds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ShellOutcome {
    /// The command's exit status code.
    pub exit_code: i32,
    /// Everything the command wrote to standard output, trailing newline
    /// trimmed so a one-line result is usable directly.
    pub stdout: String,
    /// Everything the command wrote to standard error, trailing newline
    /// trimmed. Captured even on success, because a command that succeeds
    /// while warning is a thing an operator needs to see.
    pub stderr: String,
}

/// How a declared action treats a non-zero exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FailureMode {
    /// A non-zero exit is retryable: the engine may run the command again
    /// under the action's retry policy. This is the default because the
    /// common causes of a failing command — a busy resource, an unreachable
    /// host, a transient permission state — are the ones that succeed on a
    /// second attempt.
    #[default]
    Retryable,
    /// A non-zero exit is terminal: the command will never be retried. Use
    /// when re-running the command could not possibly change the outcome, or
    /// when running it twice would itself be harmful.
    Terminal,
}

/// A declared command, ready to run as an activity.
#[derive(Debug, Clone)]
pub struct ShellAction {
    template: CommandTemplate,
    failure_mode: FailureMode,
}

impl ShellAction {
    /// Parse `command` into a runnable action.
    ///
    /// # Errors
    ///
    /// Returns [`TemplateError`] when the command is empty, has an unterminated
    /// quote or `${`, contains a `$` naming no parameter, or puts a parameter
    /// in program position.
    pub fn new(command: &str) -> Result<Self, TemplateError> {
        Ok(Self {
            template: CommandTemplate::parse(command)?,
            failure_mode: FailureMode::default(),
        })
    }

    /// Set how a non-zero exit is classified.
    #[must_use]
    pub const fn with_failure_mode(mut self, failure_mode: FailureMode) -> Self {
        self.failure_mode = failure_mode;
        self
    }

    /// The parameter names this command references, sorted.
    ///
    /// A caller can compare these against an action's declared parameters to
    /// catch a reference to a parameter that does not exist.
    #[must_use]
    pub fn referenced_parameters(&self) -> Vec<String> {
        self.template.referenced_parameters()
    }

    /// Run the command with `arguments` bound to its parameter references.
    ///
    /// Cancellation is tree-wide: the command runs in its own process group,
    /// and cancelling the activity terminates that whole group rather than
    /// only the direct child, so a command that spawned children does not
    /// leave them running.
    ///
    /// # Errors
    ///
    /// Returns an [`ActivityFailure`] when a referenced parameter is missing or
    /// unrepresentable (terminal — re-running cannot supply it), when the
    /// command cannot be spawned or observed (terminal — the program is absent
    /// or unrunnable), when the command is cancelled (terminal), or when it
    /// exits non-zero (classified by the action's [`FailureMode`]).
    pub async fn run(
        &self,
        arguments: &BTreeMap<String, serde_json::Value>,
        context: &ActivityContext,
    ) -> Result<ShellOutcome, ActivityFailure> {
        let argv = self
            .template
            .render(arguments)
            .map_err(|error| substitution_failure(&error))?;
        let (program, rest) = argv
            .split_first()
            // `CommandTemplate::parse` refuses an empty command, so a rendered
            // argv always has a program. Handled rather than indexed so a
            // future change to the parser cannot turn this into a panic.
            .ok_or_else(|| ActivityFailure::terminal("the declared command rendered no program"))?;

        let mut command = Command::new(program);
        command.args(rest);

        match run_cancellable_command(command, context.cancelled()).await {
            Ok(CancellableCommandOutput::Completed(output)) => {
                let outcome = ShellOutcome {
                    exit_code: output.status.code().unwrap_or(EXIT_CODE_SIGNALLED),
                    stdout: trim_trailing_newline(&String::from_utf8_lossy(&output.stdout)),
                    stderr: trim_trailing_newline(&String::from_utf8_lossy(&output.stderr)),
                };
                if output.status.success() {
                    Ok(outcome)
                } else {
                    Err(self.exit_failure(program, &outcome))
                }
            }
            Ok(CancellableCommandOutput::Cancelled) => Err(ActivityFailure::terminal(format!(
                "the declared command `{program}` was cancelled and its process group was terminated"
            ))),
            Err(error) => Err(spawn_failure(program, &error)),
        }
    }

    /// Build the failure for a command that ran and exited non-zero.
    fn exit_failure(&self, program: &str, outcome: &ShellOutcome) -> ActivityFailure {
        // The message carries stderr because a failing command's own words are
        // the most useful thing an operator can be handed; without it the
        // failure says only that something exited non-zero.
        let message = if outcome.stderr.is_empty() {
            format!(
                "the declared command `{program}` exited {} with no standard error output",
                outcome.exit_code
            )
        } else {
            format!(
                "the declared command `{program}` exited {}: {}",
                outcome.exit_code, outcome.stderr
            )
        };
        match self.failure_mode {
            FailureMode::Retryable => ActivityFailure::retryable(message),
            FailureMode::Terminal => ActivityFailure::terminal(message),
        }
    }
}

/// Exit code reported when a command was ended by a signal and so has none.
///
/// Matches the long-standing shell convention of `128 + SIGKILL`, which is the
/// number an operator reading the result will already recognise.
const EXIT_CODE_SIGNALLED: i32 = 137;

/// Classify a substitution failure. All are terminal: the same call with the
/// same arguments would fail identically however many times it is retried.
fn substitution_failure(error: &SubstitutionError) -> ActivityFailure {
    ActivityFailure::terminal(error.to_string())
}

/// Classify a spawn or observation failure.
fn spawn_failure(program: &str, error: &ProcessGroupError) -> ActivityFailure {
    // Terminal in every arm: a program that is absent, unrunnable, or whose
    // containment could not be established will be exactly as absent on the
    // next attempt, and retrying a command whose termination could NOT be
    // confirmed risks running it twice concurrently.
    ActivityFailure::terminal(format!(
        "the declared command `{program}` could not be run to completion: {error}"
    ))
}

/// Trim exactly one trailing newline, and its carriage return if present.
///
/// A command's output almost always ends in a newline that belongs to the
/// terminal rather than to the value, so `echo hello` yields `hello` and not
/// `hello\n`. Only one is removed: a command that deliberately emits blank
/// trailing lines keeps them.
fn trim_trailing_newline(text: &str) -> String {
    text.strip_suffix('\n')
        .map_or(text, |trimmed| {
            trimmed.strip_suffix('\r').unwrap_or(trimmed)
        })
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{FailureMode, ShellAction, trim_trailing_newline};
    use crate::activity::Classification;
    use crate::context::ActivityContext;
    use aion_core::ActivityId;
    use std::collections::BTreeMap;

    fn context() -> (ActivityContext, crate::context::ActivityCancellationHandle) {
        ActivityContext::new(ActivityId::from_sequence_position(1), 1)
    }

    fn arguments(pairs: &[(&str, serde_json::Value)]) -> BTreeMap<String, serde_json::Value> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect()
    }

    /// What a test returns. Every fallible step is carried rather than
    /// unwrapped, because the workspace denies panicking accessors in test
    /// code as firmly as in library code.
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[tokio::test]
    async fn a_succeeding_command_returns_its_output() -> TestResult {
        let action = ShellAction::new("echo hello")?;
        let (context, _handle) = context();
        let outcome = action.run(&BTreeMap::new(), &context).await?;
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, "hello");
        assert_eq!(outcome.stderr, "");
        Ok(())
    }

    #[tokio::test]
    async fn a_parameter_value_reaches_the_program_as_one_argument() -> TestResult {
        let action = ShellAction::new("echo $greeting")?;
        let (context, _handle) = context();
        let outcome = action
            .run(
                &arguments(&[("greeting", serde_json::json!("hello there world"))]),
                &context,
            )
            .await?;
        assert_eq!(outcome.stdout, "hello there world");
        Ok(())
    }

    #[tokio::test]
    async fn a_hostile_value_is_inert_because_no_shell_ever_sees_it() -> TestResult {
        // If a shell were interposed, this would execute a second command.
        // Because argv goes straight to execve, it is printed as text.
        let action = ShellAction::new("echo $value")?;
        let (context, _handle) = context();
        let outcome = action
            .run(
                &arguments(&[("value", serde_json::json!("hi; echo PWNED"))]),
                &context,
            )
            .await?;
        assert_eq!(outcome.stdout, "hi; echo PWNED");
        assert!(
            !outcome.stdout.contains("PWNED\n"),
            "the injected command must never have run"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_nonzero_exit_is_retryable_by_default_and_carries_stderr() -> TestResult {
        let action = ShellAction::new("sh -c 'echo trouble >&2; exit 3'")?;
        let (context, _handle) = context();
        let Err(failure) = action.run(&BTreeMap::new(), &context).await else {
            return Err("a non-zero exit must fail the activity".into());
        };
        assert_eq!(failure.classification(), &Classification::Retryable);
        assert!(
            failure.message().contains("trouble"),
            "the failure must carry the command's own words: {}",
            failure.message()
        );
        assert!(failure.message().contains('3'), "the exit code is reported");
        Ok(())
    }

    #[tokio::test]
    async fn a_nonzero_exit_is_terminal_when_the_action_says_so() -> TestResult {
        let action = ShellAction::new("sh -c 'exit 1'")?.with_failure_mode(FailureMode::Terminal);
        let (context, _handle) = context();
        let Err(failure) = action.run(&BTreeMap::new(), &context).await else {
            return Err("a non-zero exit must fail the activity".into());
        };
        assert_eq!(failure.classification(), &Classification::Terminal);
        Ok(())
    }

    #[tokio::test]
    async fn a_missing_parameter_fails_terminally_before_anything_runs() -> TestResult {
        let action = ShellAction::new("echo $absent")?;
        let (context, _handle) = context();
        let Err(failure) = action.run(&BTreeMap::new(), &context).await else {
            return Err("a missing parameter must fail the activity".into());
        };
        assert_eq!(failure.classification(), &Classification::Terminal);
        assert!(failure.message().contains("absent"));
        Ok(())
    }

    #[tokio::test]
    async fn an_absent_program_fails_terminally() -> TestResult {
        let action = ShellAction::new("aion-no-such-program-exists-anywhere")?;
        let (context, _handle) = context();
        let Err(failure) = action.run(&BTreeMap::new(), &context).await else {
            return Err("an absent program must fail the activity".into());
        };
        assert_eq!(failure.classification(), &Classification::Terminal);
        Ok(())
    }

    #[tokio::test]
    async fn cancellation_stops_the_command_and_fails_terminally() -> TestResult {
        let action = ShellAction::new("sleep 30")?;
        let (context, handle) = context();
        let run = tokio::spawn(async move {
            let (context, _keep) = (context, ());
            action.run(&BTreeMap::new(), &context).await
        });
        // Give the command a moment to actually be running before cancelling,
        // so this exercises termination rather than a pre-start short circuit.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        handle.cancel();
        let Err(failure) = run.await? else {
            return Err("a cancelled command must fail the activity".into());
        };
        assert_eq!(failure.classification(), &Classification::Terminal);
        assert!(failure.message().contains("cancelled"));
        Ok(())
    }

    #[test]
    fn only_one_trailing_newline_is_trimmed() {
        assert_eq!(trim_trailing_newline("hello\n"), "hello");
        assert_eq!(trim_trailing_newline("hello\r\n"), "hello");
        assert_eq!(trim_trailing_newline("hello\n\n"), "hello\n");
        assert_eq!(trim_trailing_newline("hello"), "hello");
        assert_eq!(trim_trailing_newline(""), "");
    }
}
