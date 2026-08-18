//! Shared gate and runner for the tests in this crate that shell out to the
//! real `gleam` CLI.
//!
//! Task #74. This file is a **port** of the cure landed for `aion-awl` at
//! commit `8fb4c153` (`crates/aion-awl/tests/test_support/gleam.rs`). The
//! version it replaces announced its skip with `println!` and accepted a zero
//! exit from `gleam build` as proof, so three different outcomes were fused
//! into one indistinguishable `ok` line. They are now kept apart:
//!
//! - `gleam` is **absent** — the test skips, and announces the skip loudly
//!   enough that it can never be read as a pass (see [`announce`]).
//! - `gleam` **rejects the Gleam under test** — the test FAILS, named as a code
//!   defect. That is the signal these proofs exist for.
//! - `gleam` fails inside its own **dependency layer** (the Hex registry) —
//!   the build is retried a bounded number of times, and if every attempt
//!   fails the test still FAILS, named as an infrastructure failure. It is
//!   never skipped: a fetch failure that skipped would let the proof pass
//!   while proving nothing, which is the exact vacuity this cycle is about.
//!
//! Classification decides **whether to retry** and **what the message calls
//! the failure**. It can never turn a failure into a pass: both branches end
//! in `Err`. That is why an unrecognised failure is classified as a code
//! defect — the safe polarity is "blame the code, make a human look".
//!
//! # Why this file is copied rather than shared
//!
//! `#[path = "test_support/gleam.rs"] mod ...` resolves relative to the
//! including test file, so a single shared copy would mean each crate reaching
//! across a crate boundary with `../../<other-crate>/tests/...`. That is a
//! path hack, and it would break `cargo package` for these published crates:
//! `tests/` is packaged, and a source file outside the crate root is not.
//! The copies are therefore kept byte-identical and pinned that way by
//! [`the_shared_gleam_support_copies_have_not_diverged`], which fails in every
//! crate the moment one copy is edited alone.
//!
//! That pin checks the copies it was told about, so on its own it cannot see a
//! copy nobody wrote down — the list it verifies is the same list it derives
//! its expectation from. [`every_gleam_support_file_is_classified`] supplies
//! the other half by enumerating the filesystem and requiring every `gleam.rs`
//! it finds to be accounted for, so the three lists together are a statement
//! about **every** copy that exists rather than only the intended ones.

use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::panic::Location;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

/// `gleam`'s own error titles for failures in its dependency layer, taken
/// from the CLI's error set rather than invented: a fetch, a Hex API call, or
/// the unpacking of a downloaded tarball. `Failure opening tar archive` is
/// the one that shadowed the all-features gate on CI run `30192212491`.
///
/// None of these can be produced by a Gleam type or syntax error, so no real
/// code defect can land in this set.
const REGISTRY_FAILURE_TITLES: &[&str] = &[
    "Failed to download package",
    "Failure opening tar archive",
    "Hex API failure",
    "HTTP error",
];

/// `gleam`'s title for a failed version solve. Unlike the titles above this
/// one is ambiguous: it covers both an unreachable registry and a genuinely
/// unsatisfiable requirement in a project's `gleam.toml`, so it only counts as
/// infrastructure when the body also names the registry endpoint or a
/// transport failure ([`RESOLUTION_TRANSPORT_MARKERS`]).
const RESOLUTION_FAILURE_TITLE: &str = "Dependency resolution failed";

/// Observed bodies of a resolution failure caused by transport rather than by
/// an unsatisfiable requirement, e.g. `An error occurred while choosing the
/// version of gleam_json: error sending request for url
/// (https://repo.hex.pm/packages/gleam_json)`.
const RESOLUTION_TRANSPORT_MARKERS: &[&str] = &["error sending request", "repo.hex.pm"];

/// `gleam`'s title for file-system failures. This title is also used for
/// ordinary project-file IO, so it only counts as infrastructure when the body
/// identifies package unpacking or a package cache path
/// ([`FILE_IO_PACKAGE_MARKERS`]).
const FILE_IO_FAILURE_TITLE: &str = "File IO failure";

/// Package-layer evidence that makes a [`FILE_IO_FAILURE_TITLE`] diagnostic a
/// dependency failure rather than a fault reading or writing project sources.
const FILE_IO_PACKAGE_MARKERS: &[&str] = &[
    "failed to unpack",
    "/build/packages/",
    "\\build\\packages\\",
    "/hex/hexpm/packages/",
    "\\hex\\hexpm\\packages\\",
];

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

