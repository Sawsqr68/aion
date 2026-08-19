# Every working tree in aion, and what should happen to it

**2026-08-19.** Tom asked for a round-up of all the working trees and for
everything to be got into one main branch. This is the round-up, measured
rather than remembered: 22 trees, each one compared against `origin/main`
(`cac631108`, workspace version 0.18.1) and trial-merged onto it.

## The headline

**Thirteen of the twenty-one side trees carry nothing at all.** They are zero
commits ahead of main — every line of work in them is already merged. They are
empty rooms with the lights left on.

Of the eight that do carry something, **one is already superseded**, **three are
retired build scripts whose merge would UNDO a decision main has already made**,
and **four are real work**. So "merge everything" is, in truth, four merges — and
two of those four want a gate run before they land, not just a merge.

## The full inventory

| Tree | Unmerged | Trial merge onto main | Verdict |
|---|---:|---|---|
| `investigate-workflow` (the main tree) | 115 | 1 conflict | **MERGE** |
| `247-transcript-spool` | 7 | 1 conflict | **MERGE after a gate** |
| `lane1-declared-body-cancel` | 5 | clean | **MERGE after a gate** |
| `jit-threshold-on-0181` | 1 | clean | **MERGE** |
| `28-dig` | 3 | clean | **MERGE 1 of 3 commits** |
| `libsql-retirement` | 5 | 117 conflicts | **DELETE — superseded** |
| `fleet-53` | 4 | clean | **DO NOT MERGE** |
| `fleet-66-67` | 4 | clean | **DO NOT MERGE** |
| `fleet-70` | 4 | 1 conflict | **DO NOT MERGE** |
| 13 others | 0 | — | **nothing to merge** |

The thirteen with nothing in them: `fix-75`, `12-payload-limits`,
`15-mint-credentials`, `25-liveness`, `retire-libsql`, `57-time-glue`,
`land-stack`, `r3-cache-budget`, `241-final-message`, `release-build`
(detached), `release-v0.17.0`, `resume-adoption`, `supersession-record`.

## The three traps a plain "merge everything" would have walked into

**1. A clean merge is not the same as a wanted merge.** `fleet-53` and
`fleet-66-67` merge without a single conflict — and merging them **re-adds
`.fleet-gate.sh`, a file main deliberately DELETED.** Git has no way to tell a
file that was removed on purpose from one that never arrived; a clean merge
would quietly reverse that removal. `fleet-70` is the same work and does
conflict, and its conflict is the honest one: `modify/delete`. Three branches,
one retired script, and only the third tells you so.

**2. Superseded is not the same as unmerged.** `libsql-retirement` shows five
commits missing from main, which reads as five commits of lost work. Four of
them are already upstream under different hashes, and **libsql is already gone
from main** — no crate, no workspace entry. The fifth "missing" commit is the
same subtraction landed a different way. Merging it would fight 117 files to
re-do something already done.

**3. Diagnostics are not features.** `28-dig` is three commits. Two of them add
80 lines of tracing into the runtime's activity path to chase a bug — and the
third commit records that **the bug never reproduced**. The findings belong on
main; the instrumentation does not, and it merges just as cleanly as anything
else.

## What actually lands, in order

1. **`jit-threshold-on-0181`** — one commit, clean, surfaces beamr's JIT
   compilation threshold. Smallest thing here; lands first so the rest rebase on
   a moved main.
2. **`investigate-workflow`** — 115 commits: the MCP task-durability change
   (built, gated, killed twice on a live binary), the console "run unknown" fix,
   and the investigation records. One conflicted file,
   `crates/aion-store-haematite/src/store.rs`. It IS the `parent_workflow_id`
   clash from #77 that I predicted, but not where I predicted it: I expected it
   in my `mcp_surface_e2e.rs` fixtures, which auto-merged, and it surfaced in a
   haematite store fixture instead. Right about the cause, wrong about the site,
   which is the ordinary reason a located conflict beats a guessed one.
3. **`lane1-declared-body-cancel`** — 5 commits, clean, real feature work in
   red-then-green pairs: an authored per-attempt bound reaching the server's own
   process, and a cancelled run stopping the body the server is running. Wants
   its gate re-run on the merged tree before it lands, because a gate that ran
   on its own branch measured a different tree.
4. **`247-transcript-spool`** — 7 commits, one conflict in
   `activity_publisher.rs`, spooling transcript events across outages plus a
   collision-safe replay dedup. Same gate condition.
5. **`28-dig`** — cherry-pick `ed3f1db4f` (the findings) only.

Then the eight side trees that are done, plus the thirteen empty ones, come out.

## What is merged, as of 2026-08-19 09:0xZ

Branch `integration-20260819`, cut from `origin/main` in
`.worktrees/integration-20260819`. **Not pushed.**

```
ed495aaec docs(28): record the no-reproduction dig findings   (cherry-pick)
d428bfa4c Merge branch 'lane1-declared-body-cancel'           (clean)
3fadc993e Merge branch 'investigate-workflow'                 (1 conflict, resolved)
c03ad81c7 Merge branch 'jit-threshold-on-0181'                (clean)
cac631108 origin/main
```

126 commits over main, no working-tree changes outstanding. The one conflict
was resolved by keeping `parent_workflow_id: None` — the field main added to
`Event::WorkflowStarted` is not optional, so the side that has it is the only
side that compiles.

### 247-transcript-spool is a PORT, not a merge — and it is stopped here

Its two conflicts both sit in `activity_publisher.rs`, and neither is textual.
Main rewrote the single-event `publish` into a batched
`publish_all` → `publish_all_outcomes` → `persist_group`. The spool branch adds
durable replay-idempotency (`accepted_replay`) at two points **on the old
single-event path that no longer exists** — once before seeding the
optimistic-concurrency loop, and once inside the `SequenceConflict` arm, where a
concurrent delivery might have been the same spool entry.

`accepted_replay` itself auto-merged and is present. What has no home is the
pair of call sites: in the batched shape the check has to run per event inside
`persist_group`, against that stream's cap-splitting and its `outcomes[index]`
write-back.

Resolving that by picking a side, or by dropping the call sites to make the
merge go through, would silently disable de-duplication — a re-delivered spool
entry would be appended twice, or a distinct event eaten. **This is the exact
shape of a safety layer that fails quiet**, and it does not get done inside a
four-way merge commit where a mistake leaves no trace. It is named here as its
own piece of work, with the two call sites and the reason already located.

## What I have not done, and why

**I have not merged anything into main, and I have not removed a single tree.**

Two reasons, and neither is hesitancy:

- Main merges route through Waffles the Terrible by standing estate law, and
  main is the branch the release line cuts from. It is at 0.18.1; my tree is at
  0.17.0. That version gap is exactly the trap that made me refuse the publish
  this morning, and it does not stop mattering because the operation changed
  from publishing to merging.
- Six of these trees hold uncommitted or untracked files —
  `lane1-declared-body-cancel` alone has eleven. Removing a tree that is zero
  commits ahead still destroys whatever was never committed in it. That is not a
  judgement I make on anyone's behalf without saying out loud what would go.

The survey is the part that needed doing and it is done. The merges are five
named operations with their conflicts already located, which is a morning's work
with no surprises left in it.
