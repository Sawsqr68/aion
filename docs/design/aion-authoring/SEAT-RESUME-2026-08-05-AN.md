# Seat resume — marker AN, 5 August 2026

Supersedes AM. **Latest letter wins.** AM holds #102 and #103 in full and its
record stands; AK holds #84, the R2.4 amendment and #101.

`origin/main` = `HEAD` = `ce9a376f3`. **Tree clean** but for the three NO-SWEEP
untracked dirs. Nothing uncommitted.

## Board

- ✅ Closed today at this seat: #84 (`e314caf1`), #101 (`2afd537db`),
  #102 (`75dbbc4ff`), #103 (`be29abd8e`). R2.4 discharged (`cfc4ac42`).
- 🔬 **#107 MEASURED, NOT BUILT** — findings below. Still `pending` and it
  should stay pending: it is a missing relation with ADR-004 and Manifold
  implications, not a defect to patch.
- ⛔ #94 not taken (configured intervals, must land with #58). Route-retire and
  the hardcoded `DEFAULT_PAYLOAD_LIMIT_BYTES` / `DEFAULT_HISTORY_LIMIT` are
  **on Tom's docket via Waffles** — do not build against either.
- Held on Tom: #139 live cut, #142, the 0.11.1 decision. Queued: #141.

## 🔬 #107 — measured this evening, NOTHING BUILT

Taken up as the first live application of the new absence-claim rule (below).
**Every cited claim in the task verified true.** Unlike #101 and #103, this task
is well-cited and its facts hold.

### Verified, with citations

1. `aion_core::WorkflowSummary` (`crates/aion-core/src/filter.rs:78-79`) **has**
   `parent: Option<WorkflowId>` — it is on the served wire.
2. `core_summary_from_store` (`crates/aion-server/src/api/http/clean_dtos.rs:314-323`)
   **hardcodes `parent: None`**, and its own doc comment states why: *"parent is
   not carried in the visibility projection"*.
3. `aion_store::visibility::WorkflowSummary` (`crates/aion-store/src/visibility.rs:89-109`)
   has nine fields — `workflow_id`, `run_id`, `workflow_type`, `status`,
   `start_time`, `close_time`, `failed_step`, `failure_reason`,
   `search_attributes`. **No parent.**
4. The only production sites assigning `parent` are two hardcoded `None`s
   (`clean_dtos.rs:321`, `memory.rs:753`). The single `Some(parent)` in the tree
   is in `conformance.rs:580` — the store contract suite.
5. **Date-checked (the new rule):** `git log --since=2026-07-31 -S "parent_workflow" -- crates/`
   returns **nothing**. The task was filed 2026-07-31 and has not gone stale.

### 🔴 NEW finding the task does not state — the conformance gate pins the HOLE

`crates/aion-store/src/conformance.rs:578-585`:

```rust
let parent_filter = WorkflowFilter { parent: Some(parent), ..Default::default() };
expect_empty(
    store.query(&parent_filter).await?,
    "parent filters should return no workflows when no summary carries that parent",
)
```

Every backend must pass this. It asserts the parent filter returns **nothing** —
and since no summary can ever carry a parent, **it cannot fail.** A backend that
implemented parent linkage correctly would still pass; a backend that deleted the
`parent` field entirely would also pass. **It is a green gate over a hole, and it
would go on being green through any future fix.** Same family as the ledger case:
a verdict invariant to the error cannot see it. Whoever builds the relation must
rewrite this case, not just add to it.

### 🔴 Second finding — the wire carries a field that cannot be populated

A consumer reading `"parent": null` **cannot distinguish "this workflow has no
parent" from "this system does not record parentage."** That is strictly worse
than omitting the field, and it lands straight on Manifold, whose premise is a
tree of fanned-out agents.

The in-house precedent for fixing it properly is the `unserved` verdict on
describe, whose doc comment names exactly this class of problem in its own
words — a projected `Running` covers both "a worker is working on it" and "it is
parked with nobody to take it", so a separate field was added to tell them
apart. Same disease, already cured once in this codebase.

### 🔴 What I could NOT measure, and why the obvious number is worthless

I tried to witness the defect live and **could not**. Tom's store has **29
workflows and ZERO `ChildWorkflowStarted` events** — the event type does not
occur in any of the 29 histories. Live event-type census:

```
82 ActivityStarted · 78 ActivityScheduled · 55 ActivityCompleted
29 WorkflowStarted · 29 SearchAttributesUpdated · 21 ActivityFailed
18 WorkflowFailed · 8 WorkflowCompleted · 7 SignalReceived
3 TimerStarted · 3 TimerCancelled · 2 WorkflowCancelled
```

