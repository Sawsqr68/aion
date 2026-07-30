//! Per-role prompt assembly: the per-run CONTEXT block carrying the
//! activity's structured input JSON. The role's profile markdown is NOT part
//! of the prompt — it is the role's STATIC system prompt, passed to Norn once
//! per run via `--append-system-prompt` (which APPENDS to Norn's own system
//! instructions rather than overwriting them; see `composed_agent_harness` in
//! `main.rs`). These functions therefore assemble context only; the doctrine
//! reaches the agent out-of-band as system-prompt text, not folded into the
//! per-turn user message.
//!
//! DELIBERATELY SEPARATED AND DUMB (the dev-brief discipline, kept): each
//! role has exactly one obvious function whose body IS the interface. No
//! templating engine, no shared cleverness beyond the one context-section
//! renderer they all read as.
//!
//! # Why the gate-discipline section exists (task #236)
//!
//! Dev-brief runs burned fix cycles on pedantic clippy walls because the
//! developer never ran the brief's own gate commands in its workspace before
//! ending a turn. [`dev_item`] renders the exact configured gate argv into a
//! hard, prominent section so running it is the obvious last step of every
//! turn, not an optional confidence check.
//!
//! That section now states something that is TRUE. Until the `gate_item`
//! activity existed, the pipeline ran no battery at all: telling the dev agent
//! its work would be mechanically checked afterwards was false comfort, and a
//! dirty gate cost nothing. The battery now runs authoritatively after every
//! dev turn, its exit codes are recorded data, and a red battery makes the item
//! ineligible for acceptance no matter what any reviewer says — so the text
//! says exactly that, and no more.

use serde::Deserialize;

use crate::types::{GateCommand, GateRun, Lens};

/// The signature every role's assembly function shares:
/// `(context_json) -> per-turn prompt`. The profile is NOT a parameter — it
/// rides out-of-band as the session's `--append-system-prompt` text.
pub type AssembleFn = fn(&str) -> String;

/// The one renderer: a titled run-context section carrying the input JSON in
/// a fenced block. The doctrine is the session's system prompt, not part of
/// this text.
fn context_section(context_title: &str, context_json: &str) -> String {
    format!(
        "## {context_title}\n\nThe JSON below is this run's structured context. \
         Work from these structured artifacts — never from a prose summary of \
         them.\n\n```json\n{context_json}\n```\n"
    )
}

/// The slice of the dev context the gate-discipline section reads: `gates`,
/// this run's configured battery (`config.gates`, the same argv the item
/// profile tells the agent to run). Tolerant: a context that fails to parse
/// or omits the field renders as an empty battery (the discipline section
/// then says so explicitly) rather than panicking the prompt composer over a
/// rendering concern.
#[derive(Debug, Deserialize)]
struct DevGates {
    #[serde(default)]
    gates: Vec<GateCommand>,
}

fn configured_gates(context_json: &str) -> Vec<GateCommand> {
    serde_json::from_str::<DevGates>(context_json)
        .map(|parsed| parsed.gates)
        .unwrap_or_default()
}

/// The hard, prominent gate-discipline section appended after the dev run
/// context (see the module doc for why it exists: task #236). Lists this
/// run's configured gate argv VERBATIM, in prose, a second time — the dev
/// agent should never have to infer the exact command from a name.
fn gate_discipline_section(context_json: &str) -> String {
    let gates = configured_gates(context_json);
    let commands = if gates.is_empty() {
        "(this run's configured gate battery is empty — the operator's \
         explicit choice, a recorded vacuous pass; there is nothing to run, \
         but hold your own work to the same bar the gates would have \
         enforced)"
            .to_owned()
    } else {
        gates
            .iter()
            .map(|gate| format!("- {}: `{}`", gate.name, gate.argv.join(" ")))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "## GATE DISCIPLINE — DO THIS BEFORE YOU FINISH THIS TURN\n\n\
         Before you end ANY turn — the first round and every feedback \
         round — you MUST run every command below, VERBATIM, in your \
         workspace, and keep fixing and re-running until every one of them \
         is fully clean. This run's configured gate battery, in order:\n\n\
         {commands}\n\n\
         What happens after your turn ends, exactly: the pipeline runs this \
         same battery itself, in this same worktree, and RECORDS every exit \
         code as data. A non-zero exit makes the item ineligible for \
         acceptance no matter what the reviewers say, the failing output is \
         attached to your next round as feedback, and that costs one of the \
         run's bounded rounds. Then a panel of named review lenses reads the \
         diff and the battery's output and votes; every lens must accept \
         before the item is done.\n\n\
         So the battery is not advice and it is not a formality: it is the \
         measurement, and it runs whether you ran it or not. Running it \
         yourself is how you find out before it costs a round.\n\n\
         Note that a write-mode formatter in the battery may leave changes \
         behind; the machinery commits those for you and re-runs the whole \
         battery against the result. You still run no git.\n"
    )
}

