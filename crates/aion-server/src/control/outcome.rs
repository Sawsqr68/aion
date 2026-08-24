//! The shutdown outcome record: how the drain's rich result crosses the
//! process boundary.
//!
//! The process exit code deliberately collapses the drain outcome (#207:
//! `Parked` exits success because parked state is recoverable by design), so
//! the dying server writes its full [`OutcomeRecord`] into the death note —
//! the file that already owns "what ended this process" — as one `OUTCOME`
//! entry, and `aion server stop` reads it back after the process is gone.
//! One file, one writer, no second channel; the note's line-oriented format
//! means readers of the existing entries are untouched by the new kind.
//!
//! A server that dies without writing the record (`kill -9`, a panic before
//! the drain) leaves an ARMED bracket with no `OUTCOME` and no `DISARMED`;
//! the reader reports exactly that shape and never fabricates a drain
//! summary from it.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ServerError;
use crate::shutdown::ShutdownOutcome;

/// One worker's parked in-flight work at drain timeout, by name.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ParkedWorkerRecord {
    /// Registry id of the worker whose tasks were parked.
    pub worker: String,
    /// Task queue the worker was serving, when the registry still knew it.
    pub queue: Option<String>,
    /// The parked activities as `workflow/activity#attempt` names.
    pub tasks: Vec<String>,
}

/// The durable record of one shutdown's drain outcome.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OutcomeRecord {
    /// Pid of the server that wrote the record, tying it to an ARMED bracket.
    pub pid: u32,
    /// Which face the drain ended on.
    pub outcome: ShutdownOutcome,
    /// The drain window that governed the wait, in seconds.
    pub drain_timeout_seconds: u64,
    /// How many connected workers received the drain request.
    pub delivered_drain_requests: usize,
    /// Parked in-flight work, by worker and task name. Empty on a clean
    /// drain.
    pub parked: Vec<ParkedWorkerRecord>,
    /// Declared-body commands still executing when the drain ended, as
    /// `workflow/activity#attempt` names: server-run commands no worker holds,
    /// ended with the server and re-dispatched by the next boot's replay.
    /// Defaulted on read because records written before the declared census
    /// existed — the death note is shared with older builds — carry no field,
    /// and their honest reading is "none named", not a parse refusal.
    #[serde(default)]
    pub parked_declared_commands: Vec<String>,
    /// Managed workers proven stopped (process group empty) at shutdown.
    pub managed_workers_stopped: Vec<String>,
    /// Managed workers that could NOT be proven stopped, by name — never
    /// summed into a count that could read as calm.
    pub managed_workers_unstopped: Vec<String>,
}

impl OutcomeRecord {
    /// Build the record from what the drain observed, named where the
    /// observation was made: workers by registry id, parked tasks as
    /// `workflow/activity#attempt`.
    #[must_use]
    pub fn from_report(pid: u32, report: &crate::shutdown::ShutdownReport) -> Self {
        Self {
            pid,
            outcome: report.outcome,
            drain_timeout_seconds: report.drain_timeout.as_secs(),
            delivered_drain_requests: report.delivered_drain_requests,
            parked: report
                .parked
                .iter()
                .map(|lost| ParkedWorkerRecord {
                    worker: lost.worker_id.value().to_string(),
                    queue: lost.task_queue.clone(),
                    tasks: lost
                        .tasks
                        .iter()
                        .map(|task| {
                            format!("{}/{}#{}", task.workflow_id, task.activity_id, task.attempt)
                        })
                        .collect(),
                })
                .collect(),
            parked_declared_commands: report
                .parked_declared
                .iter()
                .map(|key| format!("{}/{}#{}", key.workflow_id, key.activity_id, key.attempt))
                .collect(),
            managed_workers_stopped: report.managed_workers_stopped.clone(),
            managed_workers_unstopped: report.managed_workers_unstopped.clone(),
        }
    }
}

/// The entry keyword the record is written under in the death note.
const OUTCOME_KIND: &str = "OUTCOME";

/// Render the record as the death-note entry body (`OUTCOME <one-line json>`).
///
/// # Errors
///
/// Returns [`ServerError::DeathNote`] when the record cannot be serialized —
/// which the caller reports and survives: failing the shutdown over its own
/// receipt would invert the receipt's purpose.
pub fn render_entry(record: &OutcomeRecord) -> Result<String, ServerError> {
    let json = serde_json::to_string(record).map_err(|serialize_error| ServerError::DeathNote {
        message: format!("cannot serialize the shutdown outcome record: {serialize_error}"),
    })?;
    Ok(format!("{OUTCOME_KIND} {json}"))
}