/// Gleam sources live here, one module per `.gleam` file, nested directories
/// flattened into the Erlang module name with `@`.
const GLEAM_SOURCE_ROOT: &str = "src";

/// The copies of this file that must stay byte-identical, relative to the
/// repository root. Every crate in this list compiles this same text, so the
/// list is the same in every copy and the pin is symmetric.
///
/// 🔴 **This list is an expectation, not a census.** It cannot answer "how many
/// copies exist" — a copy that is never written down is, to this list, not
/// there. That is why [`discover_gleam_support_files`] enumerates the
/// filesystem independently and
/// `every_gleam_support_file_is_classified` requires every file it
/// finds to appear in this list or in [`DIVERGENT_COPIES`] /
/// [`SEPARATE_BY_DESIGN`]. A baseline derived from the thing it audits passes
/// hardest when it is most wrong.
const SHARED_COPIES: &[&str] = &[
    "crates/aion/tests/test_support/gleam.rs",
    "crates/aion-cli/tests/test_support/gleam.rs",
    "crates/aion-awl-package/tests/test_support/gleam.rs",
    "crates/aion-server/tests/test_support/gleam.rs",
];

/// Copies that are **not** byte-identical to [`SHARED_COPIES`] and should be,
/// each with the reason it has not converged yet.
///
/// An entry here is a debt, not a decision. It is deliberately a separate
/// constant from [`SEPARATE_BY_DESIGN`] so that the *name* carries the status
/// and a reader does not have to weigh prose to tell "known-wrong" from
/// "correctly different".
const DIVERGENT_COPIES: &[(&str, &str)] = &[(
    "crates/aion-awl/tests/test_support/gleam.rs",
    "The ANCESTOR this file was ported from at 8fb4c153 (#74). It never \
     received the improvements made in transit — it still calls a rejected \
     build an 'emitter defect' where the ported text says 'code defect', and \
     it has no dependency-layer classification at all. Converging it is a real \
     refactor, not a copy: it exposes `compile_project` with a different \
     signature to `compile_whole_project` here, and 13 test files in that \
     crate call it. Tracked separately; do not silently `cp` over it.",
)];

/// Files that share this file's name and purpose but are correctly a different
/// artefact, each with the reason it is not a copy.
const SEPARATE_BY_DESIGN: &[(&str, &str)] = &[(
    "crates/aion-package/tests/test_support/gleam.rs",
    "A deliberate crate-local gate, not a stale copy. It carries only the \
     runtime skip and its announcement — no project runner, no build \
     classification — because that is all `aion-package` needs, and it is the \
     single definition included by all three of that crate's call sites, \
     including the unit test inside `src/` which reaches it through a `#[path]` \
     in `src/structure/mod.rs`. Converging it would import machinery the crate \
     does not use.",
)];

/// Distinguishes the scratch projects the runner pins create, so parallel test
/// binaries never share one directory.
static PIN_PROJECT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// The repository root, from this crate's manifest directory. Every crate that
/// compiles this file lives at `crates/<name>`, so the expression is shared.
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

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

/// Run `gleam build` over the whole project at `project`, tolerating a
/// transient Hex registry, and prove that every module under its `src/`
/// directory was actually compiled.
///
/// Returns every `ebin` directory the build produced, so callers that load the
/// artefacts can do so without re-walking the tree.
///
/// The claimed module set is derived from the sources rather than passed in, so
/// it cannot rot as a project gains modules, and a project with no sources at
/// all fails instead of proving nothing.
///
/// # Errors
///
/// Fails when the project declares no Gleam modules, when `gleam` cannot be
/// executed, when it rejects the sources, when every attempt dies in the
/// dependency layer, or when it reports success without having produced a
/// `.beam` for one of the modules. The report names which of those happened.
pub(crate) fn compile_whole_project(project: &Path) -> Result<Vec<PathBuf>, GleamBuildFailure> {
    let modules = project_modules(project)?;
    let claimed: Vec<&str> = modules.iter().map(String::as_str).collect();
    compile_project(project, &claimed)
}