// --- the role assembly functions ---------------------------------------------

/// The planner prompt (context only — the doctrine is the session's
/// `--append-system-prompt` text): the material, the repo root the session
/// is read-only rooted at, and the output charter.
#[must_use]
pub fn planner(context_json: &str) -> String {
    let context = context_section("Run context: material + repo root", context_json);
    format!(
        "{context}\n\
         ## Your job\n\n\
         Read the material and the repository (your working directory IS the \
         repo, read-only — ground the plan in the actual tree). Return the \
         typed plan: 3-10 work items, each with a git-ref-safe slug `id` \
         (lowercase, `[a-z0-9-]`), a one-sentence `goal`, `scope_in` (the \
         files/dirs the item MAY touch), `scope_out` (hard walls), an \
         integer `phase` (1 = no prerequisites), `depends_on` listing the \
         item ids whose MERGED output this item needs (a released \
         dependent's worktree is provisioned from the base branch WITH \
         those items' accepted branches merged in, so it really starts from \
         their work), and `feedback: \"\"`.\n\n\
         ## scope_in is MANDATORY on every item, and it is mechanical\n\n\
         `scope_in` must never be empty, and it must actually cover \
         everything the item will write, including files that do not exist \
         yet. It is not documentation: after the dev agent's turn the \
         machinery commits the round by staging every change to a file that \
         ALREADY exists, plus NEW files only where they fall inside that \
         item's `scope_in` (exact path, or anything under a directory you \
         name — there is no globbing, so `src/parser.rs` and `src/` both \
         work but `src/*.rs` matches nothing). A new file outside the fence \
         is left off the branch and reported by name. An item whose \
         `scope_in` is empty, or which creates its real work outside the \
         paths it declared, therefore produces a round the panel reviews as \
         an empty change — and a plan with any unfenced item is REJECTED \
         outright, before a single worktree is created.\n\n\
         Items in the same phase run IN PARALLEL in separate worktrees \
         against the same base branch — they MUST NOT touch the same files. \
         A later item sees an earlier item's work ONLY through `depends_on`: \
         an item that builds on, or edits the same files as, another item's \
         output MUST name that item in `depends_on`; a bare `phase` number \
         alone sequences nothing at the git level.\n\n\
         ## Output\n\n\
         The JSON schema is enforced; emit nothing but the object.\n"
    )
}

/// The dev-item prompt: the item context (including reviewer feedback when
/// cycling) plus the standing gate-discipline section.
#[must_use]
pub fn dev_item(context_json: &str) -> String {
    let context = context_section(
        "Run context: your work item (+ its feedback when cycling)",
        context_json,
    );
    let gate_discipline = gate_discipline_section(context_json);
    format!(
        "{context}\n\
         ## Your workspace and your turn\n\n\
         Your working directory IS this item's own git worktree on its own \
         branch. Implement `work.item.goal` strictly inside `scope_in`, \
         never touching `scope_out` — sibling items are being edited in \
         parallel, and an out-of-fence change manufactures merge conflicts \
         for the whole run. You run NO git: the machinery commits your work \
         after the turn and records the real hash.\n\n\
         If `work.item.feedback` is non-empty, this is your RESUMED session \
         continuing YOUR previous round: it was not accepted. The feedback \
         holds whichever of these applied — the gate battery's own failing \
         output, every review lens that rejected the round with its reason, \
         and any lens that failed to report at all. Address exactly those, \
         GATE FAILURES FIRST: they are objective, and they are usually the \
         cause of the findings.\n\n\
         {gate_discipline}\n\
         ## Report\n\n\
         One `claims` entry per acceptance-relevant point: concretely HOW \
         the diff meets it. The JSON schema is enforced; emit nothing but \
         the object.\n"
    )
}

