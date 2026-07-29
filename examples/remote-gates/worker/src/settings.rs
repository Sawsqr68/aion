//! Operator-supplied settings shared by every activity in this worker.
//!
//! Nothing here has a default. The workspace root, the progress cadence, and the
//! notification channel are all decisions about the gate HOST, so they come from
//! the command line and the worker refuses to start without the required ones.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// Directory name, under the workspace root, that holds every leg log.
const LOG_DIRECTORY: &str = "logs";

/// Directory name, under the workspace root, that holds the checkouts.
const CHECKOUT_DIRECTORY: &str = "checkouts";

/// Everything the activities need to know about the machine they run on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    workspace_dir: PathBuf,
    notify_channel: Option<String>,
    progress_interval: Duration,
}

impl Settings {
    /// Builds the settings from already-validated operator input.
    #[must_use]
    pub const fn new(
        workspace_dir: PathBuf,
        notify_channel: Option<String>,
        progress_interval: Duration,
    ) -> Self {
        Self {
            workspace_dir,
            notify_channel,
            progress_interval,
        }
    }

    /// Root the worker owns on this host.
    #[must_use]
    pub fn workspace_dir(&self) -> &Path {
        &self.workspace_dir
    }

    /// Directory holding one working tree per repository.
    #[must_use]
    pub fn checkout_dir(&self) -> PathBuf {
        self.workspace_dir.join(CHECKOUT_DIRECTORY)
    }

    /// Directory holding one log file per leg run.
    #[must_use]
    pub fn log_dir(&self) -> PathBuf {
        self.workspace_dir.join(LOG_DIRECTORY)
    }

    /// Channel `notify_result` posts to, when the operator configured one.
    #[must_use]
    pub fn notify_channel(&self) -> Option<&str> {
        self.notify_channel.as_deref()
    }

    /// How often a running leg attaches progress to its liveness record.
    #[must_use]
    pub const fn progress_interval(&self) -> Duration {
        self.progress_interval
    }
}
