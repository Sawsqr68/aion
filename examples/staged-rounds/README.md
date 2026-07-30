# staged-rounds: a planned, phased, gated fan-out as one AWL workflow

Supplied material goes in; merged, gated, panel-reviewed work comes out. One
planner agent reads the material and returns a typed, phased plan of
scope-fenced work items. Bounded rounds then fan the released items out — one
worktree and one dev agent per item, IN PARALLEL — run the mechanical gate
battery per item AUTHORITATIVELY, and fan a REVIEW PANEL over every
(item × lens) pair. Accepted items fold into `done` and release their
dependents, the rest cycle with the gate output and the reviewers' feedback
attached, accepted branches merge, merge conflicts feed a bounded remediation
cycle whose agent RESUMES the planner's session — and the merged branch is then
VERIFIED by re-running the battery on the integration worktree.

One workflow, **activities only, no children** — the whole shape is a single
AWL document on the direct compile path (`aion deploy <file.awl>`), so it
appears in the authoring surface. Every agent runs on Norn in DRIVEN mode
(jsonrpc) with a schema-constrained output — live transcript + inject/cancel in
the ops console, and the operator's ChatGPT OAuth login (no API key is ever
read or passed).

## Three things this pipeline will not do

These are the holes that were closed on 2026-07-30, stated here because the
absence of each was invisible from the outside:

1. **The gate battery is not advice.** `config.gates` is executed by
   `gate_item` in the item's own worktree and every exit code is recorded as
   data. An item cannot reach `done` on a red battery no matter what its dev
   agent claimed or its reviewers concluded. Previously `config.gates` reached
   exactly one place — string interpolation into the dev agent's prompt — so
   acceptance rested on an agent's self-report, and the doctrine file told the
   agent it was being checked when nothing checked it.
2. **One reviewer is not a panel.** `config.lenses` are chartered adversaries
   run concurrently, each blind to the others' verdicts, and accept/reject is
   DERIVED from findings (any blocking finding rejects) rather than trusted from
   the reviewer's asserted `overall`. A disagreement between the two is itself
   a recorded violation and rejects.
3. **A clean merge is not a green project.** `verify_integration` re-runs the
   battery on the merged worktree, because N individually-green branches can
   merge cleanly and still produce a red project: two items touching the same
   callers, a test that only fails in combination, a lint that only fires on
   the union. Previously a clean merge routed straight to `completed` with zero
   mechanical evidence that the merged tree even compiled.

## Composition

```
staged_rounds                       (awl/staged_rounds.awl, direct-compiled)
│
├── plan
│     └── planner        (AGENT, read-only, rooted at the repo;
│                         session {workflow_id}-planner; returns the typed
│                         phased plan; fold_phase seeds ready/blocked)
│
├── execute — the bounded round loop (cap = config.max_rounds):
│     ├── fork item in state.ready → provision_item     (SHELL, parallel)
│     │     one git worktree per item under
│     │     <repo>/.staged-rounds/<workflow id>/items/<item id>,
│     │     branch staged/<workflow id>/<item id>; a dependent's worktree
│     │     gets its depends_on items' accepted branches MERGED IN (it
│     │     really starts from their work); REUSED (never reset) across
│     │     rounds so a rejected item's committed work survives
│     ├── fork p in provisioned → dev_item              (AGENT, parallel)
│     │     session {workflow_id}-dev-<item id>, resumed across that
│     │     item's feedback rounds; the WORKER commits the round's work —
│     │     agents run no git
│     ├── fork d in dev_results → gate_item             (SHELL, parallel)
│     │     the battery, run for real: {base_commit} substituted into argv,
│     │     every exit code recorded, a write-mode formatter's mutations
│     │     committed mechanically and THE FULL BATTERY RE-RUN, the diff
│     │     the panel reads produced with --no-ext-diff
│     ├── plan_reviews                                  (SHELL, pure)
│     │     flattens gated items × lenses into one panel list
│     ├── fork task in panel → review_item              (AGENT, parallel)
│     │     session {workflow_id}-review-<item id>-<lens>; read-only at the
│     │     item worktree (write tools denied); sees the gate outcome
│     └── fold_phase                                    (SHELL, pure)
│           red gate ⇒ ineligible; every configured lens must have reported;
│           derive-and-check each verdict; accepted → done + dependents
│           released; rejected → stays ready with the gate failures AND the
│           rejections attached; evidence accumulates
│
├── merge_once            (SHELL: ordered --no-ff merges of every done
│                          branch into staged/<workflow id>/integration)
│
├── fix_conflicts — the bounded merge loop (cap = config.max_merge_rounds):
│     ├── remediate       (AGENT — RESUMES {workflow_id}-planner, rooted at
│     │                    the integration worktree with the conflicted
│     │                    merge IN PROGRESS; resolves the markers; the
│     │                    WORKER concludes the merge — agents run no git —
│     │                    and REFUSES to conclude while any conflict
│     │                    marker remains in a conflicted file)
│     └── merge_branches  (SHELL, idempotent continue-style re-merge)
│
└── verify_clean / verify_fixed — the bounded integration loop
      (cap = config.max_integration_rounds):
      ├── verify_integration (SHELL: the battery on the MERGED tree; clean
      │                       asserted before AND after; deliberately no
      │                       normalization commit — a gate that dirties the
      │                       integration tree is itself a failure.
      │                       {base_commit} and the recorded diff both resolve
      │                       to merge-base(base_branch, HEAD) — where the run
      │                       started — so a scope-fence gate fences the UNION
      │                       of the run's work and Completed.verification.diff
      │                       is the whole project change. An unresolvable
      │                       merge-base is terminal, never a HEAD fallback)
      └── fix_integration    (AGENT — also resumes the planner's session, so
                              the coordinator answers for its own
                              decomposition, with the failing battery attached)
```