So **"0 of 29 workflows have a non-null parent" attributes NOTHING** — no
workflow in that store has a parent to report, so the null rate would be 100%
even on a system with perfect parent linkage. Recording this explicitly because
it is precisely the base-rate trap, and the number is tempting.
(I also confirmed I was not measuring my own spelling: `Event` is
`#[serde(tag = "type")]` and the variant is `ChildWorkflowStarted` verbatim.)

**Witnessing this live needs a run that actually spawns children** — an AWL
`distribute` document. That is a deploy + start on Tom's live server, which is a
side effect on his store, so it is NOT something to do unasked while the #139
cut is held. Named as the next measurement, not taken.

### Verdict

The task is **correct as written** and stays open. My contribution is the two
findings above plus the date-check. **Do not build the relation** — it is
schema-level (the store's visibility projection), it gates ADR-004, and Waffles
already holds route-retire and the defaults question for Tom.

## 🔴 The absence-claim rule — RULED, Waffles owns the template

Two tasks in two days (#101, #103) declared something missing that was findable
in one command. Waffles took ownership (both were filed at their level) and is
changing **the template**, on the stated ground that a discipline one person
maintains is not a mechanism:

> A task or brief asserting something is missing carries **the search that
> failed to find it** — the command and its empty output. **No search, no
> absence claim**; downgrade it to *"I could not find"*.

Mechanism, in their words: **an absence claim is the only kind of claim that
costs nothing to write and everything to act on.** Acting on a wrong one means
building a second copy of something that exists.

**My amendment, folded in verbatim as mine — the search carries the COMMIT the
tree was at when it ran.** A recorded search that goes stale is worse than no
search, because it looks settled: #103's would have been truthfully empty on the
morning of 2026-07-24 and false by that afternoon. Same command, same repo, same
person, opposite answer.

**The rule is symmetric.** Waffles cited a tool from memory that had never
existed — a presence claim, same root. Presence is the harder half: **absence
gets challenged occasionally, presence almost never does, because nobody audits
a citation that resolves.** It lands directly on recalled notes naming a file,
function or flag.

## New law from Waffles' ledger case — now a memory

Their published ledger was keyed on `seq` when `seq` is per recipient: 65 sends
collapsed into 45 rows, two rows matching neither unit invisible throughout,
open count five not three. **Every row still scored the same verdict, so the
headline stayed true while the rows under it were wrong, and nothing went red.**
It was frozen and hashed — **the hash was right and the contents were wrong.**

Same defect, mine, same day: my "951 passed / 46 targets" summed three nested
re-exec lines already counted in the parent. Green either way, so the error was
invisible to what I used the number for. Caught only by counting **rows**
(targets: 42, not 46) rather than the **total**.

**Law: a verdict invariant to a row-level error cannot detect it. Count the rows
and check the key, not the total.** Recorded at Waffles' seat as mine beside the
hash line: *the instrument being sound tells you nothing about whether it
measured the right thing.*

## Method memories current at this marker

`mutation-must-sit-on-the-tests-path` · `one-truth-in-two-places-has-drifted` ·
`confidence-is-what-stops-you-measuring` (now carries the absence rule + date
pin) · `narrowing-makes-downstream-tests-vacuous` ·
`a-verdict-invariant-to-the-error-cannot-see-it`.

## Scratchpad hygiene (Athena/Doug's hazard)

A dispatched agent's gate output silently overwrote a seat's evidence via a
filename collision — **a citation can point at bytes a later sibling replaced.**
This seat uses a `seat-*` prefix from marker AM onward (`seat-102-*.log`,
`seat-107-list.json`). No agents were dispatched today, so nothing cited in AM
or AN is at risk.

## Unchanged standing constraints

🔴 Worker rebuilds frozen until the #139 cut — the console is not a worker, and
restarting an already-built worker is not a rebuild. **Editing aion-server
source is not a worker rebuild, but do not restart Tom's live processes.**
🔴 Tom's `~/.aion/config.toml` `[ops_console] FileSystem` line must stay until he
installs a binary built from `b20d2da1` or later. 🔴 beamr JIT blocker with
Artemis. ⛔ Never `git add -A` here — name paths. ⛔ Never bare `cargo clean`.
Backticked commit messages via `git commit -F -`. Do not edit `CLAUDE.md`.
⛔ Never publish artifacts for Tom.

Live pids, untouched all session: server **45425**, worker **48399**, both from
the repo root.
