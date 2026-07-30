//! The gate-battery core and the `gate_item` handler — the fix for the
//! pipeline's worst gap: before this, `config.gates` was passed into the dev
//! AGENT's prompt and NOTHING ran it authoritatively. An item was accepted on
//! the strength of a reviewer agent's opinion about a dev agent's claim.
//!
//! The battery core ([`run_gate_battery`]) is the ONE place gate commands
//! execute — shared by `gate_item` here and `verify_integration` in
//! [`super::verify`] — so the `{base_commit}` substitution, the resolved-argv
//! recording, and the progress reporting can never diverge between the
//! per-item and the integration stage. Ported from
//! `examples/dev-brief/worker/src/handlers/gates.rs`, whose semantics are the
//! residue of real failures.
//!
//! FAILURE TAXONOMY: a non-zero exit is RECORDED DATA the caller interprets —
//! never an error, because a red gate is exactly the evidence this stage
//! exists to produce. Only a command that cannot RUN (empty argv, missing
//! executable, dead working directory) or a `git` infrastructure failure is
//! terminal.

use aion_worker::ActivityFailure;

use crate::commit;
use crate::shell::Shell;
use crate::types::{GateCommand, GateItemInput, GateOutcome, GateRun, GatedItem};

use super::progress::GateProgress;
use super::support::{clip, require_can_run, require_run};

/// One gate command's RAW (untruncated) execution record: the resolved argv,
/// the exit status, and the full combined output. Clipped into the event's
/// [`GateRun`] by the handlers.
pub(super) struct RawGateRun {
    pub(super) name: String,
    pub(super) argv: Vec<String>,
    pub(super) exit_status: i32,
    pub(super) output: String,
}

impl RawGateRun {
    /// Whether the command exited zero.
    pub(super) fn passed(&self) -> bool {
        self.exit_status == 0
    }

    /// The clipped, event-sized record carried in the outcome.
    pub(super) fn clipped(&self) -> GateRun {
        GateRun {
            name: self.name.clone(),
            argv: self.argv.clone(),
            exit_code: i64::from(self.exit_status),
            passed: self.passed(),
            output_tail: clip(&self.output),
        }
    }
}

/// Run a gate battery in `workspace`, in order: the ONE place gate commands
/// are executed.
///
/// The literal token `{base_commit}` in EVERY argv element is replaced with
/// the run's recorded base commit before execution, so a scope fence pins
/// itself to the provisioned base without the operator re-typing a SHA per
/// dispatch (a battery with a literal SHA is unaffected — there is no token
/// to replace). The RESOLVED argv is recorded on each run so the evidence
/// shows the real command, never the template.
///
/// An empty argv can never run and can never be a verdict: it is a
/// configuration fault, surfaced loudly. Each command's start and its result
/// are reported on `progress` so an operator watching a multi-minute battery
/// sees which command is in flight.
///
/// # Errors
///
/// Terminal when a gate has an empty argv or a command cannot RUN at all.
pub(super) fn run_gate_battery(
    shell: &Shell,
    progress: &dyn GateProgress,
    workspace: &str,
    base_commit: &str,
    gates: &[GateCommand],
) -> Result<Vec<RawGateRun>, ActivityFailure> {
    let total = gates.len();
    let mut raw: Vec<RawGateRun> = Vec::with_capacity(total);
    for (index, gate) in gates.iter().enumerate() {
        let resolved: Vec<String> = gate
            .argv
            .iter()
            .map(|element| element.replace("{base_commit}", base_commit))
            .collect();
        let Some((executable, rest)) = resolved.split_first() else {
            return Err(ActivityFailure::terminal(format!(
                "gate `{}` has an empty argv — nothing to execute",
                gate.name
            )));
        };
        progress.report(&format!(
            "gate {}/{total} `{}` running: {}",
            index + 1,
            gate.name,
            resolved.join(" ")
        ));
        let args: Vec<&str> = rest.iter().map(String::as_str).collect();
        let run = require_can_run(
            shell,
            executable,
            &args,
            workspace,
            &format!("gate `{}`", gate.name),
        )?;
        progress.report(&format!(
            "gate {}/{total} `{}` exited {} ({})",
            index + 1,
            gate.name,
            run.exit_status,
            if run.succeeded() { "pass" } else { "RED" }
        ));
        raw.push(RawGateRun {
            name: gate.name.clone(),
            argv: resolved,
            exit_status: run.exit_status,
            output: run.output,
        });
    }
    Ok(raw)
}

