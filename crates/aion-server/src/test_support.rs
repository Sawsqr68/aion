//! Test-only filesystem and tracing helpers.

use std::cell::RefCell;
use std::io::{self, Write};
use std::sync::{Arc, Mutex, Once};

use tempfile::TempDir;

/// A `tracing` writer that accumulates formatted events in memory, so a test
/// can assert on what the server actually told the operator.
#[derive(Clone, Default)]
pub(crate) struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

thread_local! {
    /// The buffer the CURRENT thread's capture routes events into, when one
    /// is active. Events on threads with no active capture are dropped.
    static ACTIVE_SINK: RefCell<Option<Arc<Mutex<Vec<u8>>>>> = const { RefCell::new(None) };
}

/// One-time installer for the process-wide capture subscriber.
static INSTALL_CAPTURE_SUBSCRIBER: Once = Once::new();

/// Why the capture subscriber could not be installed, when it could not.
/// Every subsequent capture writes this into its own buffer so the failure
/// is loud in the assertion that would otherwise see silence.
static INSTALL_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// The per-event writer: forwards to the current thread's active sink.
pub(crate) struct CapturedWriter;

impl Write for CapturedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        ACTIVE_SINK.with(|sink| {
            if let Some(bytes) = sink.borrow().as_ref() {
                let mut bytes = bytes
                    .lock()
                    .map_err(|_| io::Error::other("captured log lock poisoned"))?;
                bytes.extend_from_slice(buffer);
            }
            Ok(buffer.len())
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Clears the thread's active sink when a capture ends, even on panic.
struct SinkGuard;

impl Drop for SinkGuard {
    fn drop(&mut self) {
        ACTIVE_SINK.with(|sink| sink.borrow_mut().take());
    }
}

impl CapturedLogs {
    /// Run `body` with every `tracing` event emitted ON THIS THREAD routed
    /// into this buffer.
    ///
    /// Mechanically this is ONE process-wide subscriber (installed on first
    /// use) writing through a thread-local sink, not a scoped `with_default`
    /// per capture. The scoped form registers and drops a dispatcher per
    /// capture, and every registration rebuilds `tracing`'s global
    /// per-callsite interest cache — a rebuild landing on another thread
    /// mid-event silently drops that event, which surfaced as intermittent,
    /// schedule-dependent misses of log assertions across the parallel test
    /// harness. A single permanent subscriber keeps callsite interest stable
    /// forever, so no capture can lose an event to another capture's
    /// lifecycle. Events on OTHER threads (including ones `body` spawns) are
    /// not captured — the same visibility the scoped form had.
    pub(crate) fn capture<T>(body: impl FnOnce() -> T) -> (Self, T) {
        INSTALL_CAPTURE_SUBSCRIBER.call_once(|| {
            let subscriber = tracing_subscriber::fmt()
                .without_time()
                .with_ansi(false)
                .with_writer(|| CapturedWriter)
                .finish();
            if let Err(error) = tracing::subscriber::set_global_default(subscriber) {
                // Loud, not silent: a foreign global subscriber means every
                // capture in this process would assert against an empty
                // buffer. No test process installs one before a capture; if
                // that ever changes, every capture assertion must fail
                // naming the cause, so record it for every capture to carry.
                if let Ok(mut slot) = INSTALL_ERROR.lock() {
                    *slot = Some(format!("capture subscriber not installed: {error}"));
                }
            }
        });
        let captured = Self::default();
        if let Some(reason) = INSTALL_ERROR
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_deref()
        {
            if let Ok(mut bytes) = captured.0.lock() {
                bytes.extend_from_slice(reason.as_bytes());
            }
        }
        ACTIVE_SINK.with(|sink| {
            *sink.borrow_mut() = Some(Arc::clone(&captured.0));
        });
        let guard = SinkGuard;
        let value = body();
        drop(guard);
        (captured, value)
    }

    /// The captured events as text, or an error if a writer panicked mid-write.
    pub(crate) fn text(&self) -> Result<String, Box<dyn std::error::Error>> {
        let bytes = self.0.lock().map_err(|_| "captured log lock poisoned")?;
        Ok(String::from_utf8(bytes.clone())?)
    }
}

/// Tightens an already-created directory to mode `0700` so a test-built
/// workspace root starts from the mode the server would give it, rather than
/// whatever the harness umask happened to produce.
pub(crate) fn make_private(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

/// Creates a temporary directory whose mode is `0700` regardless of the
/// process umask.
///
/// `tempfile::tempdir` inherits the umask, so under a conventional `022` a
/// fresh directory is `0755`. The server would now tighten such a directory on
/// sight, which would silently mutate the fixture a test is asserting against —
/// so tests that hand a temporary directory to any sensitive-root surface go
/// through this helper and start from a known mode under every umask.
pub(crate) fn private_tempdir() -> std::io::Result<TempDir> {
    let dir = tempfile::tempdir()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(dir)
}
