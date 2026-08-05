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

---

## 🔴 AMENDMENT 1 to this marker — my #107 enumeration was WRONG

Written after Waffles verified the report at their own hands and named two
`parent: None` sites I had not listed. **Amending rather than editing the claim
above**, so the error stays legible.

**The false claim, §"Verified, with citations" item 4:** *"The only production
sites assigning `parent` are two hardcoded `None`s (`clean_dtos.rs:321`,
`memory.rs:753`)."* A universal negative about a codebase — exactly the class I
had just finished ruling on — and I wrote it from a partial grep. The command
that falsifies it is one line:
`grep -rn "parent: None\|parent: Some" crates/ --include="*.rs"`.

### The corrected enumeration — FOUR sites, and they are not alike

| site | what it is | verdict |
|---|---|---|
| `aion-server/src/api/http/clean_dtos.rs:321` | HTTP surface, store summary → core summary | fabricates `None` |
| **`aion-client/src/ops.rs:663`** | **Rust caller SDK, the same conversion** | **fabricates `None` — I MISSED THIS** |
| `aion-store/src/memory.rs:753` | in-memory visibility projection | fabricates `None` |
| `aion-core/src/filter.rs:133` (`from_history`) | projection from event history | **NOT a defect — correctly justified** |

`ops.rs:888` is a test helper inside `mod tests`, not a production site.

**Why the miss matters:** `aion-client` is the Rust caller SDK. So the field
that cannot be populated is fabricated on **three** independent paths, not one,
and the false `"parent": null` reaches SDK consumers as well as HTTP consumers.
The blast radius of the wire finding is wider than I reported it.

### 🔴 The site I nearly reported as the WORST is the one that is RIGHT

`WorkflowSummary::from_history` also hardcodes `parent: None`, and read cold it
looks like the most damning site of all — the projection that reads the history
itself, discarding the linkage. **It is correct, and its doc comment says why:**

> *"Parent linkage is not present in a child workflow's own history, so this
> helper leaves `parent` unset."*

True. `ChildWorkflowStarted` is recorded in the **parent's** history; the child's
own history carries `WorkflowStarted` and nothing about who asked for it. No
amount of reading the child's events can recover the parent.

**That is the structural finding, and it is worth more than the defect list:
the relation cannot be derived after the fact — it must be WRITTEN at the moment
the child is started**, into the child's start envelope or its visibility row.
Anyone who plans to "just project it from history" will find there is nothing
there to project. Reading the justification is what produced this; the
assertion-message rule cuts both ways.

### And the filter itself is not a husk — only the STORE case is

`aion-core/src/filter.rs:378-388`, `parent_filter_matches_parent_workflow_id`,
asserts positively (`filter.matches(matching_summary)`) **and** negatively
(a summary with `parent: None` must not match). That test would fail if the
matching logic broke. So the earlier finding narrows and gets sharper: **the
core filter logic is genuinely proven; the husk is specifically the STORE
CONTRACT case** at `conformance.rs:578-585`, which asserts emptiness and cannot
fail. Two levels, opposite verdicts, and the wide claim "the parent filter is
untested" would have been false.

**Method note for myself:** I caught #107's base-rate trap and then walked
straight into a universal-negative in the same report. The check is cheap and I
did not run it because the claim felt like bookkeeping rather than a claim.
See [[confidence-is-what-stops-you-measuring]] — this is its third instance in
three days, and the first one that is mine twice over.

---

## 🔬 #141 LEG 1 MEASURED — console REST contract vs server registration

Started the wire-with-oracle lane at its cheapest named leg. **Read-only; no
oracle written yet.**

### Result: the contract table has NO dead link

All 21 paths in `AW_REST_CONTRACT.endpoints`
(`apps/aion-ops-console/src/lib/api/client-transport.ts:6-39`) are registered in
`workflow_router` (`crates/aion-server/src/api/http/router.rs:181-283`), methods
included. Checked by hand, both lists read whole. **This leg finds no bug — its
value is the oracle, not a hunt.**

### 🔴 But the contract table is NOT the console's REST surface

The table covers 21 endpoints. The console also calls, with literal paths in
other files and no contract entry: `/awl/check`, `/awl/fmt`, `/awl/edit`,
`/awl/scaffold`, `/awl/deploy`, `/awl/documents`, `/awl/documents/{path}`,
`/awl/layout/{path}`, `/awl/deployed`, `/awl/deployed/{type}/{hash}`,
`/awl/workers/availability`, `/awl/runs/{id}`, `/awl/runs/{id}/binding`,
`/workflows/transcript`, `/workflows/transcripts`, `/events/stream`,
`/health/live`, `/metrics`. **An oracle over the contract table alone would
cover barely half the calls and read as full coverage** — the denominator trap
again. #140's dead link lived outside this table, which is precisely why hand
inspection was what found it.

### 🔴 Two traps any path-scraping oracle must handle

1. **The console's own SPA routes look identical to server routes.**
   `app/routePaths.ts` declares `/workflows/:id` and `/authoring`;
   `TimelineEntry.tsx` builds `/workflows/${childWorkflowId}` as a browser link.
   None is a server call. A scraper that treats them as calls reports dead links
   that were never wires.
