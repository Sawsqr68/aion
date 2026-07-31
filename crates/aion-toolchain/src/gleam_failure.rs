//! Which layer of `gleam` refused a build: its own dependency layer, or the
//! sources it was given.
//!
//! # Why this lives in production code and not only in the test harness
//!
//! The test suites have classified this correctly since task #74 — a fetch
//! failure is retried, then reported as **infrastructure**, and never allowed
//! to masquerade as a compile error. Production code did not classify at all.
//! Every non-zero `gleam` exit was reported as though the submitted source was
//! at fault, which on a registry outage is a false statement about code that
//! was never compiled.
//!
//! # 🔴 The polarity rule, and why the default cannot simply be copied
//!
//! `tests/test_support/gleam.rs` states its default deliberately:
//!
//! > "an unrecognised failure is classified as a code defect — the safe
//! > polarity is 'blame the code, make a human look'."
//!
//! That is right in a harness and **inverted in shipped code**. The rule is
//! identical; the audience is not:
//!
//! - in a harness, "blame the code" sends a **maintainer** to code they own and
//!   can fix;
//! - in `aion generate` or the authoring studio, it sends an **author** to
//!   rewrite **working Gleam** over an outage they cannot fix.
//!
//! > A diagnostic's safe default is a property of who RECEIVES it, not of the
//! > failure.
//!
//! So this module shares the *classification* and deliberately does **not**
//! share the default. [`GleamFailureLayer`] has no "the author's code is
//! broken" variant to reach for: one variant is a positive identification, the
//! other is the **absence** of one. A caller cannot accidentally read
//! [`GleamFailureLayer::NotDependencyLayer`] as a finding about the source,
//! because its name says it is not one.

/// What a failed `gleam` invocation was positively identified as.
///
/// Deliberately asymmetric. [`Self::DependencyLayer`] is a **finding** — the
/// output named a registry failure. [`Self::NotDependencyLayer`] is an
/// **absence** — no such signal was present, which is not evidence about the
/// author's source. Callers must phrase the second case as "`gleam` refused the
/// build, here is what it said", never as a claim about a named file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GleamFailureLayer {
    /// `gleam` failed inside its own dependency layer: a fetch, a Hex API
    /// call, unpacking a downloaded tarball, or a version solve that could not
    /// reach the registry. The sources were never compiled, so nothing about
    /// them has been established. Transient, and worth retrying.
    DependencyLayer,
    /// No dependency-layer signal was found in the output.
    ///
    /// **This is an absence, not a diagnosis.** It is the default, it is
    /// reached by every unrecognised failure, and it does not mean the
    /// submitted source is at fault.
    NotDependencyLayer,
}

/// `gleam`'s own error titles for failures in its dependency layer, taken from
/// the CLI's error set rather than invented: a fetch, a Hex API call, or the
/// unpacking of a downloaded tarball.
///
/// None of these can be produced by a Gleam type or syntax error, so no real
/// code defect can land in this set. Kept identical to the set the test
/// harness pins in `tests/test_support/gleam.rs`.
const REGISTRY_FAILURE_TITLES: &[&str] = &[
    "Failed to download package",
    "Failure opening tar archive",
    "Hex API failure",
    "HTTP error",
];

/// `gleam`'s title for a failed version solve.
///
/// Unlike [`REGISTRY_FAILURE_TITLES`] this one is ambiguous: it covers both an
/// unreachable registry and a genuinely unsatisfiable requirement in a
/// project's `gleam.toml`. It therefore only counts as a dependency-layer
/// failure when the body also names a transport failure
/// ([`RESOLUTION_TRANSPORT_MARKERS`]) — an unsatisfiable requirement IS the
/// author's to fix, and must not be excused as infrastructure.
const RESOLUTION_FAILURE_TITLE: &str = "Dependency resolution failed";

/// Observed bodies of a resolution failure caused by transport rather than by
/// an unsatisfiable requirement, e.g. `An error occurred while choosing the
/// version of gleam_json: error sending request for url
/// (https://repo.hex.pm/packages/gleam_json)`.
const RESOLUTION_TRANSPORT_MARKERS: &[&str] = &["error sending request", "repo.hex.pm"];

