//! Where each command's log lands on the gate host.
//!
//! One file per command per activity attempt, under the operator's workspace
//! root, so a run that finished hours ago can still be read at the bytes.

use std::path::{Path, PathBuf};

use aion_worker::ActivityContext;

use crate::settings::Settings;

/// Directory name used when a checkout path has no final component.
pub const UNNAMED_CHECKOUT: &str = "workspace";

/// Log file for one command of one activity attempt.
#[must_use]
pub fn log_path(
    settings: &Settings,
    workspace: &Path,
    step: &str,
    context: &ActivityContext,
) -> PathBuf {
    settings
        .log_dir()
        .join(checkout_name(workspace))
        .join(format!("{}-{}.log", sanitize(step), run_key(context)))
}

/// Discriminator that keeps one attempt's log from overwriting another's.
///
/// The server's external-effect key is unique per activity task; a context
/// created without one (an isolated unit test) falls back to the activity's own
/// identifier, which is unique within its run.
#[must_use]
pub fn run_key(context: &ActivityContext) -> String {
    let base = context.idempotency_key().map_or_else(
        || context.activity_id().to_string(),
        std::borrow::ToOwned::to_owned,
    );
    sanitize(&format!("{base}-attempt{}", context.attempt()))
}

/// Final path component of a checkout, reduced to a safe directory name.
#[must_use]
pub fn checkout_name(workspace: &Path) -> String {
    workspace.file_name().map_or_else(
        || UNNAMED_CHECKOUT.to_owned(),
        |name| sanitize(&name.to_string_lossy()),
    )
}

/// Keeps a path component to characters that mean the same thing on every
/// filesystem the gate host might use.
#[must_use]
pub fn sanitize(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if cleaned.is_empty() {
        UNNAMED_CHECKOUT.to_owned()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{checkout_name, sanitize};

    #[test]
    fn path_components_are_reduced_to_portable_characters() {
        assert_eq!(sanitize("aion/../etc"), "aion_.._etc");
        assert_eq!(sanitize("remote gates"), "remote_gates");
        assert_eq!(sanitize(""), "workspace");
    }

    #[test]
    fn a_checkout_directory_names_its_own_log_folder() {
        assert_eq!(
            checkout_name(Path::new("/srv/gates/checkouts/aion")),
            "aion"
        );
        assert_eq!(checkout_name(Path::new("/")), "workspace");
    }
}
