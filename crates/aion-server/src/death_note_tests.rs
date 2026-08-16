//! Death-note tests: one sequential test drives every observable path — the
//! global panic hook and signal registrations are process-wide, so a single
//! ordered test avoids cross-test interference by construction.
#![cfg(unix)]
#![allow(clippy::expect_used)]

use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::{DeathNote, FatalAction};

fn note_content(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// Wait for `needle` to appear in the note. Signal delivery is asynchronous
/// (kernel → watcher thread → file), so the completion EVENT is the entry
/// itself; this polls for that event under a generous deadline rather than
/// sleeping and hoping.
fn wait_for_entry(path: &Path, needle: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let content = note_content(path);
        if content.contains(needle) {
            return content;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for note entry `{needle}`; note so far:\n{content}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn the_death_note_records_every_observable_path_once_armed() {
    let home = tempfile::tempdir().expect("create a temporary Aion home");
    let (fatal_tx, fatal_rx) = mpsc::channel();
    let recorder: FatalAction = Box::new(move |signal| {
        // A send failure means the receiver is gone, which only the test's
        // own teardown can cause; nothing to propagate from a signal thread.
        let _ = fatal_tx.send(signal);
    });
    let note = DeathNote::arm_with_fatal_action(home.path(), Some(recorder))
        .expect("arm the death note under the temporary home");
    let path = note.path().to_path_buf();

    // The bracket opens with the process identity.
    let content = note_content(&path);
    assert!(
        content.contains("ARMED pid="),
        "the note must open with an ARMED line, got:\n{content}"
    );

    // A second arm in the same process is refused, not doubled.
    let second = DeathNote::arm(home.path());
    assert!(
        matches!(second, Err(crate::ServerError::DeathNote { .. })),
        "a second arm must refuse with the DeathNote error"
    );

    // A panic is recorded with payload and location, and is survivable.
    let unwound = std::panic::catch_unwind(|| panic!("death-note test panic"));
    assert!(unwound.is_err(), "the probe panic must actually unwind");
    let content = wait_for_entry(&path, "PANIC");
    assert!(
        content.contains("death-note test panic"),
        "the PANIC entry must carry the payload, got:\n{content}"
    );
    assert!(
        content.contains("death_note_tests.rs"),
        "the PANIC entry must carry the location, got:\n{content}"
    );

    // SIGTERM is observed only: an entry, and NO fatal action.
    signal_hook::low_level::raise(signal_hook::consts::SIGTERM).expect("raise SIGTERM");
    wait_for_entry(&path, "SIGNAL SIGTERM observed");
    assert!(
        fatal_rx.try_recv().is_err(),
        "SIGTERM must never reach the fatal action; the graceful drain owns it"
    );

    // SIGHUP takes the fatal arm: entry first, then the (injected) action.
    signal_hook::low_level::raise(signal_hook::consts::SIGHUP).expect("raise SIGHUP");
    let delivered = fatal_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the fatal action must be invoked for SIGHUP");
    assert_eq!(delivered, signal_hook::consts::SIGHUP);
    wait_for_entry(&path, "SIGNAL SIGHUP received; terminating");

    // Disarm closes the bracket with the caller's reason…
    note.disarm("test disarm: clean exit");
    let content = wait_for_entry(&path, "DISARMED test disarm: clean exit");
    let settled_len = content.len();

    // …and gates the hook off: a later panic writes nothing.
    let unwound = std::panic::catch_unwind(|| panic!("post-disarm panic must not be recorded"));
    assert!(unwound.is_err(), "the post-disarm probe panic must unwind");
    let content = note_content(&path);
    assert_eq!(
        content.len(),
        settled_len,
        "a disarmed note must not accept panic entries, got:\n{content}"
    );
}
