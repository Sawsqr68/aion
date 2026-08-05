//! The refusal an operator reads when retained package versions disagree about
//! an action's declared body — and the remedy it names.
//!
//! Content-hash namespacing keeps every deployed version of a document alive at
//! once, so editing a declared command body and deploying again leaves TWO
//! retained versions declaring the same action with different commands. Running
//! either would guess which deploy the workflow meant, so the dispatch is
//! refused.
//!
//! Refusing is the easy half. The refusal used to say *"redeploy so one body
//! remains"*, which cannot be followed: redeploying is exactly what created the
//! second body, and the route-active version cannot be unloaded (the deploy API
//! answers `RouteActive`). The operator is then stuck holding a terminal error
//! whose only instruction makes the problem worse.
//!
//! What actually clears it is retiring the SUPERSEDED versions — which needs
//! their content hashes, which the operator does not have and the message did
//! not carry. So the message carries them now, as runnable `aion unload`
//! commands, and distinguishes the two situations that need different moves:
//! a superseded version left behind (retire it) from two route-active packages
//! that genuinely disagree (nothing to retire — the action name is shared).

use std::collections::BTreeSet;

/// One retained package version that declares a body for the ambiguous action,
/// carrying enough identity to be named in a command the operator can run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaringVersion {
    /// Content hash of the package version, in the canonical 64-character form.
    /// Never abbreviated: the deploy API parses the whole hash and refuses a
    /// short one, so a truncated hash would print a command that cannot run.
    pub content_hash: String,
    /// Every workflow type this exact version implements, sorted. Unload is
    /// keyed by `(workflow_type, content_hash)`, so a package archive carrying
    /// several entry modules needs one command per type.
    pub workflow_types: Vec<String>,
    /// Whether new starts still route to this version. A route-active version
    /// cannot be unloaded, so it is the one the operator keeps.
    pub route_active: bool,
    /// Which distinct body this version declares, as an index into the order
    /// the bodies were first seen. Versions sharing an index agree.
    pub body: usize,
}

/// Builds the terminal refusal for `action` on `task_queue`, given every
/// retained version that declares a body for it.
///
/// `declaring` is expected to hold at least two distinct `body` indices — that
/// is what makes the lookup ambiguous — but the text is written so a shorter
/// list still reads as a true sentence rather than a malformed one.
#[must_use]
pub fn ambiguous_body_refusal(
    action: &str,
    task_queue: &str,
    declaring: &[DeclaringVersion],
) -> String {
    let distinct: BTreeSet<usize> = declaring.iter().map(|version| version.body).collect();
    let live: BTreeSet<usize> = declaring
        .iter()
        .filter(|version| version.route_active)
        .map(|version| version.body)
        .collect();

    let remedy = remedy_for(declaring, &live);

    format!(
        "terminal:action `{action}` on task queue `{task_queue}` declares {bodies} different \
         bodies across {versions} retained package versions; refusing to guess which deploy this \
         run meant. {remedy}",
        bodies = distinct.len(),
        versions = declaring.len(),
    )
}

/// The sentence that tells the operator what to actually do, chosen from what
/// the retained set looks like. `live` holds the body indices that route-active
/// versions carry.
fn remedy_for(declaring: &[DeclaringVersion], live: &BTreeSet<usize>) -> String {
    if live.len() > 1 {
        // Nothing is superseded here: two packages that new starts can both
        // reach disagree about what this action name means. Unloading either
        // would break a live route, so the remedy is an authoring change.
        return format!(
            "Every disagreeing version is route-active, so there is no superseded version to \
             retire — {live_versions} are all reachable by new starts and declare different \
             commands for the same action name. Give the action a distinct name in each document, \
             or declare the same body in both.",
            live_versions = route_active_names(declaring),
        );
    }

    let superseded = unload_commands(declaring);
    if superseded.is_empty() {
        // Every declaring version is route-active and they agree, so the
        // ambiguity is not between deploys — say what was actually seen rather
        // than prescribing a move that has no target.
        return "No superseded version is retained, so there is nothing to unload; the \
                disagreement is inside the route-active set and needs the documents themselves \
                reconciled."
            .to_owned();
    }

    if live.is_empty() {
        // Nothing routes new starts any more: the run in hand came from a
        // version that has been superseded out of routing entirely. Deploying
        // first is what gives the operator a version to keep.
        return format!(
            "No retained version is route-active, so deploy the document you mean first — that \
             makes its version the one new starts reach — then retire the rest and start the run \
             again: {superseded}.",
            superseded = superseded.join(", "),
        );
    }

    format!(
        "The route-active version already carries the body new starts use; retiring the \
         superseded ones is what leaves a single body. Redeploying adds a version rather than \
         removing one, so unload instead, then start the run again: {superseded}. Kept, because \
         new starts route to it: {kept}.",
        superseded = superseded.join(", "),
        kept = route_active_names(declaring),
    )
}

/// One runnable `aion unload` command per (type, version) catalog entry that is
/// safe to retire, in the order the versions were given.
///
/// Every superseded entry is listed. A cap would drop exactly the command the
/// operator needs, and a silent one would read as a complete instruction.
fn unload_commands(declaring: &[DeclaringVersion]) -> Vec<String> {
    declaring
        .iter()
        .filter(|version| !version.route_active)
        .flat_map(|version| {
            version.workflow_types.iter().map(move |workflow_type| {
                format!(
                    "`aion unload {workflow_type} {hash}`",
                    hash = version.content_hash
                )
            })
        })
        .collect()
}

