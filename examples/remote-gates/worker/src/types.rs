//! Wire types for the `remote_gates` actions.
//!
//! Every type here is one half of a contract: the other half is the matching
//! declaration in `remote_gates.awl`. The registry advertises schemas derived
//! from these structs, and `contract_true_registration_matches_the_awl_document`
//! in `main.rs` fails the build if the two halves ever drift apart.

use serde::{Deserialize, Serialize};

/// Input accepted by `clone_repo`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CloneInput {
    /// Git remote to clone or fetch from.
    pub repo: String,
    /// Branch whose tip the gates run against.
    pub branch: String,
}

/// The working tree the legs actually ran over.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Checkout {
    /// Absolute path of the resolved working tree.
    pub path: String,
    /// Resolved tip commit, so findings bind to a commit and not a branch name.
    pub commit: String,
    /// Branch that was checked out.
    pub branch: String,
    /// True when this run had to make a fresh clone; false when it reused and
    /// hard-reset an existing checkout (keeping its ignored build caches warm).
    pub fetched_fresh: bool,
    /// Free bytes on the filesystem holding the working tree, read after the
    /// checkout settled — a gate host that is filling up says so up front.
    pub free_disk_bytes: i64,
}

/// Input accepted by every gate leg.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LegInput {
    /// Working tree the leg runs its command in.
    pub path: String,
}

/// One diagnostic lifted from a leg's machine-readable stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Finding {
    /// Leg that produced this finding.
    pub leg: String,
    /// Diagnostic severity as the tool reported it (`error`, `warning`, ...).
    pub level: String,
    /// Source file the diagnostic points at, or `unknown` when the tool gave no
    /// span at all.
    pub file: String,
    /// One-based line of the primary span; absent when the tool gave no span.
    ///
    /// `with = "i64"` describes the VALUE schema, which is what AWL's `Int?`
    /// derives: optionality is the field's absence from `required`, never a
    /// nullable value. `skip_serializing_if` is what keeps the field absent
    /// rather than null on the wire, and it is also what keeps schemars from
    /// listing the field as required.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "i64")]
    pub line: Option<i64>,
    /// One-based column of the primary span; absent when the tool gave no span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "i64")]
    pub column: Option<i64>,
    /// Lint or diagnostic code, falling back to `compile-error` for a compiler
    /// message that carries none.
    pub code: String,
    /// The diagnostic text itself.
    pub message: String,
}

/// One leg's actual result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LegResult {
    /// Stable leg name (`check`, `clippy`, `nextest`, `fmt`).
    pub leg: String,
    /// Real process exit code, or `-1` when no process could be observed.
    pub exit_code: i32,
    /// True exactly when the process exited zero AND its stream was intact.
    pub passed: bool,
    /// Every diagnostic the leg's stream yielded — never truncated.
    pub findings: Vec<Finding>,
    /// How many well-formed records the leg's stream yielded. Zero findings
    /// with a zero denominator is a broken stream, not a green gate.
    pub denominator: i64,
    /// Absolute path of the leg's log file on the gate host.
    pub log_path: String,
    /// Last lines of the combined log, enough to read a failure without it.
    pub output_tail: String,
}

/// Input accepted by `notify_result`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NotifyInput {
    /// One-line verdict summary posted to the side channel.
    pub summary: String,
}

/// What the side channel did with the summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NotifyResult {
    /// True only when the notifier reported a successful post.
    pub delivered: bool,
    /// Why it was delivered, skipped, or refused — never silence.
    pub detail: String,
}
