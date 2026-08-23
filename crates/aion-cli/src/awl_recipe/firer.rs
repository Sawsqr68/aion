//! Running a declared demonstration artifact, and reading the record its
//! output names.
//!
//! # Why the record is READ and not invented
//!
//! Every refusal this estate's instruments print carries `named cause: <code>`
//! — the command surface's own rendering, and the shape a gate script's
//! `GATE-RED:` line already uses. So a fired artifact's record is READ out of
//! the bytes it printed, not synthesised from an exit status: an exit code
//! says something failed, and the whole point of a known-answer red is that
//! the instrument said the RIGHT thing.
//!
//! An artifact whose output names no cause is refused rather than given one.
//! Inventing a code here would make every mismatch look like a match.

use std::path::{Path, PathBuf};

use aion_awl::command::Argv;
use aion_awl::command::recipe::{CommandRunner, SpecimenFirer, VerdictRecord};

/// The marker every instrument in this estate prints beside a refusal.
const CAUSE_MARKER: &str = "named cause: ";

/// The layer a fired artifact's record is filed under.
///
/// One string, stated once: a fired artifact is an instrument answering
/// about a tree, and `gate` is what such an instrument is called here.
const FIRED_LAYER: &str = "gate";

/// Fires a declared artifact as a real process, rooted.
pub(crate) struct ProcessFirer {
    root: PathBuf,
}

impl ProcessFirer {
    /// Root the firer at `root`.
    pub(crate) fn rooted(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }
}

impl SpecimenFirer for ProcessFirer {
    fn fire(&self, address: &str) -> Result<VerdictRecord, String> {
        let path = self.root.join(address);
        if !path.exists() {
            return Err(format!("{} does not exist", path.display()));
        }
        // Run to completion and read what it said. No timeout is imposed
        // here: a ceiling this command invented would be a number nobody
        // chose, and the declaration carries none. Nothing is spawned in the
        // background, so nothing needs killing.
        let output = std::process::Command::new(&path)
            .current_dir(&self.root)
            .output()
            .map_err(|error| format!("{} could not be run: {error}", path.display()))?;

        let mut said = String::from_utf8_lossy(&output.stdout).into_owned();
        said.push_str(&String::from_utf8_lossy(&output.stderr));
        let Some((code, line)) = named_cause(&said) else {
            return Err(format!(
                "{} exited {} and its output names no `{CAUSE_MARKER}<code>`, so there is nothing \
                 to compare a known answer against — an artifact fired as a known-answer red must \
                 say WHAT it found, not merely that it found something",
                path.display(),
                output
                    .status
                    .code()
                    .map_or_else(|| "by signal".to_owned(), |code| code.to_string())
            ));
        };
        VerdictRecord::new(code, FIRED_LAYER, line).map_err(|error| error.to_string())
    }
}

/// The first `named cause: <code>` an output carries, with the whole line it
/// stood on as the bytes observed.
fn named_cause(said: &str) -> Option<(String, String)> {
    said.lines().find_map(|line| {
        let at = line.find(CAUSE_MARKER)? + CAUSE_MARKER.len();
        let code: String = line[at..]
            .chars()
            .take_while(|character| character.is_ascii_lowercase() || *character == '_')
            .collect();
        (!code.is_empty()).then(|| (code, line.trim().to_owned()))
    })
}

/// Runs a declared command's EMITTED argv as a real process, rooted.
///
/// The argv arrives whole: one element per entry, never a string this side
/// re-splits. That is the property the command surface exists to keep, and
/// the executing side is the last place it could be lost — so it is spent
/// exactly as it arrived.
pub(crate) struct ProcessRunner {
    root: PathBuf,
}

impl ProcessRunner {
    /// Root the runner at `root`.
    pub(crate) fn rooted(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }
}

impl CommandRunner for ProcessRunner {
    fn run(&self, command: &Argv) -> Result<i32, String> {
        let Some((program, arguments)) = command.argv.split_first() else {
            return Err("the emitted argv is empty, so there is no program to run".to_owned());
        };
        let mut process = std::process::Command::new(program);
        process.args(arguments);
        // `cwd` arrives with its placeholder UNEXPANDED, because expanding it
        // is the executing side's act against ITS workspace — which is here,
        // and the workspace is the root this run was given.
        let working = command.cwd.as_ref().map_or_else(
            || self.root.clone(),
            |cwd| {
                self.root
                    .join(cwd.replace(aion_awl::WORKSPACE_ROOT_PLACEHOLDER, "."))
            },
        );
        process.current_dir(working);
        for (name, value) in &command.env {
            process.env(name, value);
        }
        if let Some(path) = &command.hardened_path {
            process.env("PATH", path);
        }
        // Run to completion. Nothing is spawned in the background, so nothing
        // needs killing; no ceiling is imposed that the declaration did not
        // state, because a number this side invented would be a number nobody
        // chose. A declared `timeout` is carried on the argv and is the
        // engine's to honour, not this command's to reinvent.
        let status = process
            .status()
            .map_err(|error| format!("`{program}` could not be run: {error}"))?;
        status.code().ok_or_else(|| {
            format!("`{program}` was ended by a signal rather than exiting, so it named no status")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::named_cause;

    /// The estate's own diagnostic shapes, read: a gate script's `GATE-RED:`
    /// line and a command-surface refusal both name their cause the same way.
    #[test]
    fn a_named_cause_is_read_out_of_the_shapes_this_estate_prints() {
        assert_eq!(
            named_cause(
                "GATE-RED: no changed file under crates/aion-awl/ — the census would have passed \
                 vacuously (named cause: deliverable_absent)"
            )
            .map(|(code, _)| code),
            Some("deliverable_absent".to_owned())
        );
        assert_eq!(
            named_cause("command `c` states nothing (observed: \"c\"; named cause: body_empty)")
                .map(|(code, _)| code),
            Some("body_empty".to_owned())
        );
    }

    /// An output naming no cause is NOTHING, never a nearest match.
    #[test]
    fn an_output_naming_no_cause_is_read_as_nothing() {
        assert_eq!(named_cause("everything is fine"), None);
        assert_eq!(named_cause(""), None);
        assert_eq!(named_cause("named cause: "), None);
        assert_eq!(named_cause("named cause: 404"), None);
    }

    /// The whole line rides as the bytes observed, so a receipt carries what
    /// the instrument actually printed.
    #[test]
    fn the_line_the_cause_stood_on_is_the_bytes_observed() {
        let Some((_, line)) = named_cause("  GATE-RED: x (named cause: tree_dirty)  ") else {
            unreachable!("the line names a cause");
        };
        assert_eq!(line, "GATE-RED: x (named cause: tree_dirty)");
    }
}
