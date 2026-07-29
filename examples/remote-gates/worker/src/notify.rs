//! The advisory side channel.
//!
//! `notify_result` is declared `advisory` in `remote_gates.awl`, so the engine
//! routes its faults around the step's fault arm: an unreachable notifier warns
//! and the gate run still reaches its verdict. That only works if this activity
//! is honest about what happened — so there are exactly two shapes here.
//!
//! * No channel configured: a DELIBERATE operator choice. Returns a typed skip
//!   with `delivered: false`. Not a failure, and never silence.
//! * A channel configured: the post either succeeded, or the activity FAILS.
//!   A `delivered: false` "success" would hide a real delivery failure behind a
//!   boolean, which is exactly what the advisory class exists to handle.

use aion_worker::{ActivityContext, ActivityFailure};

use crate::logs::log_path;
use crate::process::{CommandSpec, run_logged};
use crate::settings::Settings;
use crate::types::{NotifyInput, NotifyResult};

/// Collective CLI used to post the summary, resolved from `PATH`.
const MERIDIAN: &str = "meridian";

/// Posts a run summary to the configured side channel.
///
/// # Errors
///
/// Returns [`ActivityFailure`] when a channel IS configured and the post could
/// not be made. The action's `advisory` declaration is what keeps that failure
/// from sinking the gate run.
pub async fn notify_result(
    input: NotifyInput,
    settings: &Settings,
    context: &ActivityContext,
) -> Result<NotifyResult, ActivityFailure> {
    let Some(channel) = settings.notify_channel() else {
        return Ok(NotifyResult {
            delivered: false,
            detail: "skipped: no channel configured (worker started without --notify-channel)"
                .to_owned(),
        });
    };
    let workspace = settings.workspace_dir().to_path_buf();
    let log = log_path(settings, &workspace, "notify", context);
    let spec = CommandSpec {
        program: MERIDIAN.to_owned(),
        args: vec![
            "channel".to_owned(),
            "send".to_owned(),
            "--channel".to_owned(),
            channel.to_owned(),
            "--message".to_owned(),
            input.summary,
        ],
        env: Vec::new(),
        cwd: workspace,
    };
    let run = run_logged(&spec, &log, "notify", context, settings.progress_interval())
        .await
        .map_err(|error| {
            ActivityFailure::terminal(format!(
                "notify to channel `{channel}` could not run: {error}"
            ))
        })?;
    if run.exit_code != 0 {
        return Err(ActivityFailure::terminal(format!(
            "notify to channel `{channel}` exited {}; see {}\n{}",
            run.exit_code,
            run.log_path.display(),
            run.output_tail
        )));
    }
    Ok(NotifyResult {
        delivered: true,
        detail: format!(
            "posted to channel `{channel}`; see {}",
            run.log_path.display()
        ),
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use aion_core::ActivityId;
    use aion_worker::ActivityContext;

    use super::notify_result;
    use crate::settings::Settings;
    use crate::types::NotifyInput;

    #[tokio::test]
    async fn an_unconfigured_channel_is_a_typed_skip_not_a_silence()
    -> Result<(), Box<dyn std::error::Error>> {
        let settings = Settings::new(PathBuf::from("/srv/gates"), None, Duration::from_secs(30));
        let (context, _cancel) = ActivityContext::new(ActivityId::from_sequence_position(1), 1);
        let result = notify_result(
            NotifyInput {
                summary: "remote_gates CLEAN".to_owned(),
            },
            &settings,
            &context,
        )
        .await?;
        assert!(!result.delivered);
        assert!(
            result.detail.contains("--notify-channel"),
            "the skip must name the missing switch: {}",
            result.detail
        );
        Ok(())
    }
}
