//! The server's death note: a durable record of what is killing the process
//! on paths that never reach the graceful-shutdown log.
//!
//! Two production servers died on 2026-08-16 (pids 52844 and 2681) with
//! identical faces: the main log ends mid-flight on routine lines — no drain,
//! no panic output, no OS crash report. This module exists so the third such
//! death is a diagnosis instead of a mystery. It arms three recorders around
//! the run loop, writing to `<AION_HOME>/logs/aion-server.death.log`:
//!
//! - an **armed/disarmed bracket**: one line when the server boots, one line
//!   when the run scope exits by ordinary control flow (clean shutdown or an
//!   error return, written by [`DeathNote`]'s `Drop`). A note that is armed
//!   but never disarmed, with no other entry, means the process was destroyed
//!   without the run loop seeing it — the `SIGKILL` / raw-`_exit` class no
//!   in-process handler can observe.
//! - a **panic hook**, chained in front of the previously installed hook,
//!   that records every panic's thread, location, payload, and backtrace. A
//!   panic in a spawned task can be survivable; the entry says so. Death by
//!   panic reads as a `PANIC` entry followed by the drop-written `DISARMED`
//!   line as the unwind leaves the run scope.
//! - a **termination-signal watcher** for the catchable signals whose default
//!   action kills the process silently (`SIGHUP`, `SIGQUIT`, `SIGUSR1`,
//!   `SIGUSR2`): the note records the signal, then re-applies the signal's
//!   default action so process behaviour is unchanged — the death is noted,
//!   not prevented. `SIGTERM` and `SIGINT` are recorded as observations only:
//!   the graceful drain in [`crate::run`] owns those two, and both watchers
//!   coexist because tokio and this module register through the same
//!   `signal-hook-registry` chain.
//!
//! NOT covered, stated plainly: `SIGKILL`/`SIGSTOP` (uncatchable by kernel
//! contract); the fatal-fault signals (`SIGSEGV`/`SIGBUS`/`SIGILL`/`SIGFPE`)
//! and raw `abort()`, whose handlers must run in async-signal context and
//! therefore require `unsafe` this workspace denies (the OS crash reporter
//! remains the observer for that class — both 2026-08-16 corpses left no
//! crash report, which is itself evidence against that class); and a direct
//! `exit()` that bypasses the run scope. With the note armed, each of those
//! reads as ARMED-without-DISARMED plus the absence of every entry above — a
//! narrow, named remainder instead of an anonymous death.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use tracing::{error, info, warn};

use crate::ServerError;

/// One death note per process: the panic hook and signal registrations are
/// process-global, so a second armed instance would double-write every entry.
static ARMED_ONCE: AtomicBool = AtomicBool::new(false);

/// File name of the death note inside the Aion home's `logs/` directory.
const NOTE_FILE_NAME: &str = "aion-server.death.log";

/// What the signal watcher does after noting a signal whose default action
/// terminates the process. Production re-applies the default action (the
/// process dies exactly as it would have, but on the record); tests inject a
/// recorder so the suite is not killed by its own probe.
type FatalAction = Box<dyn Fn(i32) + Send + Sync>;

/// The armed death note. Constructed by [`DeathNote::arm`] early in the
/// server run loop; its `Drop` writes the `DISARMED` line, so any ordinary
/// exit from the run scope — clean shutdown, error return, or a panic
/// unwinding through it — closes the bracket on the record.
pub struct DeathNote {
    note: Arc<NoteFile>,
    armed: Arc<AtomicBool>,
    #[cfg(unix)]
    signals_handle: signal_hook::iterator::Handle,
    disarm_reason: Option<String>,
}