/// Every Erlang module name `gleam build` must produce for the sources under
/// `<project>/src`, with nested directories flattened the way Gleam does it
/// (`src/aion/flow.gleam` becomes `aion@flow`).
fn project_modules(project: &Path) -> Result<Vec<String>, GleamBuildFailure> {
    let source_root = project.join(GLEAM_SOURCE_ROOT);
    let mut modules = Vec::new();
    collect_modules(&source_root, "", &mut modules)?;
    if modules.is_empty() {
        return Err(GleamBuildFailure::new(format!(
            "no Gleam modules found under {}: a compile proof over a project with no sources \
             would pass without compiling anything",
            source_root.display()
        )));
    }
    modules.sort();
    Ok(modules)
}

/// Recursive half of [`project_modules`]. `prefix` is the already-flattened
/// package path, empty at the source root.
fn collect_modules(
    directory: &Path,
    prefix: &str,
    into: &mut Vec<String>,
) -> Result<(), GleamBuildFailure> {
    for entry in fs::read_dir(directory).map_err(|error| io_failure(directory, &error))? {
        let entry = entry.map_err(|error| io_failure(directory, &error))?;
        let path = entry.path();
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Err(GleamBuildFailure::new(format!(
                "non-UTF-8 entry under {}: the compiled module name it maps to cannot be proven",
                directory.display()
            )));
        };
        if path.is_dir() {
            let nested = format!("{prefix}{name}@");
            collect_modules(&path, &nested, into)?;
        } else if let Some(stem) = name.strip_suffix(".gleam") {
            into.push(format!("{prefix}{stem}"));
        }
    }
    Ok(())
}