/// Names the route-active versions as `type@hash`, one entry per workflow type
/// so the reader can match a name to a document.
fn route_active_names(declaring: &[DeclaringVersion]) -> String {
    let names: Vec<String> = declaring
        .iter()
        .filter(|version| version.route_active)
        .flat_map(|version| {
            version.workflow_types.iter().map(move |workflow_type| {
                format!("{workflow_type}@{hash}", hash = version.content_hash)
            })
        })
        .collect();
    if names.is_empty() {
        // Unreachable from the call sites, which both test the same predicate
        // first. Named rather than left as an empty gap so a refactor cannot
        // produce a sentence that trails off.
        return "no version".to_owned();
    }
    names.join(", ")
}

#[cfg(test)]
mod tests {
    use super::{DeclaringVersion, ambiguous_body_refusal};

    const OLD: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const NEW: &str = "2222222222222222222222222222222222222222222222222222222222222222";

    fn version(hash: &str, types: &[&str], route_active: bool, body: usize) -> DeclaringVersion {
        DeclaringVersion {
            content_hash: hash.to_owned(),
            workflow_types: types.iter().map(|name| (*name).to_owned()).collect(),
            route_active,
            body,
        }
    }

    #[test]
    fn the_remedy_names_the_superseded_version_as_a_runnable_command() {
        let refusal = ambiguous_body_refusal(
            "find_repositories",
            "local",
            &[
                version(OLD, &["git_status_sweep"], false, 0),
                version(NEW, &["git_status_sweep"], true, 1),
            ],
        );
        assert!(refusal.starts_with("terminal:"), "{refusal}");
        assert!(
            refusal.contains(&format!("`aion unload git_status_sweep {OLD}`")),
            "the superseded version must be named as a command: {refusal}"
        );
        assert!(
            !refusal.contains(&format!("`aion unload git_status_sweep {NEW}`")),
            "the route-active version cannot be unloaded and must not be told to: {refusal}"
        );
        assert!(
            refusal.contains(&format!("git_status_sweep@{NEW}")),
            "the kept version must be identified: {refusal}"
        );
    }

    #[test]
    fn the_remedy_never_tells_the_operator_to_redeploy_into_the_problem() {
        // The defect this text replaced: "redeploy so one body remains" is the
        // move that created the second body.
        let refusal = ambiguous_body_refusal(
            "find_repositories",
            "local",
            &[
                version(OLD, &["sweep"], false, 0),
                version(NEW, &["sweep"], true, 1),
            ],
        );
        assert!(
            !refusal.contains("redeploy so one body remains"),
            "{refusal}"
        );
        assert!(
            refusal.contains("Redeploying adds a version rather than removing one"),
            "the message must say why the obvious move is wrong: {refusal}"
        );
    }

    #[test]
    fn a_version_with_several_entry_types_earns_one_command_per_type() {
        let refusal = ambiguous_body_refusal(
            "build",
            "local",
            &[
                version(OLD, &["alpha", "beta"], false, 0),
                version(NEW, &["alpha"], true, 1),
            ],
        );
        assert!(
            refusal.contains(&format!("`aion unload alpha {OLD}`")),
            "{refusal}"
        );
        assert!(
            refusal.contains(&format!("`aion unload beta {OLD}`")),
            "{refusal}"
        );
    }

    #[test]
    fn two_route_active_packages_are_told_to_reconcile_not_to_unload() {
        let refusal = ambiguous_body_refusal(
            "build",
            "local",
            &[
                version(OLD, &["alpha"], true, 0),
                version(NEW, &["beta"], true, 1),
            ],
        );
        assert!(
            !refusal.contains("aion unload"),
            "unloading a route-active version is refused, so it must not be prescribed: {refusal}"
        );
        assert!(
            refusal.contains("distinct name"),
            "the remedy for a live collision is an authoring change: {refusal}"
        );
        assert!(refusal.contains(&format!("alpha@{OLD}")), "{refusal}");
        assert!(refusal.contains(&format!("beta@{NEW}")), "{refusal}");
    }

    #[test]
    fn with_nothing_route_active_the_deploy_comes_before_the_unload() -> Result<(), String> {
        let refusal = ambiguous_body_refusal(
            "build",
            "local",
            &[
                version(OLD, &["alpha"], false, 0),
                version(NEW, &["alpha"], false, 1),
            ],
        );
        let deploy = refusal
            .find("deploy the document you mean first")
            .ok_or_else(|| format!("the deploy step must be named: {refusal}"))?;
        let unload = refusal
            .find("aion unload")
            .ok_or_else(|| format!("the unload step must be named: {refusal}"))?;
        assert!(
            deploy < unload,
            "deploy must be prescribed before unload: {refusal}"
        );
        Ok(())
    }

    #[test]
    fn the_counts_report_bodies_and_versions_separately() {
        // Three versions, two distinct bodies: the operator needs both numbers
        // to know that retiring one version is not enough.
        let refusal = ambiguous_body_refusal(
            "build",
            "local",
            &[
                version(OLD, &["alpha"], false, 0),
                version(NEW, &["alpha"], false, 0),
                version(
                    "3333333333333333333333333333333333333333333333333333333333333333",
                    &["alpha"],
                    true,
                    1,
                ),
            ],
        );
        assert!(
            refusal.contains("declares 2 different bodies across 3 retained package versions"),
            "{refusal}"
        );
    }
}
