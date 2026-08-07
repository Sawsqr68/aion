# SEAT RESUME — marker BE (2026-08-06)

Supersedes **BD** for state. BB (review-5 findings), BC (remedy round 1) and BD
(review-6 findings) remain the record of what was found and why; this marker is
what is TRUE now.

## What changed since BD

BD left the tree **not compiling**: two reopen fixtures called a
`append_through_a_separate_recorder` helper that had never been written. BD
recommended preferring two concrete non-closure helpers over the banked closure
signature. That recommendation was taken.

### 1. Blocker B2 — CLOSED

`crates/aion/src/lifecycle/completion.rs`, test module. Three helpers now exist:

- `separate_recorder(&active) -> Recorder` — builds a recorder at the CURRENT
  history head, the way every non-monitor writer in the engine builds one. Its
  doc comment states the mechanism: a `Recorder` never re-reads the store head
  after construction, so writing fixture setup through the monitor's own
  recorder leaves that recorder current and the monitor's later append succeeds
  for a reason production never offers it.
- `terminal_by_another_writer(&active, payload)` — drives the run terminal from
  a writer that is not this monitor.
- `reopen_by_its_own_recorder(&active, &run_id)` — appends `WorkflowReopened`
  the way `reopen::reopen` does, carrying the run's visibility seam.

Both reopen fixtures now use them. `a_reopened_run_that_finishes_again_records_
its_terminal` deliberately keeps its setup on the handle's own recorder, because
in production the reopen's recorder IS the successor lease's recorder — same
machine, correctly modelled.

### 2. Blocker B1 — CLOSED (was already applied in BD)

The orphaned doc comment on `terminal_outcome_from_history` is rewritten to
describe that function.

### 3. The guard comment — REWRITTEN (`completion.rs`, the `progress` disjunct)

The false claim ("would append a SECOND one") is gone. It now says what is true:
the reopen wrote through a recorder of its own, so the monitor's recorder is
stale by one and the append is **REFUSED on sequence, not silently doubled**.
What the disjunct buys is not data integrity — the store already has that — it
is the operator not being handed a `SequenceConflict` incident for an ordinary
reopen.

### 4. Mo1's residual — NAMED IN THE CODE

The mirror case (a `NotRecorded` monitor whose run was driven terminal by
another writer and then reopened, landing inside the registry-empty interval) is
uncovered and reachable. The comment says so, names the third disjunct that
would close it, and states why it is deliberately not written: it becomes the
refuted history predicate the moment any path removes a live run's entry, and
would then strand a legitimate terminal in `Running` forever with nothing
logged. **A logged, fail-safe false alarm beats a possible silent permanent
zombie.**

### 5. M1 — CLOSED

`WorkflowNotFound` → `not_found` and `InvalidState` → `invalid_state` rows added
to `admission_and_run_state_cases`. They were the two oldest named arms and the
only two with no row: either could have been deleted and every assertion passed.

### 6. M2 — CLOSED (the claim was false, and is corrected)

The doc claim "No client operation can produce that" about the bare
`EngineError::Store(..)` shape is **wrong**. `EngineError` carries
`Store(#[from] StoreError)`, so every `?` on a store call inside an engine API
returning `EngineError` produces it. Three of the eight engine operations this
transport calls do exactly that, all verified at source:

| operation | site |
|---|---|
| `list_workflows` | `engine/api_workflow_ops.rs:493` and `:500` |
| `cancel` | `lifecycle/terminate.rs:131` |
| `signal` | `engine/delegated.rs:266` |

All three are store READS, which is why no `Recorder` is involved and no
`Durability` wrapper appears. The comment now says this.

### 7. M3 — CLOSED, AND WIDER THAN THE REVIEW FOUND

The review named two call sites reading the store directly. There are **three**:

- `resolve_run_id` (`embedded.rs:57`, `read_run_chain`)
- `describe_workflow` (`:238`, `read_history`)
- `subscribe`'s resuming half (`:279`, `read_history`)

All three now route through `store_error_class`. `store_error_class`'s signature
changed from `(&StoreError, &EngineError)` to `(&StoreError, String)` because
these sites have no `EngineError` to render.

### 8. NEW INSTRUMENT — `aion_store::testing::RefusingReadStore`

M3 had no coverage: a mutation reverting any of the three sites turned nothing
red. `InMemoryStore` owns every shard and can never produce `NotOwner`, so the
flattening was invisible to every existing test.

`crates/aion-store/src/testing/refusing_reads.rs` is a store double whose three
history reads refuse with `StoreError::NotOwner` **once armed**. Armed rather
than always-on because an engine reads its store while being built and while a
fixture seeds history — and because it gives the test a real before/after
control. Follows the house pattern of `StaleActiveListStore` /`ShardSeamSpy`,
gated behind the existing `test-support` feature so it never ships in a release
build. `aion-client` picks it up as a dev-dependency with that feature.

New test: `a_directly_read_store_refusal_keeps_its_routing_class` — one unarmed
control pass, then all three call sites asserted to answer `not_owner`.

## MUTATION BATTERY — ALL 12 RED, ZERO SURVIVORS

