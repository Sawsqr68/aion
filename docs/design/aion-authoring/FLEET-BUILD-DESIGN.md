# Fleet build: staged_rounds made whole

Tom's ask, 2026-07-30: a workflow he can run on a separate device as a fleet
stress test, that takes a plan for an ENTIRE project and works it end to end
without interruption — planning docs, rescheduling, gates, reviews,
verifications, full streaming — and that he can SEE in the authoring surface.
Target project: `/Users/tom/Developer/ablative/dev-ops/tharsis`.

## Why this extends `staged_rounds` rather than forking it

`examples/staged-rounds/awl/staged_rounds.awl` (190 lines) is already the
project-scale shape: a planner agent returns a typed phased plan of
scope-fenced items with `phase` and `depends_on`; bounded rounds fan the
released items out one worktree + dev agent + reviewer per item IN PARALLEL;
`fold_phase` moves accepted items to `done`, keeps rejected items pending with
the reviewer's feedback attached, and releases newly-unblocked next-phase
items — that is the rescheduling Tom asked for, already built. Accepted
branches merge; conflicts feed a bounded remediation cycle whose agent resumes
the planner's session. Three honest terminal dispositions: `completed`,
`exhausted`, `conflicted`.

It is a single AWL document on the direct compile path, so
`aion deploy examples/staged-rounds/awl/staged_rounds.awl` publishes it and it
appears in the authoring surface. That satisfies Tom's second message with no
work at all.

House rule (CLAUDE.md): NO BACKWARDS COMPATIBILITY during this build — replace,
don't add alongside. A second near-identical document and a second near-identical
worker crate would be exactly the duplication that rule forbids, and the
integration-verification gap below is a DEFECT in the existing pipeline, not a
fleet-only nicety. So the existing artifact gets fixed.

## The three gaps

### G1 — the gate battery is advice, not a gate

`config.gates: [GateCommand]` is passed into `dev_item(work, gates)`, i.e. into
the AGENT's prompt. Nothing in the pipeline runs the battery authoritatively and
records exit codes as data before an item is accepted. An item is accepted on
the strength of a reviewer agent's opinion about an agent's claim.

dev_brief solved this years of lessons ago: `run_gates` is a SHELL activity, the
exit status IS recorded data, `{base_commit}` is substituted into argv, and a
gate that mutates the tree gets a mechanical normalization commit and a full
re-run. See `examples/dev-brief/worker/src/handlers/gates.rs`.

**Fix:** new shell activity `gate_item(work: DevItemResult, gates: [GateCommand])
-> GateOutcome`, run per item after `dev_item` and before review. `fold_phase`
takes the gate outcomes; a red gate makes the item ineligible for `done`
regardless of any verdict, and the failure text lands in the item's `feedback`
for the next round's dev turn.

### G2 — one reviewer per item, not a panel

`review_item(work: DevItemResult) -> ItemVerdict` is a single generalist
reviewer. dev_brief's measured lesson is a PANEL of chartered lenses run
concurrently — correctness, regressions, brief_compliance (production adds
spec_fidelity) — each blind to the others' verdicts, with accept/reject DERIVED
from findings (any blocking finding ⇒ reject) rather than asserted, and
asserted-vs-derived mismatch recorded as its own violation.

