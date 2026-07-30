//! Shared gate and runner for the `aion-awl` tests that shell out to the real
//! `gleam` CLI.
//!
//! Task #71 / R2. Three outcomes were previously fused into one bare
//! `assert!`, so a registry hiccup on a CI runner read as an AWL emitter
//! defect. They are now kept apart:
//!
//! - `gleam` is **absent** — the test skips, and announces the skip loudly
//!   enough that it can never be read as a pass (see [`announce`]).
//! - `gleam` **rejects the generated Gleam** — the test FAILS, named as an
//!   emitter defect. That is the signal these proofs exist for.
//! - `gleam` fails inside its own **dependency layer** (the Hex registry) —
//!   the build is retried a bounded number of times, and if every attempt
//!   fails the test still FAILS, named as an infrastructure failure. It is
//!   never skipped: a fetch failure that skipped would let the proof pass
//!   while proving nothing, which is the exact vacuity this cycle is about.
//!
//! Classification decides **whether to retry** and **what the message calls
//! the failure**. It can never turn a failure into a pass: both branches end
//! in `Err`. That is why an unrecognised failure is classified as an emitter
//! defect — the safe polarity is "blame the code, make a human look".

use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::panic::Location;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::Duration;

/// `gleam`'s own error titles for failures in its dependency layer, taken
/// from the CLI's error set rather than invented: a fetch, a Hex API call, or
/// the unpacking of a downloaded tarball. `Failure opening tar archive` is
/// the one that shadowed the all-features gate on CI run `30192212491`.
///
/// None of these can be produced by a Gleam type or syntax error, so no real
/// emitter defect can land in this set.
const REGISTRY_FAILURE_TITLES: &[&str] = &[
    "Failed to download package",
    "Failure opening tar archive",
    "Hex API failure",
    "HTTP error",
];

/// `gleam`'s title for a failed version solve. Unlike the titles above this
/// one is ambiguous: it covers both an unreachable registry and a genuinely
/// unsatisfiable requirement in the generated `gleam.toml`, so it only counts
/// as infrastructure when the body also names the registry endpoint or a
/// transport failure ([`RESOLUTION_TRANSPORT_MARKERS`]).
const RESOLUTION_FAILURE_TITLE: &str = "Dependency resolution failed";

/// Observed bodies of a resolution failure caused by transport rather than by
/// an unsatisfiable requirement, e.g. `An error occurred while choosing the
/// version of gleam_json: error sending request for url
/// (https://repo.hex.pm/packages/gleam_json)`.
const RESOLUTION_TRANSPORT_MARKERS: &[&str] = &["error sending request", "repo.hex.pm"];

/// Total `gleam build` attempts when a failure is classified as a registry
/// failure. NOT a chosen number: it matches the only bounded-retry precedent
/// for a transient external failure in this repository's test suites,
/// `crates/aion-server/tests/outbox_transport_e2e.rs` (`for attempt in 0..5`,
/// retrying only the transient classification and asserting on the final
/// attempt). A genuine outage still surfaces, and it surfaces named.
const BUILD_ATTEMPTS: u32 = 5;

/// Backoff step: attempt *n* waits `RETRY_BACKOFF_STEP * n`. The step and the
/// linear shape come from the same precedent (`50 * (attempt + 1)` ms) rather
/// than from a guess. Each retried `gleam build` re-does its own network work,
/// which dominates the wall clock here either way.
const RETRY_BACKOFF_STEP: Duration = Duration::from_millis(50);

/// Where `gleam build` leaves compiled Erlang artefacts, relative to a project
/// root: one directory per package, each with an `ebin` holding a `.beam` per
/// compiled module.
const ERLANG_BUILD_ROOT: &str = "build/dev/erlang";

/// True when the `gleam` CLI is unavailable and the caller should return
/// `Ok(())` — a runtime gate, never `#[ignore]`.
///
/// The skip is announced through [`announce`], so it is visible in a plain
/// `cargo test` log and cannot be confused with a pass.
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
        "SKIPPED (PROVES NOTHING): `gleam` is not available on PATH, so the Gleam-dependent test \
         at {}:{} compiled nothing",
        location.file(),
        location.line()
    ));
    true
}