impl std::fmt::Debug for DeathNote {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeathNote")
            .field("path", &self.note.path)
            .field("armed", &self.armed.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

impl DeathNote {
    /// Arm the death note under the resolved Aion home: create
    /// `<home>/logs/`, open the note file append-only, write the `ARMED`
    /// line, install the panic hook, and start the signal watcher.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::DeathNote`] when a note is already armed in
    /// this process, when the logs directory or note file cannot be created,
    /// or when the signal watcher cannot be installed. Arming failures are
    /// fatal to boot by design: a server that cannot record its own death is
    /// exactly the server this module exists for.
    pub fn arm(home: &Path) -> Result<Self, ServerError> {
        Self::arm_with_fatal_action(home, None)
    }

    /// [`DeathNote::arm`] with an injectable fatal action. `None` selects the
    /// production action (re-apply the signal's default, terminating the
    /// process); tests inject a recorder so the probe signal does not kill
    /// the test binary. The seam sits on the production path — the injected
    /// closure replaces only the final kill, never the note write.
    fn arm_with_fatal_action(
        home: &Path,
        fatal_action: Option<FatalAction>,
    ) -> Result<Self, ServerError> {
        if ARMED_ONCE
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(death_note_error(
                "a death note is already armed in this process; the panic hook and \
                 signal registrations are process-global and must not be doubled",
            ));
        }
        let logs_dir = home.join("logs");
        std::fs::create_dir_all(&logs_dir).map_err(|source| {
            death_note_error(format!(
                "could not create logs directory `{}`: {source}",
                logs_dir.display()
            ))
        })?;
        let path = logs_dir.join(NOTE_FILE_NAME);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|source| {
                death_note_error(format!(
                    "could not open death note `{}`: {source}",
                    path.display()
                ))
            })?;
        let note = Arc::new(NoteFile {
            path,
            file: Mutex::new(file),
        });
        let build = crate::build_identity::BuildIdentity::current();
        note.write_entry(&format!(
            "ARMED pid={} version={} build={}",
            std::process::id(),
            env!("CARGO_PKG_VERSION"),
            build.line(),
        ));
        let armed = Arc::new(AtomicBool::new(true));
        install_panic_hook(Arc::clone(&note), Arc::clone(&armed));
        #[cfg(unix)]
        let signals_handle = {
            let action =
                fatal_action.unwrap_or_else(|| terminate_with_default_action(Arc::clone(&note)));
            spawn_signal_watcher(Arc::clone(&note), Arc::clone(&armed), action)?
        };
        #[cfg(not(unix))]
        drop(fatal_action);
        info!(path = %note.path.display(), "death note armed");
        Ok(Self {
            note,
            armed,
            #[cfg(unix)]
            signals_handle,
            disarm_reason: None,
        })
    }

    /// Path of the note file, for the startup banner and operator docs.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.note.path
    }

    /// Close the bracket explicitly with the run loop's own account of how it
    /// ended. Consumes the note; the `DISARMED` line is written by `Drop`.
    pub fn disarm(mut self, reason: &str) {
        self.disarm_reason = Some(reason.to_owned());
    }
}

impl Drop for DeathNote {
    fn drop(&mut self) {
        self.armed.store(false, Ordering::SeqCst);
        #[cfg(unix)]
        self.signals_handle.close();
        let reason = self.disarm_reason.as_deref().unwrap_or(
            "run scope exited without an explicit disarm (an error return, or an unwind — \
             see any PANIC entry directly above)",
        );
        self.note.write_entry(&format!("DISARMED {reason}"));
        info!(path = %self.note.path.display(), reason, "death note disarmed");
    }
}

/// The note file plus its path, shared by the run scope, the panic hook, and
/// the signal watcher thread.
struct NoteFile {
    path: PathBuf,
    file: Mutex<File>,
}

impl NoteFile {
    /// Append one timestamped entry and force it to disk. A poisoned lock is
    /// recovered and written through — the writer that poisoned it was a
    /// panicking thread, which is precisely when this file must still accept
    /// entries. A write or sync failure falls back to stderr so the entry is
    /// never silently lost while the process can still say anything at all.
    fn write_entry(&self, entry: &str) {
        let line = format!("{} {entry}\n", chrono::Utc::now().to_rfc3339());
        let mut file = self.file.lock().unwrap_or_else(PoisonError::into_inner);
        let written = file
            .write_all(line.as_bytes())
            .and_then(|()| file.flush())
            .and_then(|()| file.sync_all());
        if let Err(io_error) = written {
            eprintln!(
                "death note write to `{}` failed ({io_error}); the entry was: {line}",
                self.path.display()
            );
        }
    }
}

/// Build the [`ServerError`] for an arming failure.
fn death_note_error(message: impl Into<String>) -> ServerError {
    ServerError::DeathNote {
        message: message.into(),
    }
}

