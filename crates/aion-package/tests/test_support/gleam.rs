//! The one runtime gate for every `aion-package` test that shells out to the
//! real `gleam` CLI.
//!
//! WHY THIS FILE EXISTS (#74). Three tests in this crate gated themselves on
//! `gleam` being installed and announced the skip in a way nothing ever saw:
//! two through `eprintln!` and one through `tracing::info!`. Both are invisible
//! for a test that PASSES. `libtest` captures the `print!`/`eprintln!` macros
//! and replays the capture only when a test fails, and `tracing` with no
//! subscriber installed writes nowhere at all. So the skip line existed, was
//! correct, and could never be read: the suite reported `ok` and proved
//! nothing, which is the exact vacuity these gates were written to avoid.
//!
//! [`announce`] writes to the raw `io::stderr()` handle rather than through the
//! macro, which is not routed through `libtest`'s capture and therefore lands
//! in a plain `cargo test` log next to the `ok`. That mechanism is the whole
//! point of this file and is pinned by
//! `the_skip_is_visible_in_a_passing_run` in `golden_gleam_exec.rs`.
//!
//! It is ONE definition, included by all three call sites — including the unit
//! test inside `src/`, which reaches it through a `#[path]` in
//! `src/structure/mod.rs`. An invariant with several independent enforcers and
//! no shared source of truth diverges silently, and "the skip must be visible"
//! is exactly that kind of invariant: every copy looks right in isolation.

use std::io::{self, Write};
use std::panic::Location;
use std::process::Command;

/// The head of the announcement, and the string the visibility pin greps for.
///
/// It says PROVES NOTHING rather than "skipped" because the failure being
/// guarded is a reader who sees `ok` and concludes the gate ran.
pub(crate) const SKIP_MARKER: &str = "SKIPPED (PROVES NOTHING)";

/// True when the `gleam` CLI is unavailable and the caller should return early
/// — a runtime gate, never `#[ignore]`, which the house rules ban outright.
///
/// The skip is announced through [`announce`], so it is visible in a plain
/// `cargo test` log and cannot be confused with a pass. The caller's own file
/// and line are named, so a reader can go straight to the test that did not
/// run rather than to this shared helper.
#[must_use]
#[track_caller]
pub(crate) fn skip_if_unavailable() -> bool {
    if Command::new("gleam")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        return false;
    }

    let location = Location::caller();
    announce(&format!(
        "{SKIP_MARKER}: `gleam` is not available on PATH, so the Gleam-dependent test at {}:{} \
         compiled nothing",
        location.file(),
        location.line()
    ));
    true
}

/// Write `message` where a plain `cargo test` run will show it.
///
/// The raw handle is deliberate: `eprintln!` would be captured by `libtest` and
/// discarded for a passing test, which is the defect this file exists to close.
fn announce(message: &str) {
    let mut stderr = io::stderr();
    // An announcement failing must not fail the test, but it must not be
    // silent either: fall back to the captured stream.
    if writeln!(stderr, "{message}")
        .and_then(|()| stderr.flush())
        .is_err()
    {
        println!("{message}");
    }
}
