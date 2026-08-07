# The open issues, as a whole picture — 2026-08-07

Written for Tom, at his ask: *"can we get a handle on all those open issues …
we need to make sure that we're considering the whole picture together."*

**Sorted by what a person actually ran into, not by which crate is at fault.**
That ordering is deliberate. Someone who is not deep in the system can only
describe the wall they hit, never what is behind it — so a report that says
"scaffold a worker" has to land somewhere real even though the truth was a
check asking the wrong question. Sorting by blame makes those reports
unroutable. Sorting by symptom makes them routable by the person who filed them.

## How to read the verification marks

Being honest about this matters more than the list looking tidy.

- **[VERIFIED 2026-08-07]** — I re-checked this today, at my own hand, against
  running code or the actual repo state. Trust it.
- **[AS FILED]** — recorded when it was found and **not re-checked since**. Some
  of these are days old. I expect one or two to be dead already. Treat each as a
  claim to re-test, not a fact.
- **[ROUTED]** — belongs to another seat; ours is to track, not to fix.

🔴 **Nothing below should be built against until its mark is VERIFIED.** The
cheapest step on any of these is re-testing whether it is still true.

---

# The five underlying causes

Thirty-odd items, but not thirty-odd mistakes. They are mostly five, wearing
different clothes. The clusters are the point of this document; the per-item
lists underneath are just the evidence for them.

| # | The pattern | Why it keeps happening | Cure |
|---|---|---|---|
| **A** | One rule is known in two places, and the two disagree | Nothing forces the copies to agree, so only diligence holds them equal — and diligence does not survive contact with a deadline | **Subtraction**: delete the copy, derive it from the one source |
| **B** | Something reports success or activity it did not measure | A green that measures nothing looks exactly like a green that measures everything | Make the check state what it measured, and prove it can go red |
| **C** | Real runtime breakage under durability | These are genuine defects, several already reproduced | Ordinary engineering — reproduce, fix, pin |
| **D** | A surface a newcomer needs simply is not there | We built inward-out; the first-contact path got the least traffic | Build the missing surface |
| **E** | A decision is parked, not a defect at all | Waiting on a ruling or on another lane | Decide, or leave parked deliberately |

**Cluster B is the most dangerous, and it is why the others survived.** A check
that cannot fail hides every defect underneath it. Fix B first and the rest
start surfacing on their own.

---

# A — Two places know one rule, and they disagree

*What a person runs into: "it told me to do something that isn't the problem",
or "the error message contradicts the manual".*

- **#154 — the console refuses to start a workflow the server can run.**
  🔴 **[VERIFIED 2026-08-07]** — Tom hit this live. Proven at my hand: same
  binary, zero workers, run passes, and the console's own pre-flight
  simultaneously says "no worker serves this queue". The pre-flight asks whether
  a *worker* exists; the deciding question is whether the *actions can be
  served*. It only ever receives a queue name, never the document, so it cannot
  tell the two apart. **The engine already knows the right answer**
  (`start_admission.rs`) and so does the CLI. Only the console does not.
  → Cure is A's cure exactly: ask the deployed contract, do not re-derive.
- **#152 — a `~` in `store.data_dir` is taken literally, and the server's own
  error then recommends the syntax that breaks it.**
  🔴 **[VERIFIED 2026-08-07 — BOTH HALVES]** The half I could not confirm before
  is now pinned: `error.rs:31` tells the operator to *"move `store.data_dir`
  beneath the private Aion home (`$AION_HOME`, default `~/.aion`)"*. And no
  expansion exists to make that advice work — the only `home_dir` call anywhere
  in `crates/aion-server/src` is a test helper at `state.rs:2690`, not config
  resolution. So the message hands over a path shape the loader cannot read.
  Two-for-one: the defect, plus the message that walks you back into it.
- **#153 — `validate_document_name` accepts a leading underscore the AWL lexer
  rejects.** 🔴 **[VERIFIED 2026-08-07]** `documents.rs:268` admits a leading `_`
  (`character == '_'`). The lexer does not: `aion awl check` on a `_foo`
  workflow gives `error: stray character `_``, exit 1. So the name passes the
  validator, then dies inside `new_document_source` when the generated source is
  parsed — and the failure is re-wrapped as `DocumentError::InvalidName`. Net
  effect: **you ask for a document name and are told about a stray character in
  source you never wrote.** Textbook cluster A.
- **#151 — a CLI error names an internal counter where the workflow id
  belongs.** 🔴 **[VERIFIED 2026-08-07 — reproduced live]** A failing `aion run`
  answers `error: workflow run failed: workflow 2 returned error`. "workflow 2"
  is an internal counter. The operator is handed a number that identifies nothing
  they can act on — and the same response carries the real ids in its `details`
  payload, so the useful identifier was in hand and was not used.
- **#88 — a Gleam-authored package with activities cannot start under
  queue-routed serving; only AWL can.** [AS FILED] Two authoring surfaces, one
  admission rule, and only one of them satisfies it.

**Read together, these are one job, not five.** Same shape every time, and the
same cure. Worth doing as a single sweep rather than five tickets.

---

# B — Something reports success it did not measure

*What a person runs into: "it said it was fine and it wasn't."*

- **#71 — CI reads alive and is dead.** 🔴 **[VERIFIED 2026-08-07]**
  Actions are **off** at the repo level (`enabled: false`); the last run was
  **26 July** and it **failed**; and GitHub still reports both workflows as
  `state: active`. **Per Tom's ruling 2026-08-07 this is now correct by
  intent — we do not want Actions running.** So this is no longer "CI is
  broken"; it is "the repo claims something that must never be true".
  → Recommendation pending Tom: delete both workflow files. Then nothing claims
  to be active and nothing has a timer. Off switch stays as a second lock.