The verification appears TWICE — once after a clean merge, once after a
repaired one — and the document says why on its face: an AWL binding exists
only on the path that bound it, so one shared step could name neither merge
result.

Terminal dispositions are honest: `completed` (success, carrying the
integration battery as evidence), `exhausted` (rounds ran out with items still
pending/blocked), `conflicted` (merge remediation ran out), `unverified` (the
merge succeeded and the project still is not green — every branch passed alone,
the union did not). Nothing pushes anywhere: the handoff is the integration
branch plus the run's accumulated evidence in durable history. The operator
merges.

## The node table

The server routes by (namespace × task_queue × node) only — never by activity
type — so the worker opens one connection PER NODE on the one task queue
`staged_rounds`. `awl/staged_rounds.awl` is the authoritative node table:

| activity             | node          | connection                              |
|----------------------|---------------|-----------------------------------------|
| `planner`            | `planner`     | driven agent (read-only, plan schema)   |
| `provision_item`     | `shell`       | typed registry handler                  |
| `dev_item`           | `developer`   | driven agent (`--max-parallel` wide)    |
| `gate_item`          | `shell`       | typed registry handler + progress        |
| `plan_reviews`       | `shell`       | typed registry handler (pure)           |
| `review_item`        | `reviewer`    | driven agent (read-only, `--max-parallel`) |
| `fold_phase`         | `shell`       | typed registry handler (pure)           |
| `merge_branches`     | `shell`       | typed registry handler                  |
| `remediate`          | `remediation` | driven agent (resumes the planner)      |
| `verify_integration` | `shell`       | typed registry handler + progress        |
| `fix_integration`    | `remediation` | driven agent (resumes the planner)      |

`remediate` and `fix_integration` share the node `remediation` — so they share
ONE connection, and the worker refuses at startup, by name, if two roles on one
node disagree on anything the connection owns (output schema, profile,
disallowed tools, concurrency). Two connections on one node would let the
server land an activity on a connection that has no handler for it.

PARALLELISM: the engine fans items out via `workflow.map` (fail-fast); the
genuine N-wide execution knob is the worker's `max_concurrency` on the
developer and reviewer connections (`--max-parallel`, default 4).

## Check, deploy, run (direct path — no Gleam project)

```sh
aion awl check examples/staged-rounds/awl/staged_rounds.awl
aion deploy    examples/staged-rounds/awl/staged_rounds.awl

cd examples/staged-rounds/worker
cargo test                       # hermetic worker tests (real git) + the
                                 # direct-path compile proof
cargo build --release
./target/release/staged-rounds-worker \
  --profiles-dir ./profiles \
  --identity staged-rounds \
  --max-parallel 4 \
  --ready-file /tmp/staged-rounds-worker.ready
```

`--profiles-dir` is REQUIRED and must contain all four of `planner.md`,
`developer.md`, `reviewer.md` and `remediator.md`; a missing or empty file is a
loud startup failure naming the file. `norn` must be on PATH and logged in
through ChatGPT OAuth — `OPENAI_API_KEY` is stripped from the child
environment, so an API-key-only setup will fail.

Start the worker manually (no launchd). Then start a run from the ops console
start form (workflow type `staged_rounds`), or:

```sh
aion start staged_rounds \
  --input-file examples/staged-rounds/awl/example-input.json
# or, one motion:
aion run examples/staged-rounds/awl/staged_rounds.awl \
  --input "$(cat examples/staged-rounds/awl/example-input.json)"
```

There is no task-queue flag: the run's task queue is derived from the deployed
document (its first `worker` name), never passed at start. `--input` is always
inline JSON; a file rides `--input-file` (start only).

## Writing the input

`awl/example-input.json` is the template. `awl/tharsis-input.json` is a real
whole-project run against `/Users/tom/Developer/ablative/dev-ops/tharsis` and
is the better worked example: it shows a brief that makes the planner read the
target repo's OWN plan rather than invent one, and that hard-fences the work
which belongs to other repositories.

The parts that decide run quality:

- **material.brief** — the planner's whole world; write it like you're
  briefing a capable lead who has never seen the repo. State the dependency
  ordering and the hard scope fences explicitly; the planner cannot infer that
  a brief lands in a different repository.
- **material.pointers** — files/docs to read first.
- **config.gates** — the mechanical battery, run FOR REAL in each item's
  worktree and again on the merged tree. Write-mode formatters are fine as
  gates: the mutations they leave are committed mechanically and the battery
  re-runs. An empty battery is honoured as an explicitly diagnosed vacuous
  pass, never a silent one.
- **config.lenses** — the review panel. Each needs a `name` (it keys the
  verdict and names the reviewer's session) and a `charter` (its marching
  orders). Every configured lens MUST report for an item to be accepted, so
  an empty list is refused: it could never accept anything.
- **config.max_rounds / max_merge_rounds / max_integration_rounds** — the
  bounded-cycle caps; cap exhaustion is a terminal disposition, never a silent
  success.

Work item ids are git-ref-safe slugs (`[a-z0-9-]`, no leading/trailing `-`):
they name branches. A terminal per-item activity failure fails the run — the
parallel fan-out is engine-owned fail-fast.

## Known limits

- **`aion describe` dies on histories over 4 MiB** ("decoded message length too
  large"). A whole-project run will exceed this. Watch terminal state with
  `aion list` instead, and read per-item detail from the run's evidence.
- **Evidence is clipped** (16 KiB, head + tail around an explicit TRUNCATED
  marker), so the durable record of a pathological gate tail is lossy.
- **Shell activities report per-gate-command progress** (each command's start
  and result) but nothing finer: a single 10-minute `cargo test` is one
  progress beat, not a stream.
