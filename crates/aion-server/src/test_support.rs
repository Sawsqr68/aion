//! Test-only filesystem and tracing helpers.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use tempfile::TempDir;
use tracing_subscriber::fmt::MakeWriter;

/// A `tracing` writer that accumulates formatted events in memory, so a test
/// can assert on what the server actually told the operator.
#[derive(Clone, Default)]
pub(crate) struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

/// The per-event writer handed out by [`CapturedLogs`].
pub(crate) struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CapturedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut bytes = self
            .0
            .lock()
            .map_err(|_| io::Error::other("captured log lock poisoned"))?;
        bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'writer> MakeWriter<'writer> for CapturedLogs {
    type Writer = CapturedWriter;

    fn make_writer(&'writer self) -> Self::Writer {
        CapturedWriter(Arc::clone(&self.0))
    }
}

impl CapturedLogs {
    /// Run `body` with every `tracing` event routed into this buffer.
    ///
    /// The subscriber is thread-local (`with_default`), so a parallel test
    /// harness cannot cross-contaminate two captures.
    pub(crate) fn capture<T>(body: impl FnOnce() -> T) -> (Self, T) {
        let captured = Self::default();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(captured.clone())
            .finish();
        let value = tracing::subscriber::with_default(subscriber, body);
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