/// The slice of the reviewer context the workspace and charter sections read:
/// the panel seat under `task` — its lens (name + charter, the lens's marching
/// orders) and the gated item's base commit and battery verdict. Tolerant: a
/// context that fails to parse or omits a key renders explicit placeholders
/// rather than failing the prompt composer over a rendering concern.
#[derive(Debug, Default, Deserialize)]
struct ReviewerContext {
    #[serde(default)]
    task: ReviewerTask,
}

#[derive(Debug, Default, Deserialize)]
struct ReviewerTask {
    #[serde(default)]
    lens: Lens,
    #[serde(default)]
    work: ReviewerWork,
}

#[derive(Debug, Default, Deserialize)]
struct ReviewerWork {
    #[serde(default)]
    base_commit: String,
    #[serde(default)]
    gate: ReviewerGate,
}

/// The slice of the battery outcome the reviewer is told about: the verdict,
/// each command's status, and the diagnostics.
#[derive(Debug, Default, Deserialize)]
struct ReviewerGate {
    #[serde(default)]
    pass: bool,
    #[serde(default)]
    runs: Vec<GateRun>,
    #[serde(default)]
    diagnostics: String,
}

fn reviewer_context(context_json: &str) -> ReviewerContext {
    serde_json::from_str::<ReviewerContext>(context_json).unwrap_or_default()
}

/// The reviewer workspace section, appended after the panel-seat run context.
/// It states the standing facts the reviewer role depends on: the session's
/// working directory IS the item worktree at the exact reviewed state, the
/// reviewer is READ-ONLY (file-mutating tools denied at the process
/// boundary), and the full diff is reconstructed with `git diff` against
/// the base commit.
fn reviewer_workspace_section(parsed: &ReviewerContext) -> String {
    let base_commit = parsed.task.work.base_commit.trim();
    let base = if base_commit.is_empty() {
        "<base_commit>"
    } else {
        base_commit
    };
    format!(
        "## YOUR WORKSPACE — the worktree under review\n\n\
         Your working directory IS the item's worktree at the EXACT state \
         you are reviewing (the round's work is committed and the mechanical \
         gate battery has already run against it; the tree is clean). You are \
         READ-ONLY: your file-writing tools are disabled — read, grep, and run \
         `git` freely, but do not attempt to modify anything.\n\n\
         Reconstruct the full change yourself, in your workspace:\n\n\
         ```\n\
         git diff {base}\n\
         ```\n\n\
         and read any file at its reviewed state directly. You MAY re-run the \
         configured gate commands read-only when their output would ground a \
         finding. Ground every finding in what the code and the real diff \
         actually say.\n"
    )
}