/// The change the panel reads: worktree vs `base_commit`, so committed AND
/// any residual uncommitted work are both visible.
///
/// `--no-ext-diff` is LOAD-BEARING, not tidiness: this repository configures
/// an external diff driver that emits no `+`/`-` prefixes, so without the flag
/// the recorded evidence a review lens reads would not be a unified diff at
/// all. An operator's difftastic/pager configuration must never shape durable
/// workflow evidence.
fn panel_diff(
    shell: &Shell,
    workspace: &str,
    base_commit: &str,
) -> Result<String, ActivityFailure> {
    let diff = require_run(
        shell,
        "git",
        &["-C", workspace, "diff", "--no-ext-diff", base_commit],
        workspace,
        "git diff (panel evidence)",
    )?;
    Ok(clip(&diff.stdout))
}

/// `gate_item`: run the configured battery in the item's worktree, FULLY
/// MECHANICAL, and record every exit code as data.
///
/// 1. Every configured command runs in order in the worktree root; pass =
///    exit 0. All commands always run, so a loop-back carries the full
///    picture rather than stopping at the first red.
/// 2. A command that MUTATES the worktree (a write-mode formatter is the
///    expected case) leaves normalization the dev agent never saw: it is
///    committed mechanically under the gate machinery identity so the branch
///    stays complete and the item's next re-provisioning does not refuse a
///    dirty worktree.
/// 3. When that commit moved HEAD, the FULL battery is re-run against the
///    post-normalization HEAD and THOSE statuses become the recorded verdict.
///    The normalization commit can flip a `{base_commit}`-pinned scope fence
///    (or any HEAD-sensitive gate) red, and the commit the panel reviews must
///    never be recorded green on a verdict it never earned. The re-run is
///    bounded to a single pass: a gate that re-dirties the tree is itself a
///    recorded failure, not an unbounded normalize/re-run loop.
/// 4. An EMPTY battery is the operator's explicit choice: a recorded VACUOUS
///    pass, named in `diagnostics`, never silent.
///
/// THE FENCE, and why the re-dirty assertion asks the staging rule rather than
/// `git status`. Normalization stages every tracked change plus untracked files
/// inside the item's own `scope_in` (see [`crate::commit`]); a gate battery's
/// build output is untracked and outside it, so it is left alone and named in
/// `diagnostics`. The post-normalization assertion therefore has to ask the
/// SAME question the commit answered — "is anything the rule would stage still
/// sitting in the tree?" — because a bare `git status --porcelain` would see
/// that deliberately-untouched build output, call it a re-dirtied worktree, and
/// fail every battery in exactly the repositories this fence exists to unblock.
///
/// # Errors
///
/// Terminal only when a gate has an empty argv, a command cannot RUN at all,
/// or `git` fails as infrastructure.
pub fn gate_item(
    shell: &Shell,
    progress: &dyn GateProgress,
    input: GateItemInput,
) -> Result<GatedItem, ActivityFailure> {
    let GateItemInput { work, gates } = input;
    let workspace = work.workspace_path.as_str();
    let base_commit = work.base_commit.as_str();
    let fence = commit::ScopeFence::new(&work.item.scope_in);

    // Judge the dev agent's HEAD.
    let raw = run_gate_battery(shell, progress, workspace, base_commit, &gates)?;

    let normalization = commit::commit_gate_normalization(shell, workspace, &fence)
        .map_err(ActivityFailure::terminal)?;

    let mut notes: Vec<String> = Vec::new();
    push_exclusion_note(
        &mut notes,
        "the gate-normalization commit",
        &fence,
        normalization.excluded(),
    );
    let mut redirtied = false;
    let raw = match &normalization {
        commit::FixCommitOutcome::Committed { commit, paths, .. } => {
            notes.push(format!(
                "gate commands normalized the tree; committed mechanically as {commit} \
                 ({} path(s))",
                paths.len()
            ));
            let pre_green = raw.iter().all(RawGateRun::passed);
            progress.report(
                "the normalization commit moved HEAD — re-running the FULL battery against it",
            );
            let rerun = run_gate_battery(shell, progress, workspace, base_commit, &gates)?;
            let survey = commit::survey_worktree(shell, workspace, &fence)
                .map_err(ActivityFailure::terminal)?;
            if survey.diverges() {
                redirtied = true;
                notes.push(format!(
                    "a gate command re-dirtied the worktree on the post-normalization \
                     re-run (a non-idempotent formatter): the reviewed commit does not \
                     embody these changes — recorded as a gate failure, looping back to \
                     the dev agent. Paths left changed:\n{}",
                    survey
                        .divergent()
                        .iter()
                        .map(|path| format!("  {path}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                ));
            }
            push_exclusion_note(
                &mut notes,
                "the post-normalization clean assertion",
                &fence,
                survey.excluded(),
            );
            if pre_green && !rerun.iter().all(RawGateRun::passed) {
                notes.push(
                    "the post-normalization re-run flipped a gate RED — the dev agent's \
                     HEAD was green but the normalization commit is not; looping back \
                     with the post-normalization statuses"
                        .to_owned(),
                );
            }
            rerun
        }
        commit::FixCommitOutcome::Skipped { .. } => raw,
    };

    let gate = summarize(
        &raw,
        notes,
        redirtied,
        panel_diff(shell, workspace, base_commit)?,
    );
    Ok(GatedItem {
        item: work.item,
        workspace_path: work.workspace_path,
        branch: work.branch,
        base_commit: work.base_commit,
        report: work.report,
        gate,
    })
}

/// Record, in the battery's own notes, every untracked path the staging fence
/// turned away — on a SKIP as loudly as on a commit. `summarize` guarantees
/// `diagnostics` is never empty and carries every note whether the battery
/// passed or not, so this is the recorded outcome the reviewers read, not a
/// log line they never see.
///
/// Shared with [`super::verify`] deliberately: both stages must say the same
/// thing, in the same words, about the same fact.
pub(super) fn push_exclusion_note(
    notes: &mut Vec<String>,
    step: &str,
    fence: &commit::ScopeFence,
    excluded: &[String],
) {
    if let Some(note) = commit::exclusion_note(step, fence, excluded) {
        notes.push(note);
    }
}

/// Fold the raw battery records, the stage's notes, and the diff into the
/// recorded [`GateOutcome`].
///
/// `pass` is true only when every run passed AND `tree_diverged` is false —
/// that flag is how both stages say "the worktree does not equal the branch
/// head", which makes any green the battery reported untrustworthy: in
/// `gate_item` a non-idempotent formatter re-dirtied the tree after the
/// normalization commit; in `verify_integration` a gate mutated the merged
/// tree (or it was already dirty going in).
///
/// `diagnostics` is NEVER empty — a vacuous pass says so on its face, and
/// every note survives whether the battery passed or not.
pub(super) fn summarize(
    raw: &[RawGateRun],
    notes: Vec<String>,
    tree_diverged: bool,
    diff: String,
) -> GateOutcome {
    let mut runs: Vec<GateRun> = Vec::with_capacity(raw.len());
    let mut adverse: Vec<String> = Vec::new();
    for record in raw {
        if !record.passed() {
            adverse.push(format!(
                "gate `{}` exited {}:\n{}",
                record.name,
                record.exit_status,
                clip(record.output.trim())
            ));
        }
        runs.push(record.clipped());
    }
    let ran = runs.len();
    let pass = !tree_diverged && runs.iter().all(|run| run.passed);
    let mut lines: Vec<String> = Vec::new();
    if pass {
        lines.push(if ran == 0 {
            "no gates configured — VACUOUS pass: nothing was verified \
             mechanically. This is the operator's explicit choice (an empty \
             config.gates), recorded so it can never be mistaken for a green \
             battery."
                .to_owned()
        } else {
            format!("all {ran} gate command(s) green")
        });
    } else if ran == 0 {
        lines.push(
            "no gates configured, and the battery still did not pass — see the \
             notes below"
                .to_owned(),
        );
    }
    lines.extend(adverse);
    lines.extend(notes);
    GateOutcome {
        pass,
        runs,
        diff,
        diagnostics: lines.join("\n\n"),
    }
}
