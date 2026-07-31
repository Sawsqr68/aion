//! Stamps the binary with the source revision it was built from (#123).
//!
//! # Why a build script exists for this
//!
//! A running server could not be asked what code it is. The only build identity
//! anywhere was one startup log line carrying `CARGO_PKG_VERSION` — a crate
//! version, which cannot distinguish two builds from different commits of the
//! same version. That is not a hypothetical gap: on 2026-07-31 a live server's
//! running image was found to differ from every preserved copy of "the same"
//! binary, and there was no way to establish which revision was actually
//! serving. Restarting it would have been a substitution presented as a
//! restoration.
//!
//! Only a build script can capture this. The revision is a property of the
//! source tree at compile time and is unavailable to the compiled program by
//! any other means.
//!
//! # Failing to find git is NOT an error
//!
//! A crate built from a crates.io tarball, a vendored copy, or an exported
//! archive has no repository at all. That is a perfectly legitimate build and
//! must not fail. Every probe here degrades to [`UNKNOWN`], which the runtime
//! surface reports as literally "unknown" — an honest absence the operator can
//! see, never a fabricated or defaulted value.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// What every probe reports when the answer genuinely cannot be established.
///
/// One sentinel, used everywhere, and it is a word rather than an empty string:
/// an empty value reads as a formatting bug at the far end, where "unknown"
/// reads as the measurement it is.
const UNKNOWN: &str = "unknown";

fn main() {
    // Re-run when the checked-out revision changes. Without these the recorded
    // commit would be whatever it was the last time cargo happened to re-run
    // this script — a stale label that looks exactly like a current one, which
    // is worse than reporting nothing.
    println!("cargo::rerun-if-changed=build.rs");
    let git_dir = git_dir();
    if let Some(git_dir) = git_dir.as_deref() {
        watch_head(git_dir);
    }

    println!("cargo::rustc-env=AION_BUILD_COMMIT={}", commit());
    println!("cargo::rustc-env=AION_BUILD_DIRTY={}", dirty());
    println!("cargo::rustc-env=AION_BUILD_EPOCH={}", build_epoch());
}

/// Absolute path of this package's `.git` directory, when there is one.
///
/// Asked of git rather than assembled from the manifest directory, because the
/// repository root is not at a fixed depth above the package and a worktree's
/// `.git` is a FILE pointing elsewhere, not a directory.
fn git_dir() -> Option<PathBuf> {
    let output = git(&["rev-parse", "--absolute-git-dir"])?;
    Some(PathBuf::from(output))
}

/// Register the files whose contents change when the checkout moves.
///
/// `HEAD` covers a detached checkout directly. On a branch, `HEAD` holds a
/// symbolic ref whose contents do NOT change as commits land, so the ref file it
/// names is registered too — otherwise every commit on the same branch would
/// leave the stamp untouched, which is precisely the common case.
fn watch_head(git_dir: &Path) {
    let head = git_dir.join("HEAD");
    if !head.exists() {
        return;
    }
    println!("cargo::rerun-if-changed={}", head.display());
    let Some(reference) = git(&["symbolic-ref", "--quiet", "HEAD"]) else {
        // Detached HEAD: the HEAD file itself carries the commit, already
        // registered above, and there is no ref file to follow.
        return;
    };
    // A packed ref has no file of its own; registering a path that does not
    // exist is not an error for cargo, and the loose file appears the moment
    // the ref is next written.
    println!(
        "cargo::rerun-if-changed={}",
        git_dir.join(&reference).display()
    );
    println!(
        "cargo::rerun-if-changed={}",
        git_dir.join("packed-refs").display()
    );
}

/// Full 40-character commit hash of the source this binary is built from.
fn commit() -> String {
    git(&["rev-parse", "HEAD"]).unwrap_or_else(|| UNKNOWN.to_owned())
}

/// Whether the working tree carried uncommitted changes when this script ran.
///
/// **Stated limitation, and it is why the runtime field says "at build time".**
/// Cargo cannot watch a working tree, so this script re-runs on the triggers
/// registered above and on changes to this package's own sources — not on every
/// build. A tree dirtied in a DIFFERENT workspace crate after this ran can
/// therefore be reported as clean. It is stated rather than silently trusted:
/// `false` means "no uncommitted change was observed", not "the tree is
/// provably clean". `true` is always trustworthy, and so is the commit.
fn dirty() -> String {
    match git(&["status", "--porcelain", "--untracked-files=no"]) {
        Some(output) if output.is_empty() => String::from("false"),
        Some(_) => String::from("true"),
        None => UNKNOWN.to_owned(),
    }
}

/// Seconds since the Unix epoch at which this script ran.
///
/// Honours `SOURCE_DATE_EPOCH` when set, so a reproducible-build environment
/// gets a reproducible stamp rather than this being the one field that defeats
/// it. Formatting is deferred to the runtime, which already has a date library;
/// carrying an integer across the boundary keeps the build script free of
/// dependencies.
fn build_epoch() -> String {
    println!("cargo::rerun-if-env-changed=SOURCE_DATE_EPOCH");
    if let Ok(declared) = std::env::var("SOURCE_DATE_EPOCH") {
        // Parsed, not merely present: an unparsable value is a misconfiguration
        // to fall through, never a string to stamp into the binary.
        if declared.parse::<i64>().is_ok() {
            return declared;
        }
    }
    SystemTime::now().duration_since(UNIX_EPOCH).map_or_else(
        |_error| UNKNOWN.to_owned(),
        |since| since.as_secs().to_string(),
    )
}

/// Run a git command, returning its trimmed stdout only on a clean exit.
///
/// A missing git binary, a directory that is not a repository, and a command
/// that fails all collapse to `None` — every one of them means the same thing
/// here (the answer cannot be established) and none of them is a build failure.
fn git(args: &[&str]) -> Option<String> {
    // Read at run time rather than with `env!`: the manifest directory of the
    // package being built is what matters, and reading it from the environment
    // cargo actually sets for this script cannot disagree with it.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let output = Command::new("git")
        .args(args)
        .current_dir(manifest_dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(text.trim().to_owned())
}