/// Run `gleam build` in `project`, tolerating a transient Hex registry, and
/// prove that every module in `modules` was actually compiled.
///
/// # Errors
///
/// Fails when `gleam` cannot be executed, when it rejects the Gleam under
/// test, when every attempt dies in the dependency layer, or when it reports
/// success without having produced a `.beam` for one of `modules`.
fn compile_project(project: &Path, modules: &[&str]) -> Result<Vec<PathBuf>, GleamBuildFailure> {
    let mut attempt: u32 = 1;
    loop {
        let output = Command::new("gleam")
            .arg("build")
            .current_dir(project)
            .output()
            .map_err(|error| {
                GleamBuildFailure::new(format!(
                    "INFRASTRUCTURE FAILURE, NOT A CODE DEFECT: the `gleam` binary is required \
                     for this compile proof and could not be executed for the project at {}: \
                     {error}",
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
                 layer (the Hex registry), not on the Gleam under test; retrying",
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

/// True when `combined` — the joined stdout and stderr of a FAILED `gleam`
/// invocation — is `gleam` failing in its own dependency layer rather than
/// refusing the Gleam it was given.
///
/// For the negative compile proofs, where a build is *expected* to fail: an
/// unreachable registry would otherwise satisfy the expectation and let the
/// proof pass without ever having reached the compiler.
pub(crate) fn is_dependency_layer_failure(combined: &str) -> bool {
    matches!(classify_text(combined), BuildFailure::Registry)
}

/// Which layer of `gleam` refused the build.
enum BuildFailure {
    /// `gleam`'s dependency layer: a fetch, a Hex API call, or unpacking a
    /// downloaded tarball. Retryable, and reported as infrastructure.
    Registry,
    /// `gleam` refused the sources themselves. Never retried, and reported as
    /// a code defect. Also the classification for anything unrecognised.
    Sources,
}

/// Classify a failed `gleam build` from the text it already printed.
fn classify(output: &Output) -> BuildFailure {
    classify_text(&combined_output(output))
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
        if title == FILE_IO_FAILURE_TITLE
            && FILE_IO_PACKAGE_MARKERS
                .iter()
                .any(|marker| combined.contains(marker))
        {
            return BuildFailure::Registry;
        }
    }
    BuildFailure::Sources
}

/// Everything a `gleam` invocation printed, in one string.
pub(crate) fn combined_output(output: &Output) -> String {
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push('\n');
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    combined
}

/// The full failure report, headed by what the failure actually is.
fn report(project: &Path, failure: &BuildFailure, attempts: u32, output: &Output) -> String {
    let headline = match failure {
        BuildFailure::Registry => format!(
            "INFRASTRUCTURE FAILURE, NOT A CODE DEFECT: `gleam build` failed inside its own \
             dependency layer (the Hex registry) on all {attempts} attempt(s). The Gleam under \
             test was never compiled, so this run proves nothing — fix the environment and run it \
             again."
        ),
        BuildFailure::Sources => format!(
            "CODE DEFECT: `gleam` rejected the Gleam under test ({attempts} attempt(s), no \
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
                 cannot pass without having compiled the module it claims",
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
        "could not read {} to prove the claimed modules were compiled: {error}",
        path.display()
    ))
}

/// Write a line to the process's real stderr handle.
///
/// `libtest` captures the `print!`/`eprint!` macros but not direct writes to
/// the `stderr` handle, so this line survives a plain `cargo test` run. That
/// matters: a captured `println!` is only shown for a FAILING test, which made
/// a skip indistinguishable from a pass in the log — the defect this file
/// exists to remove. `tracing::info!` cannot serve here either: no test binary
/// in this crate installs a subscriber, so the line would vanish completely,
/// which is strictly worse than the `println!` it replaces.
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

/// The unconditional dependency titles and the package-qualified file-IO
/// title keep their respective Registry/Sources classifications.
#[test]
fn dependency_failure_titles_are_classified_by_layer() {
    let cases = [
        (
            "package download",
            "  Resolving versions\nDownloading packages\nerror: Failed to download package\n\nThe package gleam_stdlib could not be downloaded from Hex.\n",
            true,
        ),
        (
            "tar archive",
            "  Resolving versions\nDownloading packages\nerror: Failure opening tar archive\n\nAn error occurred while opening the downloaded tar archive.\n",
            true,
        ),
        (
            "Hex API",
            "  Resolving versions\nerror: Hex API failure\n\nThe Hex API returned an invalid response while fetching package metadata.\n",
            true,
        ),
        (
            "HTTP",
            "  Resolving versions\nerror: HTTP error\n\nThe package request failed with HTTP status 503 Service Unavailable.\n",
            true,
        ),
        (
            "failed package unpack",
            "  Resolving versions\nDownloading packages\nerror: File IO failure\n\nAn error occurred while writing a dependency file: failed to unpack gleam@uri.erl\n",
            true,
        ),
        (
            "project package cache paths",
            "  Resolving versions\nDownloading packages\nerror: File IO failure\n\nCould not write /tmp/awl/build/packages/gleam_stdlib/src/gleam/list.gleam; the equivalent Windows destination is C:\\work\\awl\\build\\packages\\gleam_stdlib\\src\\gleam\\list.gleam.\n",
            true,
        ),
        (
            "global Hex package cache paths",
            "  Resolving versions\nDownloading packages\nerror: File IO failure\n\nCould not write /home/runner/.cache/gleam/hex/hexpm/packages/gleam_stdlib/1.6.0/src/gleam/list.gleam; the equivalent Windows cache is C:\\Users\\runner\\AppData\\Local\\gleam\\hex\\hexpm\\packages\\gleam_stdlib\\1.6.0\\src\\gleam\\list.gleam.\n",
            true,
        ),
        (
            "ordinary source file",
            "  Compiling awl_flagship_compile_proof\nerror: File IO failure\n\nCould not read src/awl_hello.gleam: permission denied.\n",
            false,
        ),
    ];

    for (name, observed, expected_registry) in cases {
        assert_eq!(
            is_dependency_layer_failure(observed),
            expected_registry,
            "classification row `{name}` returned the wrong layer"
        );
    }
}

/// The failure that shadowed the all-features gate on run `30192212491` is an
/// infrastructure failure, and is retried rather than blamed on the code.
#[test]
fn the_ci_tar_archive_failure_is_infrastructure() {
    let observed = "  Resolving versions\nDownloading packages\nerror: Failure opening tar \
                    archive\n\nAn error occurred while opening the tar archive.\n";
    assert!(
        is_dependency_layer_failure(observed),
        "`error: Failure opening tar archive` is the Hex registry failing, not the code under test"
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
        is_dependency_layer_failure(observed),
        "a resolution failure whose body names the registry endpoint is infrastructure"
    );
}

/// A resolution failure with no transport in its body is a real defect in the
/// project's `gleam.toml`, and must fail immediately rather than be retried
/// and excused as infrastructure.
#[test]
fn an_unsatisfiable_requirement_is_a_defect() {
    let observed = "  Resolving versions\nerror: Dependency resolution failed\n\nAn error \
                    occurred while choosing the version of gleam_stdlib: no versions satisfy the \
                    given requirements\n";
    assert!(
        !is_dependency_layer_failure(observed),
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
        !is_dependency_layer_failure(observed),
        "a Gleam compile error must stay a code defect, never a tolerated registry blip"
    );
}

/// Progress chatter that mentions downloading appears on every successful and
/// every failing build alike, and must not on its own excuse anything.
#[test]
fn download_progress_alone_is_not_a_registry_failure() {
    let observed = "  Resolving versions\nDownloading packages\n Downloaded 2 packages in \
                    0.01s\n      Added gleam_stdlib v1.0.3\nerror: Type mismatch\n";
    assert!(
        !is_dependency_layer_failure(observed),
        "`Downloading packages` is progress output, not a failure classification"
    );
}

// ---- runner pins ---------------------------------------------------------

/// A project `gleam` refuses FAILS, and the report blames the code rather than
/// the environment.
///
/// Deliberately network-free: the scratch directory holds no `gleam.toml`, so
/// `gleam` stops at `error: Project not found` before it ever resolves a
/// dependency. A pin that needed Hex could not tell these two classes apart,
/// which is the whole point.
#[test]
fn a_project_gleam_refuses_fails_as_a_code_defect() -> Result<(), Box<dyn Error>> {
    if skip_if_unavailable() {
        return Ok(());
    }
    let scratch = pin_scratch_directory("no-sources")?;
    fs::create_dir_all(scratch.join(GLEAM_SOURCE_ROOT))?;
    let outcome = compile_whole_project(&scratch);
    fs::remove_dir_all(&scratch)?;
    let Err(failure) = outcome else {
        return Err("a project whose `src` holds no Gleam modules must fail, not pass".into());
    };
    let report = failure.to_string();
    assert!(
        report.contains("no Gleam modules found"),
        "a project with no sources must fail on the missing sources, not pass: {report}"
    );

    // And with sources present but no project descriptor, the `gleam build`
    // itself is refused and reported as a code defect.
    let scratch = pin_scratch_directory("no-descriptor")?;
    fs::create_dir_all(scratch.join(GLEAM_SOURCE_ROOT))?;
    fs::write(
        scratch.join(GLEAM_SOURCE_ROOT).join("pin_module.gleam"),
        "pub fn main() { Nil }\n",
    )?;
    let outcome = compile_whole_project(&scratch);
    fs::remove_dir_all(&scratch)?;
    let Err(failure) = outcome else {
        return Err("`gleam build` in a directory that is not a Gleam project must fail".into());
    };
    let report = failure.to_string();
    assert!(
        report.starts_with("CODE DEFECT:"),
        "a `gleam` refusal must be named a code defect, never excused as infrastructure: {report}"
    );
    assert!(
        report.contains("Project not found"),
        "the report must carry `gleam`'s own words so a human can act on them: {report}"
    );
    Ok(())
}

/// A scratch directory for a runner pin, unique across parallel test binaries.
fn pin_scratch_directory(purpose: &str) -> Result<PathBuf, io::Error> {
    let sequence = PIN_PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "aion-gleam-support-pin-{purpose}-{}-{sequence}",
        std::process::id()
    ));
    if directory.exists() {
        fs::remove_dir_all(&directory)?;
    }
    fs::create_dir_all(&directory)?;
    Ok(directory)
}

// ---- divergence pin ------------------------------------------------------

/// The copies of this file listed in [`SHARED_COPIES`] are byte-identical.
///
/// This is the mechanism that catches the next divergence: because the pin
/// ships inside the shared text, editing any one copy fails this test in every
/// crate that carries it, not just the crate that was edited.
#[test]
fn the_shared_gleam_support_copies_have_not_diverged() -> Result<(), Box<dyn Error>> {
    let root = repository_root();
    let mut canonical: Option<(&str, Vec<u8>)> = None;
    for relative in SHARED_COPIES {
        let path = root.join(relative);
        let bytes = fs::read(&path)
            .map_err(|error| format!("could not read shared copy {}: {error}", path.display()))?;
        match &canonical {
            None => canonical = Some((relative, bytes)),
            Some((first, expected)) => assert!(
                bytes == *expected,
                "the shared gleam test-support copies have diverged: {relative} differs from \
                 {first}. This file is copied, not shared (see the module docs) — apply the edit \
                 to every path in SHARED_COPIES."
            ),
        }
    }
    assert!(
        canonical.is_some(),
        "SHARED_COPIES must not be empty: an empty list would make this pin vacuous"
    );
    Ok(())
}

// ---- census --------------------------------------------------------------

/// The subtrees of each crate a copy of this gate can live in.
///
/// `tests/` is where every copy lives today. `src/` is searched too because
/// #125 found a hand-rolled `gleam` gate inside `src/`, where a sweep over
/// `tests/` could not see it: a search space that excludes a place the thing
/// has actually been found is not a search space.
const CRATE_SUBTREES_TO_SEARCH: &[&str] = &["src", "tests"];

/// Collect every file named `name` beneath `directory`, recursively.
///
/// A missing directory is not an error — most crates have no `tests/` — but an
/// unreadable one is, because a directory this walk cannot enter is a place a
/// copy could hide while the census reported a clean result.
fn collect_named_files(
    directory: &Path,
    name: &str,
    found: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    if !directory.is_dir() {
        return Ok(());
    }
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("could not read {}: {error}", directory.display()))?;
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            collect_named_files(&path, name, found)?;
        } else if path.file_name().and_then(|file| file.to_str()) == Some(name) {
            found.push(path);
        }
    }
    Ok(())
}