**Fix:** a `Lens { name, charter }` type and `config.lenses: [Lens]`; a pure
shell activity `plan_reviews(dev: [DevItemResult], lenses: [Lens]) ->
[ReviewTask]` flattens item × lens into one list so the existing single-level
`fork` covers the whole panel with no new vocabulary; `review_item` takes a
`ReviewTask` carrying its lens; `fold_phase` groups verdicts by `item_id` and
requires EVERY configured lens to have reported and accepted. An empty verdict
set never accepts (dev_brief's `all_accept([]) == False`).

Rationale for flattening rather than nesting `fork`: it needs no unproven
nesting semantics and it gives the console one flat set of concurrent
activities, which is also better for watching a fleet run.

### G3 — nothing verifies the merged integration branch

`merge_once` does ordered `--no-ff` merges of every done branch into
`staged/<workflow id>/integration`, and on a clean merge routes straight to
`completed`. N individually-green branches can merge cleanly and produce a red
project — different items touching the same callers, a test that only fails in
combination, a lint that only fires on the union. Nothing catches that.

This is the gap that matters most for a WHOLE-PROJECT run, because it is the
only one whose blast radius is the entire deliverable.

**Fix:** new shell activity `verify_integration(run_root, integration_branch,
gates) -> GateOutcome` run on the merged worktree, plus a bounded fix cycle
(`config.max_integration_rounds`) whose agent works on the integration worktree
with the failing battery attached. A new terminal disposition
`integration_failed` when that cycle exhausts — never a silent success.

## Streaming

Already there and needs nothing: every agent runs on Norn in DRIVEN mode over
jsonrpc, `aion-integration-norn`'s translate layer maps `event/message`,
`event/toolCall`, `event/toolResult`, `event/progress` (text/thinking deltas)
and `event/stop` onto neutral `ActivityEvent`s, anything unclassifiable falls
through to `Raw` so nothing is dropped, and the worker pumps them onto the
liminal observability channel. Live transcript plus inject/cancel in the ops
console.

The honest limit to state on its face: shell activities emit NO incremental
progress. A long `cargo test` is a detail-free liveness beat until it returns
(`ActivityContext::heartbeat` exists and takes an optional payload; the shell
handlers do not call it). Worth fixing for a fleet run where gates dominate
wall-clock — tracked separately, not silently absorbed here.

## Known limit inherited, must be stated

`aion describe` dies on histories over 4 MiB ("decoded message length too
large") — runbook task #249. A whole-project fleet run WILL exceed this. The
watcher must be terminal-state polling via `aion list`, not `describe`. This is
a real operator-facing limit for exactly this workload.

## tharsis as the first run

tharsis already contains the plan, so the planner reads it rather than inventing
it: `docs/ROADMAP.md` plus 19 briefs under `docs/briefs/`, four job kinds
(D-ecision, E-ngine, S-pike, B-uild), and four go/no-go gates G1–G4.

Constraints the material must state, because they are not derivable from the
code:

- Track 0 is fully parallel; Track 1 (`S1 → S2 → S3 → S7`) is STRICTLY ordered
  and each step invalidates everything after it if it fails. This maps onto
  `phase` + `depends_on` directly.
- E1–E4 land in the haematite repo, not tharsis — out of scope for a
  single-repo run; the planner must not emit them.
- S7 is blocked on haematite #157.
- B5 is half in aion.
- 30 of 35 `pub fn`s are `todo!("<brief id>")`; module docs say do not fill
  them ahead of the spikes that decide their shape — so B-items must be phased
  behind their spikes.
- Missing infrastructure that is itself work: no CI, no toolchain/lint pinning,
  no `docs/decisions/`, no `spikes/s1..s8/` with the `run.sh` harnesses
  `spikes/README.md` requires.

Battery for tharsis (stock cargo — the repo codifies none):

```json
[
  {"name": "fmt",    "argv": ["cargo", "fmt", "--all"]},
  {"name": "build",  "argv": ["cargo", "build", "--workspace"]},
  {"name": "clippy", "argv": ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]},
  {"name": "test",   "argv": ["cargo", "test", "--workspace"]}
]
```

`fmt` is a write-mode formatter used as a gate deliberately: tree mutations it
leaves behind are committed mechanically so the branch stays complete (per
Tom's standing rule — never format-CHECK, always autoformat).

## Build state, 2026-07-30

DONE and verified on the real release binary:

- `examples/staged-rounds/awl/staged_rounds.awl` extended with all three fixes.
  `aion awl check` → `ok (12 steps)`. `aion awl emit --target beam` → emitted,
  exit 0, so it stays inside the direct-deploy path's refusal set and will
  publish with `aion deploy <file>.awl` and appear in the authoring surface.
  Canonically formatted with `aion awl fmt`.
- Four new outcomes/types: `Lens`, `GateRun`, `GateOutcome`, `GatedItem`,
  `ReviewTask`, and the `unverified` terminal disposition.
- `examples/staged-rounds/awl/tharsis-input.json` — the whole-project run.
  Validated against the compiled input schema with `jsonschema`.
- `examples/staged-rounds/awl/example-input.json` — updated for the new
  required config fields and re-validated (it was left invalid by the type
  change; a broken template is not acceptable).

Baseline before the change, for the record: the unmodified document also
checked (8 steps) and lowered, so the extension did not rescue a broken
artifact — it extended a working one and kept it working.

Verification steps 6 and 7 in the AWL are DUPLICATED (`verify_clean` /
`verify_fixed`) and this is deliberate, not sloppiness. An AWL binding exists
only on the path that bound it: `attempt` is bound in `merge_once`, `mstate`
only inside `fix_conflicts`, so one shared verification step could name
neither merge result. The alternative — deriving the integration branch name
in `plan` so no merge binding is needed — fails at the finish step, which
still needs each path's `merged` list. Ten duplicated lines with the reason
stated on the document's face beats a wrong abstraction.

STILL TO DO — the worker. Six action changes in
`examples/staged-rounds/worker/`, which is a standalone crate with path deps
into this repo:

- NEW `gate_item(work: DevItemResult, gates: [GateCommand]) -> GatedItem`
  (node `shell`). Port `examples/dev-brief/worker/src/handlers/gates.rs`:
  `{base_commit}` argv substitution, exit status recorded as data, mechanical
  normalization commit when a write-mode gate dirties the tree followed by a
  FULL battery re-run, `git diff --no-ext-diff <base_commit>` for the panel,
  empty battery honoured as an explicitly-diagnosed vacuous pass.
- NEW `plan_reviews(gated: [GatedItem], lenses: [Lens]) -> [ReviewTask]`
  (node `shell`, pure).
- CHANGED `review_item(task: ReviewTask) -> ItemVerdict` — was
  `work: DevItemResult`. The lens charter travels in the prompt; one reviewer
  connection serves every lens.
- CHANGED `fold_phase(prior, incoming, gated, verdicts, lenses) -> PhaseState`
  — was `dev: [DevItemResult]` and had no `lenses`. Must now: refuse to accept
  an item on a red gate; group verdicts by `item_id`; require EVERY configured
  lens to have reported for an item (a missing lens is a violation, and an
  empty verdict set never accepts); derive accept/reject from findings and
  record asserted-vs-derived disagreement as its own violation; write both the
  gate failures and the rejecting verdicts into the item's `feedback`.
- NEW `verify_integration(run_root, repo_root, integration_branch, gates)
  -> GateOutcome` (node `shell`). No normalization commit here — a gate that
  dirties the integration tree is itself a failure.
- NEW `fix_integration(verify, plan, integration_branch, workspace_path)
  -> RemediationReport` (node `remediation`, resumes the planner session).

`ItemVerdict` also gained a `lens: String` field, so its output schema and the
reviewer's `--output-schema` document both change.

## Blast radius

Agents WRITE CODE in this pipeline, unlike `dev_flow` which forbade it. Every
item gets its own git worktree under
`<repo>/.staged-rounds/<workflow id>/items/<item id>` on branch
`staged/<workflow id>/<item id>`, the worker performs every git operation
(agents run no git), and nothing is ever pushed — the handoff is the integration
branch. The operator merges. Tom's working copy of tharsis is not the
workspace.