- **#104 — the SDK gates are correctly built and were never run by any
  battery.** [AS FILED] A house battery now exists; the claim needs re-measuring.
- **#68 — the client conformance harness cannot catch what it exists to
  catch.** [AS FILED]
- **#67 — the TypeScript caller SDK has no drift gate.** [AS FILED]
- **#149 — the BC-4 differential gate is red on main AND nondeterministic**, so
  it cannot certify the byte-identity it exists to certify. [AS FILED]
- **#73 — drain reports "completed cleanly" while dispatches are still parked in
  selection.** [AS FILED] Belongs here and in C: it is both a false green and a
  real shutdown defect.

---

# C — Real runtime breakage

*What a person runs into: "it stopped, or it hung, and I lost work."*

- **#82 — engine NIF state goes missing mid-run.** [AS FILED] 🔴 This is what
  killed the tharsis fleet run after 55 minutes of real work. Highest-cost item
  on the board in terms of what a user loses.
- **#87 — the unserved-queue machinery never reached the gRPC outbox leg**, so
  that path waits forever, invisibly. [AS FILED] Invisibility is the harm: a
  visible wait is an inconvenience, a silent one is a lost afternoon.
- **#70 — a JSON-RPC `id: null` request is classified as a notification and
  never answered.** [AS FILED] The caller waits for a reply that will not come.
- **#90 — reverted; the fix WAS the defect, premise false, contract restored.**
  [AS FILED] Task deliberately still open — the original question is unanswered,
  and the record should not read as though it were.
- **#89 — a durable workflow outlives its agent session** (28-day-parked
  assistant could not resume). [AS FILED] Fix verified at the time.
- **#85 — libsql 0.9.30 double-close / use-after-free.** [AS FILED, re-reproduced
  2026-08-06] **A dependency defect, not ours.** Reproduces on contended
  workspace runs; isolated runs are clean. It is the known red in our battery.
- **#127 — haematite returns the wrong error type** (`Backend(HistoryCompacted)`
  where `SequenceConflict` belongs). **[ROUTED — Apollo]**
- **#92 — tharsis' second blocker**: a shell worker refused on schema drift.
  [AS FILED]

---

# D — A surface a newcomer needs is not there

*This is Tom's "people who aren't all that skilled" cluster. These are the
issues most likely to be reported vaguely, and most likely to lose someone
entirely — because there is nothing to hit, so there is nothing to describe.*

- **#136 — `aion new` has no AWL template.** 🔴 **[VERIFIED 2026-08-07]** Six
  templates, all Gleam, **zero AWL**. So the path is: finish the tutorial, learn
  AWL, type `aion new`, land in a language we stopped teaching. Directly adjacent
  to the tutorial work and cheap to fix.
- **#107 — there is no parent→children relation.** 🔴 **[VERIFIED 2026-08-07]**
  `list_children` appears nowhere in `crates/` at all, on a search whose control
  returned 479 files. The system cannot answer "what are this workflow's
  children" — a question anyone watching a fan-out asks within a minute.
- **#59 — live run progress in the console**: no heartbeat detail, no leg output
  tails. [AS FILED] Without it a long run is indistinguishable from a hung one.
- **#150 — three engine config knobs an operator cannot reach** from any surface.
  🔴 **[VERIFIED 2026-08-07 — all three]** `signal_delivery`, `completion_retry`,
  and `visibility_reconciliation_interval` each return zero hits across
  `crates/aion-server/src`, on searches whose control returned 180 files. My
  first hand-check of this got the denominator wrong and nearly refuted it; see
  [`../../../workflows/issue-recheck/README.md`](../../../workflows/issue-recheck/README.md).
- **#69 — `WorkerConfigBuilder` demands three settings the liminal transport
  ignores.** [AS FILED] Makes someone do work that changes nothing.
- **#30 — AWL-declared workers**: scaffold-from-studio. [AS FILED] Note this is
  the same territory as #154 — worth deciding together.

---

# E — Parked decisions, not defects

Listed so they stop counting as "open issues" in the worrying sense.

#23 conditional distribute · #32 Elixir on beamr · #34 run-1bb5889a hardening
trio · #37 feed-surface spec · #41 haematite descriptor-relative I/O (Apollo) ·
#42 authored-timeout opt-in · #62 artifact-dependency registry · #124
out-of-target build leak (mechanism confirmed, ~11.6 MiB, low priority) · #132
`manifest.entry_function` bound into no identity

# F — Lanes in flight (work, not faults)

#148 the #7 fix lane (mine — one review from done) · #139 assistant bundling
live cut · #77 tharsis · #57 remote-gates · #105 manifold readiness (Waffles) ·
#141 wire-with-oracle sweep · #142 held-on-trigger merge

---

# What I would do, in order

1. **Cluster B first**, starting with the GitHub workflow files once Tom rules.
   Everything else is measured by things in this cluster, so it is the only one
   whose failure hides the others.
2. **#136 and #154 together** — both are first-contact, both are cheap, and both
   sit on the path Tom just walked in front of an audience.
3. **Cluster A as one sweep**, not five tickets. Same mistake, same cure.
4. **#82** — the largest thing a user can lose.
5. Re-mark everything still **[AS FILED]** before building against it.

**The honest caveat:** three items on this page are verified today. The rest are
as-filed and may have rotted. Any of them could turn out already fixed, or
turn out to be two claims collapsed into one. Re-testing is the first step of
each, never the second.