/// Install the panic hook, chained IN FRONT of the previously installed hook
/// so default stderr reporting (and anything a test harness installed) still
/// runs after the note is on disk.
fn install_panic_hook(note: Arc<NoteFile>, armed: Arc<AtomicBool>) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info: &PanicHookInfo<'_>| {
        if armed.load(Ordering::SeqCst) {
            note.write_entry(&panic_entry(panic_info));
        }
        previous(panic_info);
    }));
}

/// Render a panic into one note entry: thread, location, payload, and the
/// full backtrace (force-captured — by the time this runs, cost is moot).
fn panic_entry(panic_info: &PanicHookInfo<'_>) -> String {
    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("<unnamed>").to_owned();
    let location = panic_info
        .location()
        .map_or_else(|| "<unknown location>".to_owned(), ToString::to_string);
    let payload: &str = panic_info
        .payload()
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| {
            panic_info
                .payload()
                .downcast_ref::<String>()
                .map(String::as_str)
        })
        .unwrap_or("<non-string panic payload>");
    let backtrace = std::backtrace::Backtrace::force_capture();
    format!(
        "PANIC thread={thread_name} location={location} payload={payload} \
         (a panic in a spawned task can be survivable; death by panic is this entry \
         followed by a DISARMED line as the unwind leaves the run scope)\n{backtrace}"
    )
}

/// The production fatal action: re-apply the signal's default disposition so
/// the process terminates exactly as it would have without the watcher. If
/// the default action somehow fails to terminate, that failure is itself
/// noted and the process exits loudly with the conventional `128 + signal`
/// code rather than continuing in an undefined disposition.
#[cfg(unix)]
fn terminate_with_default_action(note: Arc<NoteFile>) -> FatalAction {
    Box::new(move |signal| {
        if let Err(io_error) = signal_hook::low_level::emulate_default_handler(signal) {
            note.write_entry(&format!(
                "SIGNAL-DEFAULT-FAILED signal={} error={io_error}; exiting {} instead",
                signal_label(signal),
                128 + signal,
            ));
            std::process::exit(128 + signal);
        }
    })
}

/// Spawn the signal watcher thread over the catchable termination set.
///
/// `SIGTERM`/`SIGINT` entries are observations (the run loop's graceful drain
/// owns the response); `SIGHUP`/`SIGQUIT`/`SIGUSR1`/`SIGUSR2` had a default
/// disposition of silent process death, so those are noted, synced, and then
/// handed to `fatal_action`. Registration goes through `signal-hook`'s
/// registry, which chains with tokio's own listeners rather than replacing
/// them.
#[cfg(unix)]
fn spawn_signal_watcher(
    note: Arc<NoteFile>,
    armed: Arc<AtomicBool>,
    fatal_action: FatalAction,
) -> Result<signal_hook::iterator::Handle, ServerError> {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM, SIGUSR1, SIGUSR2};

    let mut signals =
        signal_hook::iterator::Signals::new([SIGHUP, SIGINT, SIGQUIT, SIGTERM, SIGUSR1, SIGUSR2])
            .map_err(|source| {
            death_note_error(format!("could not register the signal watcher: {source}"))
        })?;
    let handle = signals.handle();
    std::thread::Builder::new()
        .name("aion-death-note".to_owned())
        .spawn(move || {
            for signal in &mut signals {
                if !armed.load(Ordering::SeqCst) {
                    continue;
                }
                let label = signal_label(signal);
                if signal == SIGTERM || signal == SIGINT {
                    note.write_entry(&format!(
                        "SIGNAL {label} observed; the graceful drain owns the response"
                    ));
                    warn!(signal = label, "death note observed a termination signal");
                } else {
                    note.write_entry(&format!(
                        "SIGNAL {label} received; terminating with the signal's default action"
                    ));
                    error!(
                        signal = label,
                        "death note recorded a fatal signal; applying its default action"
                    );
                    fatal_action(signal);
                }
            }
        })
        .map_err(|source| {
            death_note_error(format!(
                "could not spawn the signal watcher thread: {source}"
            ))
        })?;
    Ok(handle)
}

/// Human-readable signal name (`SIGHUP`), falling back to the raw number for
/// anything the platform table does not name.
#[cfg(unix)]
fn signal_label(signal: i32) -> String {
    signal_hook::low_level::signal_name(signal)
        .map_or_else(|| format!("signal {signal}"), ToOwned::to_owned)
}

#[cfg(test)]
#[path = "death_note_tests.rs"]
mod tests;