/// The battery section: the reviewer is told the RECORDED mechanical outcome,
/// because it is real data now and it grounds findings. A green battery is the
/// floor, not the ceiling — it says the code compiles and the existing tests
/// pass, and nothing whatever about whether the change is right. A red battery
/// is a fact the reviewer does not have to re-derive, and the item is already
/// ineligible for acceptance, so the useful contribution is the CAUSE.
fn reviewer_gate_section(parsed: &ReviewerContext) -> String {
    let gate = &parsed.task.work.gate;
    let statuses = if gate.runs.is_empty() {
        "(no gate command ran — the run's configured battery is empty, so \
         NOTHING was verified mechanically; treat every mechanical property as \
         unverified and hold the diff to it yourself)"
            .to_owned()
    } else {
        gate.runs
            .iter()
            .map(|run| {
                format!(
                    "- {} — exit {} ({}): `{}`",
                    run.name,
                    run.exit_code,
                    if run.passed { "pass" } else { "RED" },
                    run.argv.join(" ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let verdict = if gate.pass {
        "The battery PASSED. That is the floor: it means the code builds and \
         the existing checks are clean, and it says nothing at all about \
         whether the change is correct or complete. Do not treat it as \
         evidence in the change's favour."
    } else {
        "The battery is RED. The item is ALREADY ineligible for acceptance, so \
         do not spend your turn re-proving that. What is worth your turn is \
         the CAUSE, and anything else the red output exposes."
    };
    let diagnostics = if gate.diagnostics.trim().is_empty() {
        String::new()
    } else {
        format!(
            "\n\nRecorded diagnostics:\n\n```\n{}\n```",
            gate.diagnostics.trim()
        )
    };
    format!(
        "## THE MECHANICAL BATTERY ALREADY RAN\n\n\
         The pipeline ran the configured battery in this worktree after the dev \
         turn and recorded every exit code as data:\n\n\
         {statuses}\n\n\
         {verdict}{diagnostics}\n"
    )
}

/// The lens charter section: the lens's marching orders travel in the PROMPT,
/// not in the profile, because ONE reviewer connection serves EVERY lens — the
/// profile is the shared reviewing doctrine and the charter is what makes this
/// seat different from the seat running beside it.
fn lens_charter_section(parsed: &ReviewerContext) -> String {
    let name = parsed.task.lens.name.trim();
    let charter = parsed.task.lens.charter.trim();
    if name.is_empty() && charter.is_empty() {
        return "## YOUR LENS\n\nThis panel seat arrived with no lens name and no \
                charter. Report that as a blocking finding: a seat that does not \
                know what it is looking for cannot produce a trustworthy verdict.\n"
            .to_owned();
    }
    format!(
        "## YOUR LENS: {name}\n\n\
         You are ONE seat on a panel of named lenses reviewing this item \
         concurrently. You cannot see the other lenses' verdicts and they \
         cannot see yours — that blindness is the point. Every configured lens \
         must accept before the item is done, and the machinery stamps this \
         verdict with your lens name; do not assert it yourself.\n\n\
         YOUR CHARTER — this is what you are here to hunt, and staying inside \
         it is how the panel covers the whole surface without every seat \
         duplicating one generalist review:\n\n\
         {charter}\n\n\
         Findings outside your charter are not forbidden, but they are second \
         priority: another seat is looking there.\n"
    )
}

/// The review-item prompt: the panel seat's context, the workspace, the
/// recorded battery outcome, and the lens's charter.
#[must_use]
pub fn review_item(context_json: &str) -> String {
    let parsed = reviewer_context(context_json);
    let context = context_section(
        "Run context: your panel seat (lens + gated item + dev report)",
        context_json,
    );
    let workspace = reviewer_workspace_section(&parsed);
    let gate = reviewer_gate_section(&parsed);
    let charter = lens_charter_section(&parsed);
    format!(
        "{context}\n{workspace}\n{gate}\n{charter}\n\
         ## Standing rules for every lens\n\n\
         A `scope_out` violation is ALWAYS a blocking finding, whatever your \
         charter — sibling items are edited in parallel, so an out-of-fence \
         change manufactures merge conflicts for the whole run. The dev \
         report's `claims` are assertions, not facts; check the ones your \
         charter touches against the diff. Any blocking finding means overall \
         reject with a `reject_reason`; the fold DERIVES the overall from your \
         findings and records an asserted value that disagrees as a violation. \
         The JSON schema is enforced; emit nothing but the object.\n"
    )
}

/// The remediate prompt: the merge state + the plan, and the resumed-planner
/// framing.
#[must_use]
pub fn remediate(context_json: &str) -> String {
    let context = context_section("Run context: merge state + the plan", context_json);
    format!(
        "{context}\n\
         ## You are the planner, resumed\n\n\
         This session planned these items. A merge of their branches hit \
         the conflicts listed in `merge.conflicts`. Your working directory \
         IS the integration worktree with the conflicted merge IN \
         PROGRESS — the conflict markers are in the tree right now. Resolve \
         every conflict marker, preserving BOTH items' intent per the plan. \
         Do NOT run `git commit` — the machinery concludes the merge after \
         your turn.\n\n\
         ## Output\n\n\
         A summary (what conflicted, whose intent won where, why) plus the \
         list of files you resolved. The JSON schema is enforced; emit \
         nothing but the object.\n"
    )
}

/// The slice of the integration-fix context the failing-battery section reads:
/// the recorded [`crate::types::GateOutcome`] under `verify`. Tolerant for the
/// same reason as the reviewer's slice.
#[derive(Debug, Default, Deserialize)]
struct IntegrationFixContext {
    #[serde(default)]
    verify: ReviewerGate,
}

/// The failing battery, rendered for the integration fixer: which commands
/// went red, the exact argv, and the recorded output tail of each. The agent
/// gets the measured failure, not a summary of it.
fn failing_battery_section(context_json: &str) -> String {
    let verify = serde_json::from_str::<IntegrationFixContext>(context_json)
        .unwrap_or_default()
        .verify;
    let red: Vec<String> = verify
        .runs
        .iter()
        .filter(|run| !run.passed)
        .map(|run| {
            format!(
                "### gate `{}` exited {} running `{}`\n\n```\n{}\n```",
                run.name,
                run.exit_code,
                run.argv.join(" "),
                run.output_tail.trim()
            )
        })
        .collect();
    let body = if red.is_empty() {
        format!(
            "No individual gate command reported a non-zero exit, so the \
             verification failed on the tree assertion itself — the integration \
             worktree was not clean before the battery, or a gate command \
             DIRTIED it. The recorded diagnostics say which:\n\n```\n{}\n```",
            verify.diagnostics.trim()
        )
    } else {
        format!(
            "{}\n\nRecorded diagnostics:\n\n```\n{}\n```",
            red.join("\n\n"),
            verify.diagnostics.trim()
        )
    };
    format!("## THE FAILING BATTERY, AS RECORDED\n\n{body}\n")
}

/// The integration-fix prompt: the failing integration battery + the plan, and
/// the resumed-planner framing.
#[must_use]
pub fn fix_integration(context_json: &str) -> String {
    let context = context_section(
        "Run context: the failing integration verification + the plan",
        context_json,
    );
    let battery = failing_battery_section(context_json);
    format!(
        "{context}\n{battery}\n\
         ## You are the planner, resumed — and the union of your items is red\n\n\
         This session planned these items. Every one of them passed its own \
         battery on its own branch, and every one was accepted by the full \
         review panel. They then merged CLEANLY into the integration branch \
         `integration_branch` — and the battery re-run on the merged tree is \
         red. So this is not one item's bug: it is an interaction your \
         decomposition did not anticipate. Two items touching the same \
         callers, a test that only fails in combination, a lint that only \
         fires on the union.\n\n\
         Your working directory IS the integration worktree, on the \
         integration branch, with every item's work merged in. Fix the \
         failure there.\n\n\
         ## Hard rules\n\n\
         1. Make the battery pass on the MERGED tree. Run the failing \
         commands yourself, in your workspace, until they are clean — the \
         pipeline re-runs the whole battery after your turn and a red result \
         costs another bounded round.\n\
         2. Preserve every item's intent. You planned them all; a repair that \
         quietly reverts one item's work to make the build green defeats the \
         entire run. If two items genuinely cannot coexist as written, \
         compose the result your plan intended and SAY SO in your summary.\n\
         3. Minimal, interaction-focused edits. You are repairing an \
         interaction, not taking a development turn: change what the failure \
         requires and nothing else.\n\
         4. You run NO git. The machinery commits your repair on the \
         integration branch after your turn. You may read git state (`git \
         status`, `git diff`, `git log`) freely.\n\n\
         ## Output\n\n\
         A summary naming which items interacted, what the real cause was, and \
         what you changed — plus the list of files you touched in `resolved`. \
         The JSON schema is enforced; emit nothing but the object.\n"
    )
}

#[cfg(test)]
#[path = "prompts/tests.rs"]
mod tests;