/// Every file in the workspace named `gleam.rs`, as repository-relative paths
/// with `/` separators, sorted.
///
/// This walks the filesystem and consults none of the classification lists, so
/// it is a **census** rather than a restatement of an expectation. It is the
/// only thing in this file capable of noticing a copy nobody wrote down.
fn discover_gleam_support_files(root: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let crates_root = root.join("crates");
    let entries = fs::read_dir(&crates_root)
        .map_err(|error| format!("could not read {}: {error}", crates_root.display()))?;
    let mut found: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let crate_directory = entry?.path();
        if !crate_directory.is_dir() {
            continue;
        }
        for subtree in CRATE_SUBTREES_TO_SEARCH {
            collect_named_files(&crate_directory.join(subtree), "gleam.rs", &mut found)?;
        }
    }
    let mut relative = Vec::with_capacity(found.len());
    for path in found {
        let suffix = path.strip_prefix(root).map_err(|_| {
            format!(
                "discovered {} outside the repository root {}",
                path.display(),
                root.display()
            )
        })?;
        relative.push(
            suffix
                .components()
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/"),
        );
    }
    relative.sort();
    Ok(relative)
}

/// Every copy of this gate that exists is accounted for by exactly one list.
///
/// [`the_shared_gleam_support_copies_have_not_diverged`] proves the copies in
/// [`SHARED_COPIES`] are identical to one another. It cannot prove there are no
/// others, because the list it checks is also the list it was derived from —
/// delete a copy from `SHARED_COPIES` and the pin stops looking for it and
/// still passes. This test supplies the missing half by enumerating the
/// **population** and requiring the lists to account for it.
///
/// Both directions are asserted, and neither is derived from the other:
///
/// - **population → lists** catches a new copy that nobody wrote down, which is
///   the failure the divergence pin is structurally blind to;
/// - **lists → population** catches a listed path that has been moved, renamed
///   or deleted, and is what keeps this test non-vacuous — a walk that found
///   nothing at all fails it loudly instead of passing with an empty set.
#[test]
fn every_gleam_support_file_is_classified() -> Result<(), Box<dyn Error>> {
    let root = repository_root();
    let discovered = discover_gleam_support_files(&root)?;

    for path in &discovered {
        let listed_as = usize::from(SHARED_COPIES.contains(&path.as_str()))
            + usize::from(
                DIVERGENT_COPIES
                    .iter()
                    .any(|(listed, _)| *listed == path.as_str()),
            )
            + usize::from(
                SEPARATE_BY_DESIGN
                    .iter()
                    .any(|(listed, _)| *listed == path.as_str()),
            );
        assert!(
            listed_as == 1,
            "{path} is classified by {listed_as} of the three lists, and must be classified by \
             exactly one. A new copy of this gate is not covered by any pin until it is written \
             down: put it in SHARED_COPIES if it must stay byte-identical, in DIVERGENT_COPIES if \
             it has drifted and should converge, or in SEPARATE_BY_DESIGN with the reason it is \
             correctly a different artefact."
        );
    }

    for listed in SHARED_COPIES
        .iter()
        .copied()
        .chain(DIVERGENT_COPIES.iter().map(|(path, _)| *path))
        .chain(SEPARATE_BY_DESIGN.iter().map(|(path, _)| *path))
    {
        assert!(
            discovered.iter().any(|path| path == listed),
            "{listed} is listed but the census did not find it. Either the file moved and the \
             list is stale, or the census cannot see where it now lives — and a census that \
             cannot see a copy it was told about cannot be trusted about the ones it was not. \
             The walk covers {CRATE_SUBTREES_TO_SEARCH:?} under each directory in crates/, and \
             found: {discovered:?}"
        );
    }
    Ok(())
}