Run on the FINAL tree (the BD-era results for A and B are void — the fixtures
changed). Driver:
`scratchpad/mutate.py`. Each mutation: apply → run gate → restore → verify
restore by SHA-1. **Every restore verified OK.**

| # | mutation | verdict | test killed |
|---|---|---|---|
| A | drop the lease disjunct | RED | `a_reopened_run_is_not_appended_into_by_a_monitor_that_never_recorded` |
| B | drop `progress == Recorded` | RED | `a_recorded_monitor_stands_down_inside_the_reopen_window` |
| 1 | drop the `Durability(Store)` half | RED | `every_named_engine_error_reaches_its_own_class` |
| 2 | drop the `Query` arm | RED | same |
| 3 | drop `NoQueueDeclaration` | RED | same |
| 4 | `ShuttingDown` → `unavailable` | RED | same |
| 5 | `store_error_class` `NotOwner` → `server` | RED | same **+** `a_directly_read_store_refusal_keeps_its_routing_class` |
| 6 | drop the `WorkflowNotFound` arm | RED | `every_named_engine_error_reaches_its_own_class` |
| 7 | drop the `InvalidState` arm | RED | same |
| 8 | `describe_workflow` → `ClientError::server` | RED | `a_directly_read_store_refusal_keeps_its_routing_class` |
| 9 | `subscribe` → `ClientError::server` | RED | same |
| 10 | `resolve_run_id` → `ClientError::server` | RED | same |

A and B kill **different** tests — that is the pairwise separation review 5
demanded and review 6 voided. Mutation 5 kills two, correctly: it is the shared
rule both the table and the call-site proof depend on.

## GATES MEASURED AT MY OWN HAND

- `cargo fmt --all` (write) — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — **EXIT=0**, read bare
  from `$pipestatus[1]`.
- `cargo test -p aion-client --lib` — **63 passed / 0 failed**.
- `cargo test -p aion-store --lib testing::refusing` — 1/0.
- `cargo test -p aion-rs --lib lifecycle::completion` — 24/0.
- `cargo test -p aion-rs --lib lifecycle::` — 97/0 (seen repeatedly as the
  mutation baseline).

## STILL TO DO, IN ORDER

1. `clippy -p <crate>` variants — `--workspace` resolves `preserve_order` ON,
   `-p` off; different runs, BOTH required.
2. `cargo test -p aion-server --lib` and `-p aion-store --lib` in full.
3. Full `aion-rs` battery, **quiet**. The BD-era run had one failure —
   `outbox_happy_path_completes_with_n_terminals_in_order`, `"SQLite failure:
   bad parameter or other API misuse"` = the **#85** libsql 0.9.30 double-close
   signature — and it ran CONTENDED with a reviewer's builds. Re-run quiet
   before attributing anything.
4. **Review 7 on Opus.** Give it BB, BC, BD, BE and review 6's findings. Tell it
   the unreviewed layer still stands: `engine/api.rs`, `engine/builder.rs`, the
   `nif_child_tasks.rs → engine_tasks.rs` rename, `api_workflow_ops.rs`,
   `builder_assembly.rs`, `completion_retry.rs`, `store_faults.rs`.
5. Commit naming files EXPLICITLY (never `git add -A` in this tree), then push.
6. Route **Mo3** to Waffles as a ruling: the unbounded 1 ms / 8 ms
   `CompletionRetryConfig` default is unreachable from `aion server`.
7. Tell Tom plainly that review 6 failed, why, and that it is now fixed.
8. Then #5 (spec banked in `SPEC-5-AWL-SUM-COMBINATOR.md`), #1, #6, #9.
9. **Owed to Tom:** #146 tutorial dry run + one-page crib sheet, without
   touching his pids.

## STANDING CONSTRAINTS (unchanged, carried)

- NO WORKTREES in aion. One mutating lane. Unmarked dirty = STOP.
- ⛔ Never bare `cargo clean`. No custom `CARGO_TARGET_DIR`. No launchd.
- ⛔ **NEVER `git add -A` in this tree** — it swallows the NO-SWEEP untracked
  snapshot dirs and hangs. Name paths explicitly.
- ⛔ `git stash push` must NEVER use `-u`. No `git gc`/`prune`. DELETE NOTHING.
- ⛔ No `--force`/`--mirror`/`--prune`/`--delete` on any push.
- NO-SWEEP holds: `.aion-accepted-binary-20260731/` (SOLE copy of Tom's accepted
  build), `.aion-jjuabk-snapshot-20260731/`, `.aion-data-preswap-20260804/`,
  `../aion-pre-scrub-backup-20260804.git`.
- 🔴 Tom's live pids: server 45425, worker 48399. DO NOT TOUCH.
- 🔴 WORKER REBUILDS FROZEN estate-wide until the #139 live cut.
- 🔴 beamr JIT blocker (Artemis): keep long workflows off until fixed beamr lands.
- Do not commit while a battery is in flight (#123 build-identity pin goes red).
- Commit messages with backticks go through `git commit -F -` + quoted heredoc.
- zsh: no `PIPESTATUS`; `$pipestatus` is 1-indexed; `status` is read-only — name
  it `rc`.
- CLAUDE.md mandates an Opus sub-agent review before work lands. Do not skip it.
