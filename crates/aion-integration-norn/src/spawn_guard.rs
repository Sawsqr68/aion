//! Worktree-scoped live-sibling exclusion for agent process launches.

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use aion_integrations::{AgentRunSpec, HarnessError};
use nix::errno::Errno;
use nix::sys::signal::kill;
use nix::unistd::Pid;
use serde_json::{Value, json};

use crate::claim_area;
const MARKER_NAME: &str = ".aion-held-execution.json";

#[derive(Debug)]
struct ProcessIdentity {
    started: String,
    command: String,
}

#[derive(Debug)]
struct Marker {
    tree: String,
    workflow_id: String,
    activity_id: String,
    pid: u32,
    process: ProcessIdentity,
}

/// Exclusive claim held across one child spawn.
///
/// The reservation is removed on every pre-spawn failure. Once bound to the
/// spawned child, the marker remains until the next claimant observes that the
/// exact process incarnation is gone and cleans it.
#[derive(Debug)]
pub(crate) struct SpawnClaim {
    marker_path: PathBuf,
    tree: String,
    armed: bool,
}

impl SpawnClaim {
    /// Atomically reserve `cwd`, cleaning only a marker whose exact process
    /// incarnation is no longer live.
    pub(crate) fn acquire(cwd: &Path, spec: &AgentRunSpec) -> Result<Self, HarnessError> {
        let area = claim_area::for_tree(cwd)?;
        Self::acquire_in(&area, spec)
    }

    #[cfg(test)]
    fn acquire_under(
        cwd: &Path,
        state_root: &Path,
        spec: &AgentRunSpec,
    ) -> Result<Self, HarnessError> {
        let area = claim_area::for_tree_under(cwd, state_root)?;
        Self::acquire_in(&area, spec)
    }

    fn acquire_in(area: &claim_area::ClaimArea, spec: &AgentRunSpec) -> Result<Self, HarnessError> {
        let tree = area
            .canonical_tree
            .to_str()
            .map(str::to_owned)
            .ok_or_else(|| {
                HarnessError::configuration(format!(
                    "canonical spawn-claim tree `{}` is not valid UTF-8",
                    area.canonical_tree.display()
                ))
            })?;
        std::fs::create_dir_all(&area.directory)
            .map_err(|error| marker_io("create claim directory for", &area.directory, &error))?;
        let marker_path = area.directory.join(MARKER_NAME);
        loop {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&marker_path)
            {
                Ok(mut file) => {
                    let pid = std::process::id();
                    let process = process_identity(pid)?;
                    write_marker(&mut file, &tree, spec, pid, &process)?;
                    return Ok(Self {
                        marker_path,
                        tree,
                        armed: true,
                    });
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    let marker = read_marker(&marker_path)?;
                    if marker.tree != tree {
                        return Err(HarnessError::configuration(format!(
                            "spawn marker `{}` claims tree `{}` but key resolved for `{tree}`",
                            marker_path.display(),
                            marker.tree
                        )));
                    }
                    if marker_matches_live_process(&marker) {
                        let same_execution = marker.workflow_id == spec.workflow_id.to_string()
                            && marker.activity_id == spec.activity_id.to_string();
                        if same_execution {
                            return Ok(Self {
                                marker_path,
                                tree,
                                armed: false,
                            });
                        }
                        return Err(HarnessError::occupied(format!(
                            "spawn refused: live sibling pid {} holds this tree \
                             (workflow {}, activity {})",
                            marker.pid, marker.workflow_id, marker.activity_id
                        )));
                    }
                    match std::fs::remove_file(&marker_path) {
                        Ok(()) => {}
                        Err(error) if error.kind() == ErrorKind::NotFound => {}
                        Err(error) => return Err(marker_io("clean stale", &marker_path, &error)),
                    }
                }
                Err(error) => return Err(marker_io("create", &marker_path, &error)),
            }
        }
    }

    /// Replace the launch reservation atomically with the spawned child's exact
    /// process incarnation. Failure removes the reservation so it cannot wedge
    /// later launches.
    pub(crate) fn bind_child(
        mut self,
        spec: &AgentRunSpec,
        child_pid: u32,
    ) -> Result<(), HarnessError> {
        let process = process_identity(child_pid)?;
        let temporary = self
            .marker_path
            .with_extension(format!("json.{}.tmp", std::process::id()));
        let write_result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|error| marker_io("create replacement for", &temporary, &error))?;
            write_marker(&mut file, &self.tree, spec, child_pid, &process)?;
            std::fs::rename(&temporary, &self.marker_path)
                .map_err(|error| marker_io("replace", &self.marker_path, &error))
        })();
        if write_result.is_err() {
            let _ = std::fs::remove_file(&temporary);
            return write_result;
        }
        self.armed = false;
        Ok(())
    }
}