2. **A negation reads as a citation.** `features/failover/lib/clusterConfig.ts:7`
   contains the string `/cluster/shards`, which is registered nowhere — but the
   line is a comment stating *"No `/cluster/shards` endpoint"*. I flagged it as a
   candidate dead link and it survived exactly one read. **A grep cannot tell an
   assertion from its denial.**

### Design constraints carried forward for the oracle

- The absent side must come from the **server's own registration**, per the task.
  Axum exposes no route table, so the workable oracle is to build the production
  router in-process and issue one request per declared call, asserting the router
  did not answer "no such route."
- **The discriminator matters:** a router miss is a 404 with an *empty* body; a
  handler 404 (`WorkflowTypeNotFound` and friends) carries a JSON `WireError`.
  Method mismatch is a 405, also empty. So the assertion is *not (404-or-405
  with an empty body)*, never status alone.
- **Feature-gated families must be mounted for the run:** `/deploy/*`,
  `/authoring/*` and `/dev/*` answer a plain 404 when switched off, deliberately
  *"indistinguishable from an unmounted route family"* (`router.rs:122-148`).
  A test on default config would score the deploy endpoints dead.
- `clusterCommand` is in `endpoints` but has **no `methods` entry** — the `'POST'`
  is a literal at `client.ts:158`. A (path, method) pair oracle keyed on
  `methods` silently drops it. **Key on `endpoints`, join methods, and assert
  the two objects have the same key set**, or the denominator lies.

**Nothing built. Leg 1 is measurement only, and #141 remains queued behind the
#139 cut.**

---

## 🔴 AMENDMENT 2 — #141 LEG 2: the task's SDK and CLI bullets are the wrong wire class

Read-only continuation of the sweep across the scopes the task names beyond the
console. **The result shrinks the lane rather than extending it.**

### Measured: every non-console caller reaches the server over gRPC

| caller | transport | evidence |
|---|---|---|
| ops console | **HTTP + WebSocket** | `AW_REST_CONTRACT` + literal paths, `/events/stream` |
| `aion` CLI | gRPC | `aion-cli/src/main.rs:615` builds `aion_client::ClientBuilder`; deploy uses `aion-proto` generated stubs (`Cargo.toml` comment says so explicitly) |
| `aion-client` (Rust SDK) | gRPC | endpoints are `http://…:50051`, tonic |
| Python SDK | gRPC | `workflow_pb2_grpc.py`, `/aion.WorkflowService/*` |
| TypeScript SDK | gRPC | no HTTP path literals in `src/` at all |
| workers | liminal transport | `aion-worker` `liminal-transport` feature; **not HTTP** |

`crates/aion-cli/src` contains **no HTTP API path literal** — every `"/…"` in it
is a filesystem path or a test fixture. And `aion-server` builds exactly one
production HTTP router (`http_router` → `workflow_router` + ops-console assets
+ metrics/health); there is no second HTTP surface for workers.

### 🔴 The finding: the dead-link class is STRUCTURALLY CONFINED to the console

A gRPC method path is **generated from the shared proto descriptor**. It is a
derived artifact, not a hand-written string, so it cannot drift the way
`client-transport.ts` can. #140's defect — a caller compiling happily against a
route that never existed — **is not reachable through a generated stub.**

⇒ **The task's bullets for "the Python and TypeScript SDKs" and "the CLI's
endpoints" are the wrong wire class for this lane.** What can go wrong there is
**message shape**, not path existence, and that is already owned by #67 (TS
caller SDK drift gate) and #68 (client conformance harness). Folding them into
#141 would have produced an oracle that measured nothing, in the same suit as
the husk gate.

⇒ **One oracle over one consumer — the ops console — covers the entire dead-link
class.** That is a much smaller lane than the task describes, and it is smaller
for a reason that can be stated, not because scope was quietly dropped.

### 🔴 Bears directly on ROUTE-RETIRE (Tom's docket, Waffles holds it)

If the console is the only first-party programmatic HTTP consumer, then an HTTP
route with no console caller may have **no first-party caller at all**. Routes
registered in `workflow_router` that no console file calls:
`/workflows/query`, `/workflows/cancel`, `/workflows/unrecoverable`,
`/outbox/dead-letters`, `/queues/unserved`, `/workers/{id}/drain`,
`/workers/{id}/stop`, `/schedules` and its four sub-routes, `/build`,
`/awl/emit`, `/awl/revisions/{hash}`, `/authoring/compile`, `/dev/*`.

**🔴 THE BOUND, and it is load-bearing: "no console caller" is NOT "no caller."**
`docs/operations/API.md` documents this surface for operators driving it by
hand, `/build` is the version check, `/health/*` and `/metrics` are for
monitoring, and this repo cannot see an external consumer at all. **The list
above is a list of routes with no first-party programmatic caller in this
repository — it is an INPUT to the retire question, never an answer to it.**
Presenting it as a retire list would be exactly the global-gauge error: measuring
the box and attributing to the actor.

**Still nothing built. Both legs are measurement; the oracle stays queued.**
