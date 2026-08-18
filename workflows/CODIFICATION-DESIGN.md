# Codification design surface — the development process as AWL workflows

Status: DESIGN FOR REVIEW (Waffles), per Tom's go 2026-08-18 22:16Z. No AWL authored yet.
Author: Vesper. Reference trace: the #75 arc (COMPACT-66-STATE.md 21:0xZ–22:1xZ entries + issues #75/#76).

Tom's words carried into every decision here: separate single-purpose workflows plus
"the workflow that brings them all together", and "I don't think we can rely on scripts
that are local. I think the commands need to be read up, written up, whole cloth."

## 1. The workflow set

Six workflows. Five single-purpose, each independently invocable; one orchestrator.

### 1.1 `investigate` (rework of tonight's proven shape)
Input: issue ref (tracker, repo, number), repo url, ref. Steps: clone at ref → control
question (typed admission) → investigation agent → typed report read-back (schema-validated)
→ file the report as a tracker comment → teardown.
**Structural change the criteria demand:** the terminal step is a HANDOFF, not a file.
The workflow's declared result type is a union: `Handoff { verdict, next: FixFlight | Close |
Escalate }` or `Park { reason: named-park-reason }`. There is no terminal arm whose payload
is a bare verdict — the type does not exist in the result union, so "investigation ends as
a verdict file" is not writable. The orchestrator consumes the handoff; standalone invocation
still files the tracker comment (visibility) but the workflow itself completes only through
the union.

### 1.2 `fix_flight` (rework of fleet_dev)
Input: issue ref + verdict payload (from investigate's handoff), repo, base ref, result
branch name, leg specs. Steps: plan (counsel) → parallel dev legs (each leg names its own
repo and carries its brief) → gate (child workflow, §1.3 — not an in-repo script) → judge →
bounded fix rounds re-entering the gate child → push result branch → report.
**Change from tonight:** the gate is a CHILD WORKFLOW invocation, not a `.fleet-gate.sh`
committed to the branch. No branch carries executable process artifacts.

### 1.3 `gate`
Input: repo, ref (or in-place tree handle from a parent), and a gate spec: an ordered list
of legs, each leg = `{ name, argv (whole cloth, written out), timeout: none-by-default,
red_classifier }`. Output: typed per-leg results `{ name, rc, log_artifact }` — the rc is
the step's recorded result in workflow history, which IS the exit-file discipline expressed
as vocabulary: no shell writes `rc=$?` to a manifest file, the engine records the activity
result. Contended-red-is-not-red is a declared retry arm: a leg may declare
`retry_on_contention { detector, max_rounds }` so an EMFILE/lld-class red re-runs by policy,
visibly, in history.

### 1.4 `land`
Input: source (result branch or commit set), target (main), issue refs to close, battery
spec (a gate spec). Steps: fetch target → divergence check (target moved → merge and the
battery runs ON THE MERGED TREE, the diverged-tree law as control flow) → gate child on the
landed tree → push → close each issue WITH the receipt (landing hash + battery citation) in
one step — the close step's input type requires the receipt, so a receiptless close is not
writable → target-dir cleanup as a step in the same run.

### 1.5 `release_ceremony` (implements #76 + addendum)
Input: version, repo, expected member set. Steps: version cut (workspace bump + lock settle)
→ full-graph topo order computed from normal + build + versioned dev dependencies, with
authoring-time refusal of versioned dev-dep cycles naming the strip-the-version remedy →
per-crate: publish, then index-confirm (checksum recorded as the step result) BEFORE any
dependent publishes — ordering enforced by data dependency between steps, not by a script's
loop → tag pushed ONLY after all members index-confirmed (the tag step's input is the full
checksum set, so tag-without-publish is not writable — the #76 drift becomes inexpressible)
→ from-index install verification → optional exhibit replay leg.

### 1.6 `issue_lifecycle` (the one that brings them all together)
Input: issue ref (or a mint payload that creates the issue first — tracker-first). Chains
children: `investigate` → on `Handoff{next: FixFlight}` → `fix_flight` → `land` → close →
optionally joins a `release_ceremony` over the accumulated landing set. On `Park{reason}`
the lifecycle completes as `Parked{reason}` — visible, named, resumable. No stage boundary
requires a human relay; every stage boundary is a child-workflow result feeding the next
child's input. A human can still intervene (signal/cancel) — the design removes the NEED
for relay, not the ability to steer.

## 2. Whole-cloth command rule (acceptance criterion 2)

Every argv is written out in full inside the AWL step definition. Concretely:
- No step references a script path on any box. The gate spec's `argv` is the complete
  command text; the worker materializes and runs it, records rc + output as the activity
  result. Nothing lives on Tom's Mac; all execution routes server → fleet workers.
- Exit-file discipline: abolished as an idiom. Tonight's `echo "rc=$?" >> MANIFEST` exists
  because detached shells lose exit codes. In AWL the activity result IS the recorded rc —
  the discipline becomes the engine's event history. A step that wants a manifest gets it
  as a rendered VIEW of history, never as the source of truth.
- Environment hardening (PATH, HOME, ulimit) stops being per-script boilerplate: it is the
  worker document's declared environment, stated once, versioned with the deploy.

## 3. Vocabulary → failure-class mapping (acceptance criterion 3)

NOTE FOR REVIEW: I hold classes 4 and 8 verbatim from Waffles; the other six below are my
reconstruction from tonight's record, labeled (r). Waffles: send the canonical inventory
verbatim and I will reconcile this table to it before authoring — the mapping must be
against YOUR eight, not my reconstruction.

| # | Failure class | Where it becomes inexpressible |
|---|---------------|-------------------------------|
| 1 (r) | Local scripts / process-on-one-box (gate scripts, battery files, dispatch templates) | Whole-cloth rule: no step type takes a script path; gate spec argv is inline. The vocabulary has no "run this file" verb. |
| 2 (r) | Silent exit / lost rc (exit-file idiom, `\| head` truncation, markers that lie) | Activity result = recorded rc + full output artifact in history. A step with no recorded result cannot advance the chain — the engine, not discipline, enforces it. |
| 3 (r) | Verdict-terminal investigations (work that ends as a file nobody consumes) | `investigate`'s result union has no bare-verdict arm: Handoff or named Park only. The orchestrator's next stage takes the handoff as required input. |
| 4 | Claims-for-facts (a step asserts an outcome without its verification) | Every asserting step carries its verification leg in the same step definition: publish carries index-confirm; land carries battery-on-landed-tree; close carries the receipt as a required input field. A claim-only step shape does not exist in the vocabulary. |
| 5 (r) | Contended-red / environmental red treated as work red | `retry_on_contention` is a declared arm with a detector and bounded rounds, recorded in history — the classification is policy in the step, not judgment in a chat log. |
| 6 (r) | Release drift (tag/tree/index disagreeing — #76 both directions) | Tag step's input type = the full per-crate checksum set; publish steps ordered by data dependency from the full graph. Tag-without-publish and publish-out-of-order are unwritable. |
| 7 (r) | Human-relay chain gaps (stages waiting on a person to carry a result) | `issue_lifecycle` stage boundaries are child-result → child-input edges. The vocabulary has no "await operator relay" step; operator steering is signal/cancel, never data transport. |
| 8 | Habit beats standing orders (dead practice tooling still on disk) | Strike-list column below: each workflow's landing includes the removal of the artifacts it replaces, in the same landing commit/motion. Plus seat-layer supreme-orders file (Waffles, in draft) — acknowledged as the layer no step verb can cure. |

## 4. Strike list (acceptance criterion 4) — same motion as each landing

| Workflow lands | Leaves the disk in that landing |
|----------------|--------------------------------|
| `gate` | `.fleet-gate.sh` on every branch and box clone; box-side gate wrapper scripts; the exit-file manifest idiom |
| `land` | docs `tracking/battery-queue-*.md` (the queue habit itself); local battery scripts in worktrees (`run-battery.sh`) |
| `investigate` + `issue_lifecycle` | scratchpad dispatch templates (`investigate-*-input.json` as hand-built artifacts); hand-armed monitor scripts (zsh while-loops) |
| `release_ceremony` | `publish-0.18.0.sh`-class scripts; hand-run index-confirm loops |
| server → Linux box move (after workflows land) | Mac respawner recipe (replaced by Aion-native supervision on the box — honest note: the respawner guards the server itself and cannot be a workflow OF that server; it retires only when the server leaves the Mac, and its two defects found tonight are fixed in the interim) |
| already flagged | `~/.local/bin/python3` (dead Linux ELF shadowing the interpreter on the Mac) — class-8 exhibit, strike with the respawner retirement |

## 5. Composition shape

- All six deploy as one content-hashed package set; `issue_lifecycle` invokes the others as
  child workflows by type — supervision, retries, and history give the audit trail per stage.
- Single-purpose workflows stay independently startable (operator can run a bare `gate` or
  `release_ceremony`) — same definitions, no forked variants.
- Reference-trace obligation (criterion 6): §1's chain replays the #75 arc step for step —
  mint (tracker-first) → investigate (Handoff) → fix_flight (the adoption fix) → land
  (battery, receipt-close) → release_ceremony (cut, topo publish, index confirm, tag,
  from-index install, exhibit replay). Tonight's receipts are the acceptance fixtures; any
  step the AWL cannot express is a vocabulary gap to close before authoring ends.

## 6. Open questions for review

1. The canonical eight-class inventory verbatim (see §3 note).
2. Park taxonomy: proposed named reasons — `NeedsInfo(owner)`, `BlockedOn(issue)`,
   `CrossRepo(repo)`, `OperatorWord` — closed set or extensible?
3. Release-set membership for `issue_lifecycle` → `release_ceremony`: who declares a
   "coherent set" — the operator by word (today's law) or a declared grouping input?
4. Judge/review legs: tonight's cut-ceremony law (small understood lanes = one honest
   review + my read) — encode the calibration as workflow input, or keep review scaling
   as operator word at dispatch?