/// Classify the combined output of a **failed** `gleam` invocation.
///
/// `combined` is the joined stdout and stderr of an invocation that already
/// exited non-zero; this function does not decide whether a build failed, only
/// what kind of failure it was.
///
/// Matching is anchored to `gleam`'s own `error: <Title>` lines. A compile
/// error quotes the offending source back, and a source file can contain any
/// text at all — including these titles — so a substring search over the whole
/// output would let an author's own code decide the classification.
#[must_use]
pub fn classify_gleam_failure(combined: &str) -> GleamFailureLayer {
    for line in combined.lines() {
        let Some(title) = line.trim_start().strip_prefix("error: ") else {
            continue;
        };
        let title = title.trim();
        if REGISTRY_FAILURE_TITLES.contains(&title) {
            return GleamFailureLayer::DependencyLayer;
        }
        if title == RESOLUTION_FAILURE_TITLE
            && RESOLUTION_TRANSPORT_MARKERS
                .iter()
                .any(|marker| combined.contains(marker))
        {
            return GleamFailureLayer::DependencyLayer;
        }
    }
    GleamFailureLayer::NotDependencyLayer
}

#[cfg(test)]
mod tests {
    use super::{GleamFailureLayer, classify_gleam_failure};

    /// The exact output that opened this task, from run 3 of the 2026-08-01
    /// workspace battery.
    const HEX_OUTAGE: &str = "  Resolving versions\nerror: HTTP error\n\nA HTTP request failed.\n\
         The error from the HTTP client was:\n\n    error sending request for url \
         (https://hex.pm/api/packages/gleam_stdlib/releases/1.0.3)\n";

    #[test]
    fn the_hex_outage_that_opened_this_task_is_a_dependency_layer_failure() {
        assert_eq!(
            classify_gleam_failure(HEX_OUTAGE),
            GleamFailureLayer::DependencyLayer
        );
    }

    #[test]
    fn a_tar_archive_failure_is_a_dependency_layer_failure() {
        assert_eq!(
            classify_gleam_failure("error: Failure opening tar archive\n"),
            GleamFailureLayer::DependencyLayer
        );
    }

    #[test]
    fn a_type_error_is_not_a_dependency_layer_failure() {
        let output =
            "error: Type mismatch\n\nExpected type:\n\n    Int\n\nFound type:\n\n    String\n";
        assert_eq!(
            classify_gleam_failure(output),
            GleamFailureLayer::NotDependencyLayer
        );
    }

    /// An unsatisfiable requirement is the AUTHOR'S to fix. Excusing it as
    /// infrastructure would retry it pointlessly and then blame the registry
    /// for a real defect in `gleam.toml` — the mirror image of the bug this
    /// module exists to fix, and just as dishonest.
    #[test]
    fn an_unsatisfiable_requirement_is_not_excused_as_infrastructure() {
        let output = "error: Dependency resolution failed\n\nAn error occurred while choosing \
             the version of gleam_stdlib: no compatible version exists\n";
        assert_eq!(
            classify_gleam_failure(output),
            GleamFailureLayer::NotDependencyLayer
        );
    }

    #[test]
    fn a_resolution_failure_that_names_a_transport_fault_is_infrastructure() {
        let output = "error: Dependency resolution failed\n\nAn error occurred while choosing \
             the version of gleam_json: error sending request for url \
             (https://repo.hex.pm/packages/gleam_json)\n";
        assert_eq!(
            classify_gleam_failure(output),
            GleamFailureLayer::DependencyLayer
        );
    }

    /// 🔴 THE HAZARD THIS ANCHORING EXISTS FOR. A compile error quotes the
    /// author's source back, so a Gleam file containing the literal text of a
    /// registry error title would otherwise classify its own compile failure as
    /// an outage — and the build would be retried, then excused, forever.
    #[test]
    fn a_source_snippet_quoting_a_registry_title_does_not_decide_the_classification() {
        let output = "error: Syntax error\n\n  3 │   let message = \"HTTP error\"\n    │\
             \n    ╵ I was expecting a value here\n";
        assert_eq!(
            classify_gleam_failure(output),
            GleamFailureLayer::NotDependencyLayer,
            "a registry title inside a QUOTED SOURCE LINE must not classify the failure"
        );
    }

    /// Download progress is printed on a healthy build too, so it is not on its
    /// own evidence of anything.
    #[test]
    fn download_progress_alone_is_not_a_dependency_layer_failure() {
        let output = "  Downloading packages\n Downloaded 2 packages in 0.11s\n\
             error: Type mismatch\n";
        assert_eq!(
            classify_gleam_failure(output),
            GleamFailureLayer::NotDependencyLayer
        );
    }

    /// Non-vacuity: empty output must not be excused as infrastructure, or a
    /// `gleam` that dies silently would be retried and then blamed on the
    /// registry.
    #[test]
    fn empty_output_is_not_a_dependency_layer_failure() {
        assert_eq!(
            classify_gleam_failure(""),
            GleamFailureLayer::NotDependencyLayer
        );
    }
}
