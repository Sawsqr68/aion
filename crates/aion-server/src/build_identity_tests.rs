//! Tests for the compiled-in build identity (#123).

use super::{BuildIdentity, UNKNOWN};

/// A commit hash is 40 lowercase hex characters, and that is the only shape
/// other than the honest absence that may reach an operator.
fn is_commit_hash(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// The build script really ran and really stamped this binary.
///
/// This is the non-vacuity guard for every other assertion here: if the script
/// silently produced nothing, `env!` would fail to compile — so reaching this
/// test at all proves the stamp exists, and what it checks is that the stamp
/// says something an operator can act on.
#[test]
fn the_binary_knows_what_revision_it_is() {
    let identity = BuildIdentity::current();

    assert_eq!(
        identity.version,
        env!("CARGO_PKG_VERSION"),
        "the version must be this crate's own, never a copy that can drift"
    );
    assert!(
        is_commit_hash(identity.commit) || identity.commit == UNKNOWN,
        "the commit must be a full hash or the honest absence, got `{}` — a \
         truncated or fabricated value is worse than no value, because it reads \
         exactly like an answer",
        identity.commit
    );
    assert!(
        matches!(identity.dirty, "true" | "false" | UNKNOWN),
        "dirty must be one of three stated states, got `{}`",
        identity.dirty
    );
}

/// Built from this repository, so the stamp is the real thing rather than the
/// degraded path — and the degraded path is not what the other tests are
/// silently exercising.
///
/// Gated at RUNTIME on whether a repository is actually reachable, never with
/// `#[ignore]`: a build from a crates.io tarball has no git and must still pass
/// its suite. The skip announces itself rather than passing quietly, so a
/// permanently-skipping test cannot masquerade as a permanently-passing one.
#[test]
fn a_build_from_a_repository_stamps_a_real_commit() {
    let identity = BuildIdentity::current();
    if identity.commit == UNKNOWN {
        tracing::info!(
            "skipped: this binary was built with no reachable git repository, which is a \
             supported build and the stamped `unknown` is the correct answer for it"
        );
        return;
    }
    assert!(
        is_commit_hash(identity.commit),
        "a stamped commit must be a full 40-character hash, got `{}`",
        identity.commit
    );
    assert_ne!(
        identity.built_at, UNKNOWN,
        "a build that could reach git could also read a clock; `unknown` here means the \
         epoch stamp did not survive the boundary"
    );
    // RFC 3339 is the contract, so the value must round-trip through a parser
    // rather than merely being a non-empty string.
    assert!(
        chrono::DateTime::parse_from_rfc3339(&identity.built_at).is_ok(),
        "built_at must be RFC 3339, got `{}`",
        identity.built_at
    );
}

/// The stamped commit IS the checked-out commit.
///
/// Every other assertion here checks the stamp's SHAPE. Shape is an
/// enumeration — it would pass just as happily on a stale hash from three
/// weeks ago, which is the only failure mode that matters, because a stale
/// revision reads exactly like a current one. This checks the invariant: the
/// binary's claim about its source equals the source.
///
/// It is what makes the build script's `rerun-if-changed` registrations
/// load-bearing rather than decorative. Delete them and this test is the one
/// that notices.
///
/// Skipped, announcing itself, when git is not reachable — the same supported
/// build the sibling test allows for.
#[test]
fn the_stamped_commit_is_the_commit_that_is_checked_out() {
    let identity = BuildIdentity::current();
    let Some(head) = head_commit() else {
        tracing::info!(
            "skipped: git is not reachable from the test, so there is no independent \
             answer to compare the stamp against"
        );
        return;
    };
    assert_eq!(
        identity.commit, head,
        "the binary claims a different revision than the tree it was built from — a \
         stale stamp is indistinguishable from a current one to every reader, which is \
         the whole reason the build script registers rerun triggers"
    );
}

/// The checked-out commit, read independently of the build script.
///
/// Deliberately a second, separate invocation rather than anything the stamp
/// touched: a comparison against a value derived from the thing under test
/// proves nothing.
fn head_commit() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(text.trim().to_owned())
}

/// The banner line carries the version and the commit, because the log a
/// crashed server leaves behind is the only identity available when the
/// endpoint cannot be asked.
#[test]
fn the_banner_line_names_the_version_and_the_commit() {
    let identity = BuildIdentity::current();
    let line = identity.line();

    assert!(line.contains(identity.version), "{line}");
    assert!(line.contains(identity.commit), "{line}");
    assert!(line.contains(&identity.built_at), "{line}");
}

/// A dirty build says so in the line, and a clean one does not.
///
/// The discriminating control is the second half: without it, a `line` that
/// appended `-dirty` unconditionally would pass the first assertion.
#[test]
fn the_banner_line_marks_a_dirty_tree_and_only_a_dirty_tree() {
    let dirty = BuildIdentity {
        version: "0.0.0",
        commit: "0123456789abcdef0123456789abcdef01234567",
        dirty: "true",
        built_at: String::from("1970-01-01T00:00:00+00:00"),
    };
    let clean = BuildIdentity {
        dirty: "false",
        ..dirty.clone()
    };
    let unestablished = BuildIdentity {
        dirty: UNKNOWN,
        ..dirty.clone()
    };

    assert!(dirty.line().contains("-dirty"), "{}", dirty.line());
    assert!(!clean.line().contains("-dirty"), "{}", clean.line());
    // `unknown` is not `true`: an unestablished tree state must not be
    // presented as a dirty one, which would be a claim nobody measured.
    assert!(
        !unestablished.line().contains("-dirty"),
        "{}",
        unestablished.line()
    );
}
