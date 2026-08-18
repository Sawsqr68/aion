# Codification design surface — the development process as AWL workflows

Status: APPROVED TO AUTHOR (Waffles review 22:20Z, four deltas + rulings incorporated below).
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

### 1.7 `propagate` (delta a — Tom has asked for it by name: "the beamr work still hasn't propagated")
Input: released version + the service set it must reach. Steps: per service — resolve the
running instance → stage the released artifact → restart under that service's own
supervision discipline → VERIFY THE VERSION BANNER AT THE RUNNING SERVICE (landed ≠ running;
the claim "propagated" carries its verification in the same step, class 4). Honest carve-out:
covers every service EXCEPT the aion server's own binary, which stays with native supervision
until the server moves off the Mac (§4 respawner note).

### Salvage is an input mode (delta b)
`fix_flight` and `gate` accept `resume_from: KeptTree { path, provenance: salvage }` —
tonight's proven pattern (three flights salvaged by hand) becomes expressible: the run
resumes from the kept tree with provenance recorded in history, so a Failed status column
measures instruments, never work. An input mode, not a tenth workflow.

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

RECONCILED to the canonical inventory (Waffles → Tom 22:06Z, quoted verbatim below).
Classes 1–8 are the canon; 9–10 are accepted additions from this design's first draft.
Classes 6 and 7 carry NAMED RESIDUE — no vocabulary cure, resolution named, never dropped.

| # | Canonical class (verbatim) | Where it becomes inexpressible |
|---|---------------------------|-------------------------------|
| 1 | "Gates and landing batteries ran as local scripts on your Mac when workflows existed for exactly that job — the box wasn't free and the habit outlived your order." | Whole-cloth rule: no step type takes a script path; gate spec argv is inline; execution routes server → fleet workers only. The vocabulary has no "run this file" verb and no Mac-local execution arm. |
| 2 | "Releases claimed off the tag with no index check — 0.16 and 0.17 tagged but never published, two versions of silent drift." | `release_ceremony`: the tag step's input type = the full per-crate checksum set from index confirmation; publish steps ordered by data dependency over the full graph. Tag-without-publish and publish-out-of-order are unwritable. (#76 + addendum.) |
| 3 | "Investigations that terminate as verdict files instead of chaining through fix, land, close, release — 27 verdicts, zero landings, main unmoved all night." | `investigate`'s result union has no bare-verdict arm: `Handoff` or `Park{reason, recheck}` only. `issue_lifecycle` consumes the handoff as the next stage's required input. |
| 4 | "Claims standing in for facts everywhere: executor 'tests pass' reports that didn't match the final tree, 'reviewed' treated as 'landed', 'landed' treated as 'running', publishes assumed rather than verified." | Every asserting step carries its verification leg in the same step definition: publish carries index-confirm; land carries battery-on-the-landed-tree; close requires the receipt as an input field; `propagate` verifies the version banner AT the running service ("landed ≠ running", cured by name). A claim-only step shape does not exist. |
| 5 | "Instrument defects that ate results silently: exit codes swallowed by pipelines, watchers that miss marks wrapping across lines, a version-mismatched patch line that cargo dropped without a word." | Activity result = recorded rc + full output artifact in engine history; a step with no recorded result cannot advance. Watch/monitor idioms are replaced by workflow await/signals (no line-matching watchers). Lock-witness legs in `gate`/`release_ceremony` make the dropped-patch class a red, not a silence. |
| 6 | "Worker-plane fragility with rows already minted: idle socket death, in-flight dispatches orphaned on worker loss, server ignoring TERM, a judge dying on an API 529 and failing work that was mechanically green." | NAMED RESIDUE — no vocabulary cure. Resolves at the engine rows #69/#72/#73/#74 plus the wave-2 `estate_health` workflow. The mapping carries this row open until those land. |
| 7 | "The dispatch engine wedging after ~15 runs until the beamr floor migration lands." | NAMED RESIDUE — no vocabulary cure. Resolves at the beamr-floor migration (fleet-70 salvage → #70 landing) plus wave-2 `fleet_provision` floor checks. Carried open until landed. |
| 8 | "And the root one: standing orders losing to old habit because the dead practice's artifacts stayed on disk teaching it." | Strike-list column (§4): each workflow's landing removes the artifacts it replaces in the same motion. Plus the seat-layer supreme-orders boot-read (Waffles, in draft) — the layer no step verb can cure, held at the seat. |
| 9 | Contended-red / environmental red treated as work red (addition, accepted 22:20Z) | `retry_on_contention` is a declared arm with a detector and bounded rounds, recorded in history — classification is step policy, not chat-log judgment. |
| 10 | Human-relay chain gaps — stages waiting on a person to carry a result (addition, accepted 22:20Z) | `issue_lifecycle` stage boundaries are child-result → child-input edges. No "await operator relay" step exists; operator steering is signal/cancel, never data transport. |

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

## 6. Rulings (Waffles 22:20Z — settled, formerly open questions)

1. Eight-class inventory: received verbatim, reconciled in §3 (ten rows: canon 1–8 +
   additions 9–10; residue named on 6 and 7).
2. Park taxonomy: CLOSED set — `NeedsInfo(owner)`, `BlockedOn(issue)`, `CrossRepo(repo)`,
   `OperatorWord` — and (delta d) every `Park{reason}` carries a RECHECK trigger (time or
   event); a parked lifecycle resurfaces on it. Park reasons expire unchecked otherwise.
   A new park reason is a design change with a doc edit, never a runtime string.
3. Release-set membership: a declared grouping INPUT authored by operator word — authority
   stays with the operator (standing order 7), the declaration becomes data the ceremony
   records, so the set is auditable in history.
4. Review calibration: a workflow INPUT with the cut-ceremony law as its default,
   operator-overridable at dispatch, the override recorded in the run — calibration lives
   in history, not chat.

## 7. Wave 2 — declared now so absence reads as sequenced, not forgotten (delta c)

- `fleet_provision`: box provisioning with version floors LOUD (refuse below floor, name
  the floor) and an arch-correct interpreter probe (the ~/.local/bin Linux-ELF-on-Mac find
  becomes a typed refusal at provision time).
- `estate_health`: alarm channels PROVEN (every health probe demonstrated against a known
  dead and a known live target before it is trusted — tonight's port_healthy "000" defect
  as the founding exhibit), typed code matching throughout.
Wave-2 authoring follows wave 1's landings; both resolve the named residue on classes 6–7
together with the engine rows.