/// Run `gleam build` in `project`, tolerating a transient Hex registry, and
/// prove that every module in `modules` was actually compiled.
///
/// Returns every `ebin` directory the build produced, so callers that load the
/// artefacts can do so without re-walking the tree.
///
/// # Errors
///
/// Fails when `gleam` cannot be executed, when it rejects the generated Gleam,
/// when every attempt dies in the dependency layer, or when it reports success
/// without having produced a `.beam` for one of `modules`. The report names
/// which of those happened.
pub(crate) fn compile_project(
    project: &Path,
    modules: &[&str],
) -> Result<Vec<PathBuf>, GleamBuildFailure> {
    let mut attempt: u32 = 1;
    loop {
        let output = Command::new("gleam")
            .arg("build")
            .current_dir(project)
            .output()
            .map_err(|error| {
                GleamBuildFailure::new(format!(
                    "INFRASTRUCTURE FAILURE, NOT AN AWL DEFECT: the `gleam` binary is required \
                     for the AWL compile proofs and could not be executed for the project at \
                     {}: {error}",
                    project.display()
                ))
            })?;
        if output.status.success() {
            return prove_compiled(project, modules, attempt);
        }
        let failure = classify(&output);
        if matches!(failure, BuildFailure::Registry) && attempt < BUILD_ATTEMPTS {
            announce(&format!(
                "`gleam build` attempt {attempt}/{BUILD_ATTEMPTS} for {} failed in its dependency \
                 layer (the Hex registry), not on the generated Gleam; retrying",
                project.display()
            ));
            thread::sleep(RETRY_BACKOFF_STEP * attempt);
            attempt += 1;
            continue;
        }
        return Err(GleamBuildFailure::new(report(
            project, &failure, attempt, &output,
        )));
    }
}

/// Which layer of `gleam` refused the build.
enum BuildFailure {
    /// `gleam`'s dependency layer: a fetch, a Hex API call, or unpacking a
    /// downloaded tarball. Retryable, and reported as infrastructure.
    Registry,
    /// `gleam` refused the sources themselves. Never retried, and reported as
    /// an emitter defect. Also the classification for anything unrecognised.
    Sources,
}

/// Classify a failed `gleam build` from the text it already printed.
fn classify(output: &Output) -> BuildFailure {
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push('\n');
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    classify_text(&combined)
}

/// [`classify`] over the already-joined output, so the classification is
/// pinnable without fabricating an [`std::process::ExitStatus`].
fn classify_text(combined: &str) -> BuildFailure {
    // Match only on `gleam`'s own `error: <Title>` lines. A source snippet
    // quoted back by a compile error can contain anything, including these
    // words, and must not be mistaken for a registry failure.
    for line in combined.lines() {
        let Some(title) = line.trim_start().strip_prefix("error: ") else {
            continue;
        };
        let title = title.trim();
        if REGISTRY_FAILURE_TITLES.contains(&title) {
            return BuildFailure::Registry;
        }
        if title == RESOLUTION_FAILURE_TITLE
            && RESOLUTION_TRANSPORT_MARKERS
                .iter()
                .any(|marker| combined.contains(marker))
        {
            return BuildFailure::Registry;
        }
    }
    BuildFailure::Sources
}

/// The full failure report, headed by what the failure actually is.
fn report(project: &Path, failure: &BuildFailure, attempts: u32, output: &Output) -> String {
    let headline = match failure {
        BuildFailure::Registry => format!(
            "INFRASTRUCTURE FAILURE, NOT AN AWL DEFECT: `gleam build` failed inside its own \
             dependency layer (the Hex registry) on all {attempts} attempt(s). The emitted Gleam \
             was never compiled, so this run proves nothing about the emitter — fix the \
             environment and run it again."
        ),
        BuildFailure::Sources => format!(
            "AWL EMITTER DEFECT: `gleam` rejected the generated Gleam ({attempts} attempt(s), no \
             dependency-layer failure seen)."
        ),
    };
    format!(
        "{headline}\nproject: {}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        project.display(),
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Prove a successful `gleam build` actually compiled `modules`, and collect
/// the `ebin` directories it produced.
///
/// A zero-exit `gleam build` is not on its own evidence that anything was
/// compiled; the `.beam` files are.
fn prove_compiled(
    project: &Path,
    modules: &[&str],
    attempts: u32,
) -> Result<Vec<PathBuf>, GleamBuildFailure> {
    let packages = project.join(ERLANG_BUILD_ROOT);
    let mut ebins = Vec::new();
    let mut compiled: Vec<String> = Vec::new();
    for entry in fs::read_dir(&packages).map_err(|error| io_failure(&packages, &error))? {
        let package = entry
            .map_err(|error| io_failure(&packages, &error))?
            .path()
            .join("ebin");
        if !package.is_dir() {
            continue;
        }
        for beam in fs::read_dir(&package).map_err(|error| io_failure(&package, &error))? {
            let beam = beam.map_err(|error| io_failure(&package, &error))?;
            let name = beam.file_name();
            if let Some(name) = name.to_str() {
                compiled.push(name.to_owned());
            }
        }
        ebins.push(package);
    }
    for module in modules {
        let beam = format!("{module}.beam");
        if !compiled.iter().any(|compiled| compiled == &beam) {
            return Err(GleamBuildFailure::new(format!(
                "`gleam build` reported success but produced no `{beam}` under {}: this proof \
                 cannot pass without having compiled the emitted module",
                packages.display()
            )));
        }
    }
    announce(&format!(
        "COMPILED {} module(s) under real `gleam build` in {attempts} attempt(s): {}",
        modules.len(),
        modules.join(", ")
    ));
    Ok(ebins)
}

/// Wrap a filesystem error against the build tree, so no `io::Error` is
/// swallowed while proving the artefacts.
fn io_failure(path: &Path, error: &io::Error) -> GleamBuildFailure {
    GleamBuildFailure::new(format!(
        "could not read the `gleam build` output tree at {} to prove the emitted modules were \
         compiled: {error}",
        path.display()
    ))
}

/// Write a line to the process's real stderr handle.
///
/// `libtest` captures the `print!`/`eprint!` macros but not direct writes to
/// the `stderr` handle, so this line survives a plain `cargo test` run. That
/// matters: a captured `println!` is only shown for a FAILING test, which made
/// a skip indistinguishable from a pass in the log. `tracing::info!` cannot
/// serve here either — no test binary in this crate installs a subscriber, so
/// the line would vanish completely.
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

/// A `gleam build` that did not yield the proof a compile test needs.
pub(crate) struct GleamBuildFailure {
    /// The whole human-readable report, headed by what the failure is.
    report: String,
}

impl GleamBuildFailure {
    /// Wrap a finished report.
    fn new(report: String) -> Self {
        Self { report }
    }
}

impl fmt::Display for GleamBuildFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.report)
    }
}