impl Drop for SpawnClaim {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.marker_path);
        }
    }
}

fn write_marker(
    file: &mut File,
    tree: &str,
    spec: &AgentRunSpec,
    pid: u32,
    process: &ProcessIdentity,
) -> Result<(), HarnessError> {
    let marker = json!({
        "tree": tree,
        "workflow_id": spec.workflow_id.to_string(),
        "activity_id": spec.activity_id.to_string(),
        "pid": pid,
        "process_started": process.started,
        "process_command": process.command,
    });
    serde_json::to_writer(&mut *file, &marker).map_err(|error| {
        HarnessError::configuration(format!("could not encode spawn marker: {error}"))
    })?;
    file.write_all(b"\n").map_err(|error| {
        HarnessError::configuration(format!("could not finish spawn marker: {error}"))
    })?;
    file.sync_all().map_err(|error| {
        HarnessError::configuration(format!("could not sync spawn marker: {error}"))
    })
}

fn read_marker(path: &Path) -> Result<Marker, HarnessError> {
    let bytes = std::fs::read(path).map_err(|error| marker_io("read", path, &error))?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
        HarnessError::configuration(format!(
            "spawn marker `{}` is malformed and cannot be trusted: {error}",
            path.display()
        ))
    })?;
    let field = |name: &str| {
        value
            .get(name)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                HarnessError::configuration(format!(
                    "spawn marker `{}` has no valid `{name}` claim",
                    path.display()
                ))
            })
    };
    let pid = value
        .get("pid")
        .and_then(Value::as_u64)
        .and_then(|pid| u32::try_from(pid).ok())
        .ok_or_else(|| {
            HarnessError::configuration(format!(
                "spawn marker `{}` has no valid `pid` claim",
                path.display()
            ))
        })?;
    Ok(Marker {
        tree: field("tree")?,
        workflow_id: field("workflow_id")?,
        activity_id: field("activity_id")?,
        pid,
        process: ProcessIdentity {
            started: field("process_started")?,
            command: field("process_command")?,
        },
    })
}

fn marker_matches_live_process(marker: &Marker) -> bool {
    if !signal_zero(marker.pid) {
        return false;
    }
    process_identity(marker.pid).is_ok_and(|identity| {
        identity.started == marker.process.started && identity.command == marker.process.command
    })
}

fn signal_zero(pid: u32) -> bool {
    let Ok(raw_pid) = i32::try_from(pid) else {
        return false;
    };
    match kill(Pid::from_raw(raw_pid), None) {
        Ok(()) | Err(Errno::EPERM) => true,
        Err(_) => false,
    }
}

fn process_identity(pid: u32) -> Result<ProcessIdentity, HarnessError> {
    Ok(ProcessIdentity {
        started: ps_field(pid, "lstart")?,
        command: ps_field(pid, "command")?,
    })
}

fn ps_field(pid: u32, field: &str) -> Result<String, HarnessError> {
    let output = Command::new("/bin/ps")
        .args(["-p", &pid.to_string(), "-o", &format!("{field}=")])
        .output()
        .map_err(|error| {
            HarnessError::configuration(format!(
                "could not inspect spawn-marker pid {pid} with signal-0 companion check: {error}"
            ))
        })?;
    if !output.status.success() {
        return Err(HarnessError::configuration(format!(
            "spawn-marker pid {pid} disappeared before its process identity could be verified"
        )));
    }
    let value = String::from_utf8(output.stdout).map_err(|error| {
        HarnessError::configuration(format!(
            "spawn-marker pid {pid} returned non-UTF-8 process identity: {error}"
        ))
    })?;
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(HarnessError::configuration(format!(
            "spawn-marker pid {pid} returned an empty `{field}` identity"
        )));
    }
    Ok(value)
}

fn marker_io(action: &str, path: &Path, error: &std::io::Error) -> HarnessError {
    HarnessError::configuration(format!(
        "could not {action} spawn marker `{}`: {error}",
        path.display()
    ))
}

#[cfg(test)]
#[path = "spawn_guard_tests.rs"]
mod tests;