/// What the death note says about the server incarnation that wore `pid`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NoteFate {
    /// No death note exists under this home — nothing was ever armed here
    /// (or the home predates the death-note era).
    NoNote,
    /// A note exists but holds no ARMED bracket for this pid.
    NoBracketForPid,
    /// The pid's last bracket is ARMED with no DISARMED: the process was
    /// destroyed without the run loop seeing it (`SIGKILL` class) — or is
    /// still running. The caller knows which, from its own liveness probe.
    ArmedNotDisarmed {
        /// The outcome record, if the drain got far enough to write one
        /// before the process was destroyed.
        outcome: Option<OutcomeRecord>,
        /// An `OUTCOME` line that is PRESENT in the bracket but that this
        /// binary could not parse — a torn line, or a record written by a
        /// build whose schema this one cannot read. Distinct from `outcome:
        /// None` because "the drain never wrote a record" and "the drain
        /// wrote a record I cannot read" are different facts, and reporting
        /// the second as the first sends the operator down the wrong path.
        outcome_unreadable: Option<String>,
    },
    /// The bracket closed by ordinary control flow.
    Disarmed {
        /// The drain's recorded outcome; `None` when the exit wrote no
        /// record (an error return before serving, or a pre-record binary),
        /// reported as exactly that absence.
        outcome: Option<OutcomeRecord>,
        /// An unparseable `OUTCOME` line in the bracket — the unreadable
        /// presence, kept distinct from the honest absence exactly as on
        /// [`NoteFate::ArmedNotDisarmed`].
        outcome_unreadable: Option<String>,
        /// The DISARMED line's reason text, for the report.
        reason: String,
    },
}

/// Read the death note under `home` and classify what it records for `pid`.
///
/// # Errors
///
/// Returns [`ServerError::DeathNote`] when the note exists but cannot be read.
/// A malformed line inside the note is skipped, never fatal: the note is an
/// append-only log a panicking process writes into, and one torn line must
/// not make every other entry unreadable.
pub fn read_fate(home: &Path, pid: u32) -> Result<NoteFate, ServerError> {
    let path = crate::death_note::note_path(home);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(io_error) if io_error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(NoteFate::NoNote);
        }
        Err(io_error) => {
            return Err(ServerError::DeathNote {
                message: format!(
                    "cannot read the death note `{}`: {io_error}",
                    path.display()
                ),
            });
        }
    };
    let armed_marker = format!("ARMED pid={pid} ");
    let mut bracket_start: Option<usize> = None;
    let lines: Vec<&str> = content.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        if entry_body(line).is_some_and(|body| body.starts_with(&armed_marker)) {
            bracket_start = Some(index);
        }
    }
    let Some(start) = bracket_start else {
        return Ok(NoteFate::NoBracketForPid);
    };
    let mut outcome: Option<OutcomeRecord> = None;
    let mut outcome_unreadable: Option<String> = None;
    let mut disarmed_reason: Option<String> = None;
    for line in &lines[start + 1..] {
        let Some(body) = entry_body(line) else {
            continue;
        };
        if let Some(json) = body.strip_prefix("OUTCOME ") {
            // A record for another pid inside this bracket would mean a
            // second writer (the note's ARMED_ONCE guard makes it
            // unreachable in practice) and is ignored. A line that does not
            // parse — torn by a kill mid-write, or written by a build whose
            // schema this binary cannot read — is carried as the UNREADABLE
            // presence, never collapsed into "no record": the operator told
            // "the drain never got far enough to write one" over a record
            // that exists is debugging the wrong incident.
            // Both facts are kept in BOTH orders: a torn line beside a
            // readable record is stated by the renderers ("a readable
            // record does not un-happen the torn line beside it"), and the
            // reader must not pre-empt that rule by erasing whichever fact
            // arrived first.
            match serde_json::from_str::<OutcomeRecord>(json) {
                Ok(record) if record.pid == pid => outcome = Some(record),
                Ok(_other_pid) => {}
                Err(parse_error) => {
                    outcome_unreadable =
                        Some(format!("an OUTCOME line does not parse: {parse_error}"));
                }
            }
        } else if let Some(reason) = body.strip_prefix("DISARMED ") {
            disarmed_reason = Some(reason.to_owned());
            break;
        } else if body.starts_with("ARMED pid=") {
            // A newer incarnation's bracket begins; ours never closed.
            break;
        }
    }
    Ok(match disarmed_reason {
        Some(reason) => NoteFate::Disarmed {
            outcome,
            outcome_unreadable,
            reason,
        },
        None => NoteFate::ArmedNotDisarmed {
            outcome,
            outcome_unreadable,
        },
    })
}

/// Strip the leading RFC 3339 timestamp from a note line, returning the
/// entry body. `None` for lines that do not carry a timestamped entry.
fn entry_body(line: &str) -> Option<&str> {
    let (timestamp, body) = line.split_once(' ')?;
    // Cheap shape check, not a full parse: every writer-emitted line starts
    // with an RFC 3339 instant, and a torn line must simply be skipped.
    if timestamp.len() >= 20
        && timestamp
            .chars()
            .take(4)
            .all(|character| character.is_ascii_digit())
    {
        Some(body)
    } else {
        None
    }
}

#[cfg(test)]
#[path = "outcome_tests.rs"]
mod tests;