// `libtest` prints the `Debug` form of an `Err` returned from a test, so
// `Debug` must be the readable report rather than a struct dump with the
// `gleam` output escaped onto one line.
impl fmt::Debug for GleamBuildFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.report)
    }
}

impl Error for GleamBuildFailure {}

// ---- classification pins -------------------------------------------------
//
// Every text below is real captured `gleam` output, not a paraphrase: the CI
// run that shadowed the all-features gate, a `gleam build` against an
// unreachable registry (an unroutable HTTPS proxy, cold Hex cache), and a
// `gleam build` over a Gleam module with an unknown variable.

/// The failure that shadowed the all-features gate on run `30192212491` is an
/// infrastructure failure, and is retried rather than blamed on the emitter.
#[test]
fn the_ci_tar_archive_failure_is_infrastructure() {
    let observed = "  Resolving versions\nDownloading packages\nerror: Failure opening tar \
                    archive\n\nAn error occurred while opening the tar archive.\n";
    assert!(
        matches!(classify_text(observed), BuildFailure::Registry),
        "`error: Failure opening tar archive` is the Hex registry failing, not the AWL emitter"
    );
}

/// An unreachable registry surfaces as a resolution failure whose body names
/// the transport, and is infrastructure.
#[test]
fn an_unreachable_registry_is_infrastructure() {
    let observed = "  Resolving versions\nerror: Dependency resolution failed\n\nAn error \
                    occurred while choosing the version of gleam_json: error sending request for \
                    url (https://repo.hex.pm/packages/gleam_json)\n";
    assert!(
        matches!(classify_text(observed), BuildFailure::Registry),
        "a resolution failure whose body names the registry endpoint is infrastructure"
    );
}

/// A resolution failure with no transport in its body is a real defect in the
/// generated `gleam.toml`, and must fail immediately rather than be retried
/// and excused as infrastructure.
#[test]
fn an_unsatisfiable_requirement_is_a_defect() {
    let observed = "  Resolving versions\nerror: Dependency resolution failed\n\nAn error \
                    occurred while choosing the version of gleam_stdlib: no versions satisfy the \
                    given requirements\n";
    assert!(
        matches!(classify_text(observed), BuildFailure::Sources),
        "an unsatisfiable requirement is ours to fix, not a transient registry"
    );
}

/// A genuine compile failure is never classified as infrastructure — that is
/// the signal these proofs exist for.
#[test]
fn a_rejected_module_is_a_defect() {
    let observed = "  Compiling awl_flagship_compile_proof\nerror: Unknown variable\n  \
                    ┌─ src/awl_hello.gleam:2:3\n  │\n2 │   nope_undefined()\n  │   \
                    ^^^^^^^^^^^^^^\n\nThe name `nope_undefined` is not in scope here.\n";
    assert!(
        matches!(classify_text(observed), BuildFailure::Sources),
        "a Gleam compile error must stay an emitter defect, never a tolerated registry blip"
    );
}

/// Progress chatter that mentions downloading appears on every successful and
/// every failing build alike, and must not on its own excuse anything.
#[test]
fn download_progress_alone_is_not_a_registry_failure() {
    let observed = "  Resolving versions\nDownloading packages\n Downloaded 2 packages in \
                    0.01s\n      Added gleam_stdlib v1.0.3\nerror: Type mismatch\n";
    assert!(
        matches!(classify_text(observed), BuildFailure::Sources),
        "`Downloading packages` is progress output, not a failure classification"
    );
}
