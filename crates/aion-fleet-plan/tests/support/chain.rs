//! The R6 kill-and-resume harness.
//!
//! # ⛔ KILL SCOPE — how this file stays inside it
//!
//! The brief's kill scope is binding in its strictest reading, and the
//! venue is a shared box carrying other lanes' workers and a never-kill
//! server. This harness therefore:
//!
//! * kills **only processes it started itself**, with
//!   [`std::process::Command::spawn`], in this test;
//! * takes the pid **from the pid file that process wrote for itself**, read
//!   **AT FIRE TIME**, immediately before the kill — never a pid recited
//!   from earlier, never a pid remembered across a step;
//! * issues `kill -9 <exact-numeric-pid>` and nothing else. **There is no
//!   `pkill`, no `killall`, no pattern, no name match, and no process-tree
//!   sweep anywhere in this crate** — a fact the `no_pattern_kill_anywhere`
//!   test in `r6_replay_determinism.rs` asserts over this file's own source
//!   bytes;
//! * cross-checks that the pid it is about to kill is one it spawned, and
//!   REFUSES to kill otherwise.
//!
//! A kill this harness cannot trace to its own receipt is a kill it does
//! not perform.

use std::path::PathBuf;
use std::process::Command;

/// The scratch chain workspace one boundary walk runs in.
pub struct Workspace {
    pub root: tempfile::TempDir,
}

impl Workspace {
    pub fn new(graph_json: &str) -> Self {
        let root = tempfile::tempdir().unwrap_or_else(|e| unreachable!("no tempdir: {e}"));
        std::fs::create_dir_all(root.path().join("inbox"))
            .unwrap_or_else(|e| unreachable!("cannot create inbox: {e}"));
        std::fs::write(root.path().join("graph.json"), graph_json)
            .unwrap_or_else(|e| unreachable!("cannot write graph: {e}"));
        Self { root }
    }

    pub fn ledger_path(&self) -> PathBuf {
        self.root.path().join("ledger.json")
    }

    /// The ledger's exact bytes, or empty before the first write.
    pub fn ledger_bytes(&self) -> Vec<u8> {
        std::fs::read(self.ledger_path()).unwrap_or_default()
    }

    /// Deposit an external result at the barrier.
    pub fn deposit(&self, name: &str, json: &str) {
        std::fs::write(self.root.path().join("inbox").join(name), json)
            .unwrap_or_else(|e| unreachable!("cannot deposit {name}: {e}"));
    }

    pub(super) fn command(&self, root_base: &str, deadline_secs: u64) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_aion-fleet-chain-step"));
        cmd.arg("--graph")
            .arg(self.root.path().join("graph.json"))
            .arg("--ledger")
            .arg(self.ledger_path())
            .arg("--root-base")
            .arg(root_base)
            .arg("--inbox")
            .arg(self.root.path().join("inbox"))
            .arg("--deadline-secs")
            .arg(deadline_secs.to_string())
            .arg("--poll-millis")
            .arg("10");
        cmd
    }

    /// Run one step to completion and return its JSON stdout.
    pub fn step(&self, root_base: &str) -> serde_json::Value {
        let out = self
            .command(root_base, 20)
            .output()
            .unwrap_or_else(|e| unreachable!("cannot run the step binary: {e}"));
        let text = String::from_utf8_lossy(&out.stdout);
        serde_json::from_str(text.trim()).unwrap_or_else(|e| {
            unreachable!(
                "step stdout is not JSON ({e}): {text}\nstderr: {}",
                String::from_utf8_lossy(&out.stderr)
            )
        })
    }
}

// ------------------------------------------------------- shared fixtures

/// The design §9.1 three-stage block, verbatim — the #140 exemplar.
pub const EXEMPLAR: [&str; 3] = [
    "stage id=l1 legs=l1-design after=- admit=always conclusions=design-ready,measured-red conclusion_leg=l1-design advance=auto",
    "stage id=l2 legs=l2-classify after=l1 admit=always conclusions=classification-ready conclusion_leg=l2-classify advance=auto",
    "stage id=l3 legs=l3-route after=l2 admit=l1.conclusion!=measured-red conclusions=route-ready conclusion_leg=l3-route advance=auto",
];

/// The #140 exemplar's dispatch-fixed leg ids.
pub const EXEMPLAR_LEGS: [&str; 3] = ["l1-design", "l2-classify", "l3-route"];

/// The #140 exemplar's normalized graph, produced by the REAL validator —
/// so every chain test orders itself by the same authority the run does.
pub fn exemplar_graph() -> aion_fleet_plan::graph::NormalizedGraph {
    let plan = format!(
        "# PLAN\n\n```fleet-stages-v1\n{}\n```\n",
        EXEMPLAR.join("\n")
    );
    let inputs = aion_fleet_plan::validate::PlanInputs {
        plan_text: plan,
        dispatch_leg_ids: EXEMPLAR_LEGS.iter().map(|s| (*s).to_owned()).collect(),
        fleet_state_loose: false,
        loose_detail: String::new(),
        max_stages: None,
    };
    match aion_fleet_plan::validate::check_plan(&inputs) {
        Ok(aion_fleet_plan::validate::CheckOutcome::Valid(graph)) => *graph,
        other => unreachable!("the #140 exemplar block did not validate: {other:?}"),
    }
}

/// The exemplar graph, serialized for a harness workspace.
pub fn exemplar_graph_json() -> String {
    serde_json::to_string_pretty(&exemplar_graph())
        .unwrap_or_else(|e| unreachable!("cannot encode the exemplar graph: {e}"))
}

/// A distinct 40-hex commit id per label, so a test that mixes two SHAs up
/// fails loudly instead of passing on a shared placeholder.
pub fn sha(label: u8) -> String {
    let d = char::from(b'0' + (label % 10));
    std::iter::repeat_n(d, 40).collect()
}
