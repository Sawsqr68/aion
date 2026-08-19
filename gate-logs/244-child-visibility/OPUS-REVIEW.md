# OPUS REVIEW OF RECORD — aion #244 (child-workflow visibility)

**VERDICT: FAIL**

Reviewer: Opus 5, review of record.
Subject: branch `244-child-visibility` @ `dd5073bf2`, 8 commits atop base `7109c1f6d`.
Worktree reviewed: `/Users/tom/Developer/ablative/stack/aion/.worktrees/244-child-visibility` (untouched by this review; `git status --porcelain` empty at start and end).
Base-reproduction worktree created by this review and LEFT IN PLACE: `/Users/tom/Developer/ablative/stack/aion/.worktrees/review-244-base` (detached, returned to `7109c1f6d`, working tree clean). It carries its own `target/` — **12G on disk** — so remove it (`git worktree remove .worktrees/review-244-base`) once these findings are settled and nobody needs the baseline re-run.
Contract: `/Users/tom/Developer/ablative/stack/aion/gate-logs/244-child-visibility/BRIEF.md`.

## Verdict in one paragraph

R1 is good work and the HARD WALL is respected — the children endpoint derives
purely from `ChildWorkflowStarted` in the parent's own history, with no schema
change and no stored parent→children relation anywhere in the diff. R2 ships and
renders. But **R3's live half does not work at all**: `aion tail --follow`
decodes a WebSocket frame shape the server has never sent, so after the retained
backfill prints, every live transcript event is silently discarded and the
terminal sits quiet forever while the legs stream. That is not a cosmetic miss —
it is *the exact defect class #244 exists to remove* (a stream that reads as
"working" while nothing arrives), reintroduced in the verb built to cure it, and
it is silent by construction because serde ignores unknown fields. It shipped
because the brief's required R3 e2e was never written: nothing in this diff ever
connected `aion tail` to a running server. Three further honesty regressions
(legs section swallows the transcript's own error channels; CLI silently omits
not-yet-started legs; CLI never notices fanout that happens after it starts)
compound the same theme, and three `let _ = sender.send(...)` swallows sit in the
new CLI live path. Verdict is FAIL on R3 + R4 and on red-first discipline for the
CLI leg.

Two things the builder got right and should be credited: the R1 hard wall is held
cleanly, and the R1 red pin is genuine (I reproduced its 404-vs-200 failure at the
pin commit). Both claimed pre-existing reds are **PRE-EXISTING CONFIRMED** on base
`7109c1f6d`, plus a third the builder's narrower test invocation never reached
(`aion-cli awl_corpus_gate`) — also pre-existing. The full workspace battery
(271 targets, 5053 passed) is red on exactly those pre-existing failures and
nothing else: **this diff introduces no gate failure**. The FAIL is about what the
code does, not about a red gate.

---

## Requirement-by-requirement

### R1 — Server: derived children endpoint — MET (with defects listed below)

`POST /workflows/children` (`crates/aion-server/src/api/http/children.rs`) reads
the parent history, selects the addressed run segment (or the latest recorded
run when `run_id` is omitted), walks `Event::ChildWorkflowStarted`, and projects
each child from the child's OWN history (`display_name`, run id, status,
current activity + attempt). Dedupes repeated `ChildWorkflowStarted` for one
child id. Authorization reuses the existing `scoped_engine` namespace gate
(`history.rs:148`, visibility widened from private to `pub(super)` — the minimum
change).

**HARD WALL (#107) — HELD.** I checked the whole diff for schema/relation work:
no migration files, no `aion-store` changes, no new persisted column/table/index,
no parent→children record written anywhere. The only server files touched are
`children.rs` (new), `history.rs` (one visibility keyword), `mod.rs` (two module
declarations), `router.rs` (one route + a test-module extraction). The wall was
not approached, let alone crossed. The module doc states the design intent
explicitly ("intentionally a read projection, not a parent/child schema
relation"). Depth is one level and is disclosed both in the module doc and in the
console's own section copy ("Direct child runs derived from this run's recorded
history") — an acceptable discharge of the brief's depth clause.

Defects against R1 are F9 (Debug-formatted status + invented `Pending`), F10
(undocumented route), F12 (unwindowed N+1 full-history reads) and F13 (child
histories read without the per-workflow visibility gate).

### R2 — Console: run page folds its legs in — MET IN SUBSTANCE, honesty defects

`LiveLegsSection` renders under the parent's history (the parent's own quiet
history stays visible — the brief's "answers the quiet, doesn't hide it" is
honoured). Each leg shows display name, short id (`shortWorkflowId`, #211 law),
status, current activity + attempt, and per-attempt live transcripts through the
existing `useTranscript` (socket) path. Discovery is one REST read keyed on
`['workflow-children', namespace, workflowId, runId, discoveryRevision]` with no
`refetchInterval` — no polling theater; `discoveryRevision` is driven by the live
parent socket's latest `ChildWorkflowStarted` seq (`dd5073bf2`), so fanout on an
already-open page is picked up from the socket rather than a timer. That is the
right shape.

Defects against R2 are F3 (drops `socketError`/`backfillError`), F4 (the pinned
fixture tests a component production never renders), F11 (`nullableString`
silently coerces).

### R3 — CLI: `aion tail` — NOT MET

Snapshot mode (`--no-follow`) is sound: it discovers legs via R1, enumerates
streams, backfills, sorts by `emitted_at`, prints with leg prefixes. Follow mode
is broken (F1), never picks up post-start fanout (F6), silently drops legs whose
run has not started (F5), silently stops following a leg that continues-as-new
(F7), and dies entirely if any one leg's transcript stream lags (F1b). None of
this was caught because there is no e2e for the verb (F2).

### R4 — Attempt honesty everywhere — PARTIALLY MET

The vocabulary is present on every surface: CLI prefix carries
`activity:N attempt:M` and ` SUPERSEDED→K`, plus an explicit "superseded; this
stream is no longer working" line; console shows `attempt N` and
"Superseded by attempt K", and `data-attempt-state` is on the DOM node. Good.
But R4's second clause — *"a stream that went quiet because its attempt was
superseded says so rather than reading as working"* — is exactly the clause F1,
F3, F5 and F7 violate: four distinct ways a stream in this build goes quiet while
saying nothing at all. R4 is met for supersession and unmet for quiet.

---

## Findings

Severity: **CRITICAL** = fails the review on its own. **IMPORTANT** = must fix
before landing. Per house law there are no "minor" items; everything below is
work, nothing is deferrable.

### F1 — CRITICAL — `aion tail --follow` silently discards every live transcript frame

`crates/aion-cli/src/tail/live.rs:17-21`:

```rust
#[derive(Debug, Deserialize)]
struct ActivityFrame {
    activity_event: Option<ActivityEvent>,
    error: Option<Value>,
}
```

The server never sends a field named `activity_event`. `encode_activity_frame`
(`crates/aion-server/src/stream/transcript_stream.rs:252-259`) serializes
`StreamedActivityEvent` (`crates/aion-proto/src/events.rs:180-190`), whose shape
is:

```json
{"kind": "activity_event", "event": { …ActivityEvent… }}
```

`kind` is the discriminator; the event is under `event`. Serde derives ignore
unknown fields by default, so every transcript frame deserializes to
`ActivityFrame { activity_event: None, error: None }`, falls through both `if let
Some(...)` arms at `live.rs:224-232`, and is dropped without a word. The console's
own parser is the corroborating witness: `readActivityEventFrame` checks
`frame.kind !== TRANSCRIPT_FRAME.event` then reads `frame.event`
(`apps/aion-ops-console/src/lib/api/transcript-stream.ts:427-437`), and its test
constructs `JSON.stringify({ kind: 'activity_event', event })`
(`transcript-stream.test.ts:34`).

Operator consequence: `aion tail <parent>` prints the retained backfill, then
prints nothing ever again while the legs stream thousands of events. That is
Tom's original K9 experience, reproduced by the tool built to fix it, and it is
silent by construction. This alone is the FAIL.

**Executed, not argued.** I built a standalone serde probe carrying the reader
struct verbatim and the writer's frame bytes
(`<scratchpad>/frameprobe`, `cargo run`, exit 0):

```
decoded = ActivityFrame { activity_event: None, error: None }
activity_event.is_none() = true
error.is_none()          = true
=> CLI prints nothing for this frame: true
lag frame error = Some(Object {"code": String("transcript_lagged"), "skipped": Number(12)})
```

Serde treats a missing `Option` field as `None` (no `deny_unknown_fields`, no
rename, no flatten), so the decode SUCCEEDS and the event vanishes at
`live.rs:227` — there is not even a decode error to notice.

Fix: decode the real frame (`kind` + `event`), and make an unrecognized frame
**loud** — an unknown `kind` must surface, not `continue`. A frame-decode unit
test built from `StreamedActivityEvent::new(...)`'s actual JSON (not a
hand-written literal) is the control that keeps the two ends welded; see the
"protocol changes land at both ends" discipline — the reader must be written
against the writer's bytes.

### F1b — CRITICAL — one leg's lag kills the whole tail

The `transcript_lagged` terminal frame IS `{"error": {...}}`
(`transcript_stream.rs:271-279`), so it decodes into `ActivityFrame::error` and
`tail_socket` returns `Err`, which `follow`'s `select!` turns into an immediate
`return Err(error)` for the ENTIRE tail (`live.rs:64-68`). A single busy leg
overflowing its broadcast buffer therefore tears down the operator's whole
session, including the healthy legs. The console treats the same frame as a
recoverable reconnect-and-re-read (`transcriptLaggedError` →
`transcript-stream.ts:447-455`). "One story" requires the same handling.

### F2 — CRITICAL — the brief's R3 acceptance red was never written

BRIEF.md: *"Pin it: an e2e that starts a parent-with-children fixture, asks the
R1 endpoint / R3 verb, and asserts leg visibility — written first, committed
failing against the current tree, then built green."*

The R1 half exists and is a genuine red pin (see "Red-first discipline" below).
The **R3 verb half does not exist**. `aion tail`'s entire test surface is:
`prefix_keeps_attempt_and_supersession_visible` and
`current_attempts_uses_the_highest_attempt_per_activity` (two pure functions over
hand-built structs) and `tail_accepts_parent_id_without_any_child_id` (clap
parsing). Nothing in the diff ever runs the verb against a server, or against a
recorded server frame. F1 is the direct consequence: a single test that fed one
real server frame to the decoder would have caught it.

### F3 — IMPORTANT — the legs section drops the transcript's own error channels

`LiveAttempt` (`LegsSection.tsx:183-217`) consumes `useTranscript` but reads only
`transcript.status` and `transcript.entries`, discarding `socketError` and
`backfillError`. The repo's existing consumer does surface both:
`TranscriptPanel.tsx:118-133` renders the socket error message and *"Retained
transcript fetch failed: … — live streaming is paused to avoid an unbounded
replay."* So on a leg whose socket errored or whose retained fetch failed, the
new surface renders "Waiting for transcript events." indefinitely — the honest
information exists in the hook and is thrown away at the last step. Reuse
`TranscriptPanel` rather than re-implementing a lossier copy of it.

`transcript.status` does not cover the gap: `ConnectionStatus` is
`connected | resynced-with-possible-gap | reconnecting | idle | disconnected`
(`websocket-types.ts:54-59`) — it has no error member, so a broken socket reads
as the bare word `disconnected` with the cause dropped, and a **failed retained
backfill is not represented in `status` at all** (that lives in
`backfillState`/`backfillError`), so the header can read `connected` while the
body says "Waiting for transcript events." forever. The type's own doc comment
states the intent this code breaks: each error kind "maps to a distinct,
human-readable cause so a view can render *why* the stream is degraded rather
than swallowing the error to the console."

### F4 — IMPORTANT — the pinned console fixture tests a component production never renders

`legs.test.tsx` renders `LegsSection`; the run page renders `LiveLegsSection` →
`LiveLeg` → `LiveAttempt` (`WorkflowDetailView.tsx:72`). `LegsSection` and its
`AttemptHeading` have no other caller in `src` (verified by grep) — they exist
for the test. So the brief's "console side pins the legs section renders from a
recorded fixture" is discharged against a parallel presentation copy while the
shipped surface is unpinned, and the two copies of the header/attempt markup must
now be hand-kept in agreement. Either render `LiveLegsSection` in the fixture test
with an injected reader/manager, or make `LiveLeg` a thin container over the
`LegsSection` presentational component so the pinned markup IS the shipped markup.

### F5 — IMPORTANT — the CLI silently omits a leg whose run has not started

`tail.rs:148-156`: `filter_map(|child| { let child_run = child.run_id?; … })`. A
child with a recorded `ChildWorkflowStarted` but no `WorkflowStarted` yet is
dropped from the source list with no output. The console handles the same state
honestly ("Child start recorded; run not started yet.", `LegsSection.tsx:150-154`)
— the CLI must say the same thing rather than present a short list as complete.

### F6 — IMPORTANT — the CLI never sees fanout that happens after it starts

Discovery runs exactly once, before the socket phase. `monitor_attempts`
subscribes to each *known* workflow's events and matches only
`Event::ActivityStarted` (`live.rs:117-124`); `ChildWorkflowStarted` on the
parent's stream falls through the `else { continue }`. So an operator who runs
`aion tail` on a parent and *then* watches it fan out — which is Tom's literal
scenario in the brief's ground truth — never sees a leg appear. The console lane
recognised precisely this gap and fixed it in `dd5073bf2` ("discover fanout from
live parent events"); the CLI did not get the same fix, so R3's "terminal
operators get what the console gets — one story" is unmet. The parent socket is
already open and already carries the event; re-derive on it.

### F7 — IMPORTANT — a leg that continues-as-new goes quiet with no notice

`live.rs:111-116`: on a `WorkflowStarted` whose `run_id` differs from the source's,
`monitor_attempts` simply `return`s. The generation rolled over, the tail stops
following that leg's attempts, and nothing is printed. Same class as F1/F5: quiet
that reads as working. Say it ("leg X continued as new as run Y; no longer
following this generation") and, ideally, follow the successor.

### F8 — IMPORTANT — swallowed `Result`s in the new live path

`live.rs:105-108`, `136-141`, `179`: three `let _ = sender.send(...)`. The one at
105 is the *error-reporting* path — if that send fails the operator loses the only
notice that a leg's event socket died. House law is explicit: no swallowed
Results. Handle each (at minimum, report to stderr when the channel is gone).

### F9 — IMPORTANT — wire status is a `Debug` format string, and invents a value the domain has no name for

`children.rs:113-117`:

```rust
let status = if run_id.is_some() { format!("{:?}", status_from_events(run)) }
             else { "Pending".to_owned() };
```

`WorkflowStatus` derives `Serialize` **and** `ts_rs::TS`
(`crates/aion-core/src/status.rs:12`), i.e. there is an existing typed contract
with a generated console type, and this endpoint bypasses it to ship a
`Debug`-rendered `String`. `Debug` output is not a wire contract — nothing gates
it, and a derive change silently re-spells every status. Worse, `"Pending"` is not
a `WorkflowStatus` variant at all: a new status vocabulary word is invented at
the HTTP boundary. The console then maintains a *second* hand-written mapping of
the same vocabulary (`liveStatus`'s `labels` record, `LegsSection.tsx:239-246`).
One truth, three copies. Serialize the enum; express "start recorded, run not yet
begun" as the already-present `run_id: null`, not as a fictional status.

### F10 — IMPORTANT — the new route and the new verb are undocumented

`docs/operations/API.md` is this repo's endpoint table and every neighbour is in
it (`/workflows/transcript`, `/workflows/transcripts`, `/workflows/attempts`, …
lines 54-69). `POST /workflows/children` was not added. The convention is
established by comparable feature commits — `37f775674` (#211) is a feature
commit that updates this same file. `aion tail` likewise appears in no doc.
Additionally, `children.rs`'s request/response DTO fields carry no field docs
while the neighbouring `transcripts.rs` documents every field of every wire
struct; on a wire contract that is the documentation that matters.

### F11 — IMPORTANT — inconsistent validation silently mistypes a leg's run

`children-read.ts:88-92`: `nullableString` returns `null` for any non-string, so a
`run_id` arriving as a number/object silently becomes `null` — which the UI then
states as "Child start recorded; run not started yet." A wrong-typed field
becomes a confident false statement to the operator. `nullableNumber` two lines
below *throws* on the same class of input. Make the string reader throw for a
present-but-wrong-typed value, exactly as the numeric one does.

The sibling client this one was modelled on settles the question: `readStreamHead`
in `transcript-read.ts:169-183` **throws** on a wrong-typed `run_id`
(`typeof runId !== 'string' || runId === ''`). So this is a deviation from the
house pattern in the very file it copies, not a matter of taste.

(Checked and NOT a finding: `throw new ApiError(200, …)` for a body-shape defect
is the established convention here — `transcript-read.ts:109-178` does exactly
that in six places. I flagged it, then read the neighbour and withdrew it.)

One smaller item, still work: the discovery failure renders as
`Could not load child legs: ${String(children.error)}`
(`WorkflowDetailView.tsx:75-79`), and `String(ApiError)` yields
`Error: <message>` — the operator sees a stringified-object prefix rather than
the typed `.message` the console surfaces elsewhere.

### F12 — IMPORTANT — unwindowed N+1 full-history reads per page load

`list_children` calls `read_history` on the parent and then on **every** child, in
series, returning whole histories, and keeps only a handful of projected fields.
The console re-issues this whenever `runId` or `discoveryRevision` changes. On the
K9-scale runs this feature is aimed at (fleet parent + legs with thousands of
events each) that is a large, repeated read to answer a question about a few
events. I am not proposing an arbitrary cap (house law) — this needs a design
answer: a bounded/filtered history read, or a projection the store can answer
without materialising the full generation. Raise it and settle it rather than
shipping the loop.

The console half compounds it rather than causing it: each `LiveLeg` also opens
its own history window (`useWorkflowHistory`) plus a workflow-event socket
(`useLiveWorkflowEvents`) plus one transcript socket per displayed attempt
(`LegsSection.tsx:113-119, 183-194`). Per leg that is defensible and the history
read is windowed — but it means opening one parent run page now costs
(server-side) every child's full history and (client-side) a history window and
two-plus sockets per child. Size that against a real K9 fanout before it lands.

### F13 — IMPORTANT (authorization) — child histories are read without the per-workflow visibility gate

`list_children` authorizes **only the parent**: `scoped_engine(...,
&request.workflow_id)` builds `WorkflowTarget::workflow(parent)` and scopes the
guard on that (`history.rs:148-172`). It then calls
`engine.store().read_history(child_workflow_id)` for every child id found in the
parent's history (`children.rs:83-92`) with no second gate, and projects the
child's `display_name`, status, current activity and attempt out of that foreign
history.

Every neighbouring surface does gate per workflow id — `gate_transcript_workflow`
re-scopes `WorkflowTarget::workflow(workflow_id)` for both the transcript socket
and the REST fetch precisely so "a caller probing a foreign or nonexistent
workflow receives the guard's anti-leak `not_found`, never a transcript"
(`transcript_stream.rs:152-177`). The child ids and `workflow_type` are already
disclosed by the parent's own history, so those are not new; **the child's
recorded display name and live progress are new disclosure**, taken from a
workflow the caller was never authorized against. In the common fleet case the
child sits in the same namespace and nothing leaks — but the endpoint asserts
that rather than checking it, and "usually the same namespace" is not an
authorization argument on a surface whose siblings all check.

Fix: scope each child through the same per-workflow guard before projecting it,
and decide explicitly (in the wire contract, not by accident) whether an
unauthorized child is omitted or rendered as an opaque "a child you cannot see"
row. Either answer is defensible; silence about the question is not.

### F15 — IMPORTANT — an undiscussed 2000-event cap that truncates silently

`const DEFAULT_BACKFILL: u32 = 2_000;` (`tail.rs:14`), passed as `"last"` to
`/workflows/transcript` for every stream, with no flag to change it. Two house
laws land on this at once. *No arbitrary limits*: the brief asked for a backfill,
not a cap, and 2000 was chosen here rather than discussed. *No silent failures*:
the response carries `head_seq`, so the CLI **knows** when it showed the operator
2000 of 2759 retained events — and says nothing. On the very K9 streams the brief
cites (heads measured at 2667→2759) this cap is already binding today, so the
first real use of `aion tail` silently drops the earliest ~750 lines of a leg.
Either print the truncation (something the operator can act on: "showing the last
2000 of N retained events for this stream") or make the depth an argument whose
default Tom settles — and say which, in the verb's help, either way.

### F16 — IMPORTANT — the HTTP endpoint is a re-typed constant with no env path

`--http-endpoint` defaults to the literal `"http://127.0.0.1:8080"`
(`main.rs:548-551`). The value is CORRECT — it matches
`DEFAULT_HTTP_ADDRESS` (`crates/aion-server/src/config/defaults.rs:12`), so this
is not an invented default — but it is that constant's second, hand-maintained
home, typed into a clap attribute where nothing compares the two. The CLI also
resolves `--token` through `deploy::resolve_token` (which reads the environment)
while the new HTTP endpoint has no environment path at all, so an operator
pointing the CLI at a non-default deployment must pass the flag on every
invocation while every other connection detail follows the settings they already
have. Derive it, and give it the same override story its siblings have.

### F17 — IMPORTANT (architecture) — a second WebSocket client was hand-rolled beside the one that already speaks this channel

`aion-client`'s WS transport already handles the transcript subscription variant
— `Some(subscription_request::Subscription::Transcript(transcript))` at
`crates/aion-client/src/transport/ws.rs:208-215`, with the channel's resume-cursor
rule encoded (`reject_live_only_cursor`) and the auth / `x-aion-subject` /
`x-aion-namespaces` headers applied by `apply_headers`. What it lacks is a
*public* method that constructs one: `stream.rs` exposes `event_stream` /
`event_stream_from` and nothing transcript-shaped.

Rather than add that method in the one place that is tested against the server's
own proto types, this lane opened a second WebSocket client inside `aion-cli`:
`tokio_tungstenite::connect_async` with a hand-written subscribe payload and a
hand-written frame struct, and pulled `reqwest`, `tokio-tungstenite` and `futures`
into `aion-cli`'s dependency set to do it. F1 is the bill for that: the hand-written
decoder was never checked against the writer, while the library that already knows
this channel's shapes sat one crate away. (The subscribe payload the CLI sends IS
accepted — I checked `decode_subscription_request`'s tolerant wrapped form — so
only the read direction is wrong. That asymmetry is itself the tell: the send
direction was validated against the server's decoder, the receive direction
against nothing.)

Fix direction: expose the transcript subscription on `aion-client` and have the
CLI consume it, so there is one implementation of this wire contract with one
place to test it.

### F18 — NOTE — the ≤500 LOC law: new files comply, `main.rs` grew

Every file this lane creates is inside the bound: `children.rs` 212,
`tail.rs` 398, `tail/live.rs` 262, `LegsSection.tsx` 247, `children-read.ts` 108,
and `router.rs` came DOWN from ~751 to 409. Good discipline, and the decision to
put the tail implementation in its own module pair rather than inline is right.

The exception: `crates/aion-cli/src/main.rs` went 2710 → 2771 lines. The
violation is emphatically pre-existing (it was 2710 on base) and this lane adds
61 lines — the clap variant, the dispatch arm and a parser test. But house law is
"no god files … `main.rs` is a thin entry point", and each lane that adds to this
one defers the decomposition further. The repo already has the pattern to follow
(per-verb modules); the `Tail` variant and its dispatch arm belong beside
`tail.rs`, not in the entry point.

### F19 — NOTE — extraneous churn in the diff, disclosed and benign

`4ba205e17` moves 330 lines of pre-existing `router.rs` tests into
`router_tests.rs` (via `#[cfg(test)] #[path = "router_tests.rs"] mod tests;`) and
extracts `dev_router`. I read both sides and then checked it mechanically rather
than trusting the reading: comparing the set of function names in base
`router.rs` against the union of head `router.rs` + `router_tests.rs`, **nothing
present at base is missing at head** (17 → 18 names; the single addition is
`dev_router`). No test was quietly dropped inside a "move", and the diff also
contains zero removed test functions overall. Legitimate — but it inflates this
lane's diff with an unrelated file and should be called out in the landing note
so a future bisect knows where those tests went.

---

## Red-first discipline

**R1 pin — GENUINE RED, correctly ordered.** `e9e19bcac` ("test: pin parent run
child visibility red case") is test-only: it adds
`crates/aion-server/src/api/http/children_tests.rs` and declares the module. It
seeds a parent history with `WorkflowStarted` + `ChildWorkflowStarted` and a child
history with `SearchAttributesUpdated` (display name) + `ActivityScheduled` +
`ActivityStarted { attempt: 2 }`, then POSTs `/workflows/children` and asserts
`200` plus the exact projected body. At that commit the route does not exist —
`06823ec0b` adds it one commit later — so the router answers 404 and
`assert_eq!(response.status(), StatusCode::OK)` fails. It fails for the right
reason (endpoint absent), not for a compile error: it is written entirely against
pre-existing `test_support` helpers.

**I reproduced it rather than reading it.** Checked `e9e19bcac` out in the base
worktree and ran `cargo test -p aion-server --lib parent_run_derives` (exit 101):

```
test api::http::children_tests::parent_run_derives_its_visible_leg_from_recorded_history ... FAILED
panicked at crates/aion-server/src/api/http/children_tests.rs:54:5:
assertion `left == right` failed
  left: 404
 right: 200
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1112 filtered out
```

`1112 filtered out` proves the crate compiled — so this is a behavioural red (the
route answers 404 because it does not exist), not a "red" that is really a
compile error. That is a genuine pin, correctly ordered before all seven
implementation commits. Credit where due: this half of the red-first law was done
properly.

**R3 pin — ABSENT.** See F2. The CLI verb was built with no failing test in front
of it, and F1 is what that bought.

**R2 pin — present but aimed at the wrong component.** See F4.

---

## Gates re-run by me (not accepted from the builder)

All commands run by me. Rust commands run in the review worktree
`/Users/tom/Developer/ablative/stack/aion/.worktrees/244-child-visibility` at
`dd5073bf2` unless the row says base.

**EVIDENCE CONTAMINATION WINDOW — 04:28Z–04:32Z (14:28–14:32 local, UTC+10).**
An external `rm -rf` ran for ~4 minutes against
`.worktrees/244-child-visibility/target` — another operator's disk-sweep error,
nothing to do with the #244 diff. I dated every gate against that window from the
log mtimes: rows 1, 2, 2b, 5 finished 03:27Z–03:32Z and rows 8, 9, 10, 11 ran in
the separate `review-244-base` worktree (finishing 04:25Z), so **every row except
row 7 is pre-window or off-target and uncontaminated**. Row 7 (the workspace
battery leg) was executing across the window and is marked accordingly; the
console rows touch no cargo target at all. Per the notice, greens remain
trustworthy — a deletion cannot manufacture a pass.

| # | Command | Exit | Result |
|---|---------|------|--------|
| 1 | `cargo fmt --check` | 0 | clean |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | 0 | clean; zero `warning:` lines in the whole run |
| 2b | bypass scan over the raw diff (`git diff --no-ext-diff 7109c1f6d..dd5073bf2`, 2009 added lines) for `#[allow` / `#[expect` / `#[ignore` / `cfg(any())` / `biome-ignore` / `eslint-disable` / `@ts-ignore` / `@ts-expect-error` / `todo!` / `unimplemented!` | 0 hits | **no lint bypasses anywhere in the diff.** Also zero `.unwrap()` / `.expect(` / `panic!` in added Rust (the one `unwrap_or` at `tail.rs:150` is a default, not a panic). Three `let _ = sender.send(...)` — finding F8 |
| 3 | `bun run typecheck` (apps/aion-ops-console) | 0 | `tsc --noEmit` clean |
| 4 | `bun test` (apps/aion-ops-console) | 0 | 1006 pass, 0 fail, 133 files |
| 4b | `bun run check` (biome, apps/aion-ops-console) | 0 | 435 files, 2 warnings — **both outside this lane's files** (`update/UpdateStatusPanel.tsx:59` and `update/doc-adjacency.test.ts:77`, `noExcessiveCognitiveComplexity`). The builder's "two pre-existing complexity warnings" is accurate; the new console files are biome-clean |
| 4c | `bun run build` (vite, apps/aion-ops-console) | 0 | built in 6.33s; the >500 kB chunk advisory is pre-existing bundle shape, not this lane |
| 5 | `cargo test -p aion-server -p aion-cli` | 101 | aion-server lib 282 pass; aion-cli `tests/awl_corpus_gate.rs` FAILED (`every_living_awl_document_checks_clean`, `workflows/fleet-dev/fleet_dev.awl` agent-return-shape errors). Dated against base — see claims table |
| 6 | `cargo clippy -p aion-server -p aion-cli --all-targets --no-default-features --features libsql-backend` | 101 | slim matrix. Error set is **byte-identical to base**: `state.rs:1907` unused_self + `worker/registry.rs:1150` unnecessary_filter_map, then `could not compile aion-server (lib)/(lib test)`. Zero occurrences of `children.rs`, `tail.rs` or `tail/live.rs` anywhere in the run. Not introduced |
| 7 | `cargo test --workspace --no-fail-fast` (battery `rust:test` leg) | 101 | **271 targets, 5053 passed, 2 failed — and the 2 failures are exactly the two pre-existing ones** (`committed_closure_matches_a_fresh_oracle_harvest`, `every_living_awl_document_checks_clean`). **Nothing this diff introduced is red.** This run spans the contamination window (see note above): I ran `battery.sh`'s own coverage control over the log — `targets=271 unpaired=0`, i.e. every started target reached a `test result:` line, so the run was NOT truncated and its tally IS a coverage claim. Zero deletion-shaped failures (`No such file or directory` / `Text file busy` / link / compile) anywhere in the run: **0 hits**. Both reds also landed at log lines 1111 and 1462 — early, pre-window — and both are independently reproduced on the untouched base worktree (rows 8 and 10). No re-run needed: no red in this leg rests on contaminated evidence |
| 8 | base: `cargo test -p aion-awl-package --test sdk_closure_freshness` | 101 | claim (a) |
| 9 | base: `cargo clippy -p aion-server -p aion-cli --all-targets --no-default-features --features libsql-backend` | 101 | claim (b) |
| 10 | base: `cargo test -p aion-cli --test awl_corpus_gate` | 101 | **Identical failure on base**: `every_living_awl_document_checks_clean`, same 3 `workflows/fleet-dev/fleet_dev.awl` agent-return-shape errors (lines 192/197/220), `2 passed; 1 failed`. Row 5's red is PRE-EXISTING, not introduced |
| 11 | `e9e19bcac`: `cargo test -p aion-server --lib parent_run_derives` (in `.worktrees/review-244-base`, detached at the pin, then returned to `7109c1f6d`) | 101 | **RED CONFIRMED, right reason.** `assertion left == right failed / left: 404 / right: 200` at `children_tests.rs:54`. The crate compiled (`1112 filtered out`), so this is a behavioural red — the route did not exist — not a compile red |

One warning that appears in the `cargo test` output and is NOT this lane's:
`linker stderr: ld: __eh_frame section too large (max 16MB) …` on the `aion`
binary (`#[warn(linker_messages)]`, darwin toolchain). I suspected the three new
CLI dependencies (reqwest, tokio-tungstenite, futures) might have pushed it over
the threshold, so I checked base — it appears there too (base run of
`cargo test -p aion-cli --test awl_corpus_gate` at `7109c1f6d`). Environmental
and pre-existing.

Gate legs I did NOT run, and why: the battery's `estate-guard` leg is an estate
assertion about other repositories, not about this diff; `rust:test-all-features`,
the `posture:*` legs, the python/TS SDK legs and `gates:worker-crate` are
untouched by this diff's file set (no proto, no worker, no SDK, no package
feature surface changed) — they belong to Waffles' pre-landing battery, and this
review does not substitute for it.

---

## Contamination ruling (per the dispatcher's 04:28–04:32Z notice)

**Nothing in this review's evidence is contaminated, and no red is attributed to
the deletion or left unattributable.** Reasoning, not assertion:

- Only one gate row overlapped the window: row 7. Every other row finished by
  03:32Z (rows 1, 2, 2b, 5) or ran in the separate `review-244-base` worktree,
  which the `rm -rf` never touched (rows 8, 9, 10, 11, finishing 04:25Z). The
  console rows use no cargo target at all.
- Row 7 nonetheless came back self-consistent: the battery's own truncation
  control (`targets=271 unpaired=0`) proves the run reached the end, and a scan
  for deletion-shaped failures — missing files, busy files, link failures,
  compile aborts — returns **zero hits** across the whole run.
- Its only two reds were emitted early (log lines 1111 and 1462, pre-window) and
  each is independently reproduced on the uncontaminated base worktree. So the
  attribution rests on uncontaminated evidence in every case.
- Disk is now at 16Gi free (99% used), so a precautionary full re-run would have
  been expensive — but it is also unnecessary, because no conclusion here depends
  on the contaminated interval. Recording that explicitly rather than re-running
  for the appearance of rigour.

**The FAIL verdict is entirely independent of this window.** F1/F1b and F2 are
source-level findings proven by reading the two ends of a wire contract and by a
standalone serde probe — no battery evidence is load-bearing for the verdict.

## The two claimed-pre-existing reds

### (a) `aion-awl-package` `sdk_closure_freshness` — PRE-EXISTING CONFIRMED

Reproduced on base `7109c1f6d` in `.worktrees/review-244-base`, exit **101**,
`test result: FAILED. 11 passed; 1 failed`, failing test
`committed_closure_matches_a_fresh_oracle_harvest`, panic at
`crates/aion-awl-package/tests/sdk_closure_freshness.rs:407`:

> committed closure BYTES lag the harvest while the wire surface matches — the
> shipped beams are not what this source and toolchain produce. Run
> `scripts/regen-awl-sdk-closure.sh` and commit the result. Divergent behavioural
> chunks per module: `["aion@agent (AtU8, Code, FunT, ImpT, LitT, LocT)"]`

Identical to the builder's description, on a tree containing none of this lane's
commits. **Not introduced.** Remediation (`scripts/regen-awl-sdk-closure.sh`) is a
repo-baseline chore and correctly listed in the builder's `remaining_work`.

### (a2) A THIRD pre-existing red the builder did not name — `aion-cli` `awl_corpus_gate` — PRE-EXISTING CONFIRMED

Not in the builder's claim list, and it should have been. `cargo test -p aion-cli`
fails at `tests/awl_corpus_gate.rs` —
`every_living_awl_document_checks_clean`, `workflows/fleet-dev/fleet_dev.awl`
lines 192/197/220 (`agent` action returns `Reply`, not the canonical
three-field agent-outcome record). Reproduced on base `7109c1f6d`, exit 101,
byte-identical failure text and the same `2 passed; 1 failed`. **Not introduced.**

Why the builder missed it: their reported battery invocation was
`cargo test -p aion-rs -p aion-awl -p aion-awl-package -p aion-package -p aion-core`,
which does not include `aion-cli` — the crate this lane put a new verb in — and
which additionally has no `--no-fail-fast`, so it stopped at the first red target
(`sdk_closure_freshness`) and reported a truncated run. `scripts/battery.sh`
carries a long comment about exactly this failure mode and mandates
`--no-fail-fast` for that reason. The disclosure was honest but the coverage
claim behind it was narrower than it read.

### (b) slim libsql feature-matrix clippy — PRE-EXISTING CONFIRMED (with a caveat about what it proves)

Reproduced on base `7109c1f6d`, exit **101**. On base the run dies in
`aion-server` (lib) with two pedantic/all errors in files this lane never touches:

- `error: unused `self` argument` — `crates/aion-server/src/state.rs:1907`
  (`const fn cluster_self_node(&self)`), `-D clippy::unused-self` via pedantic.
- `error: this `.filter_map(..)` can be written more simply using `.map(..)`` —
  `crates/aion-server/src/worker/registry.rs:1150`, `-D
  clippy::unnecessary-filter-map`.

`could not compile aion-server (lib)` / `(lib test)` — **so the matrix is red on
base before the compiler ever reaches the CLI crate.** That confirms the class is
pre-existing and not introduced. It also means this matrix currently proves
*nothing* about the new CLI code under that resolution: the builder's phrasing
("the feature implementation itself compiles under this resolution") rests on a
separate focused run, not on this gate. Whoever repairs the matrix (correctly
listed in `remaining_work`) should re-run it against this lane afterwards.
I also ran the same command on the 244 tree (gate row 6, exit 101): the error set
is **byte-identical** — same two errors, same files, same line/column
(`state.rs:1907:32`, `worker/registry.rs:1150:14`) — and `children.rs`, `tail.rs`
and `tail/live.rs` appear **zero** times in the entire run. The lane adds no new
slim-matrix diagnostic up to the point the matrix dies. **Not introduced.**

---

## Files I read

Brief and lane artefacts:
- `/Users/tom/Developer/ablative/stack/aion/gate-logs/244-child-visibility/BRIEF.md`
- `/Users/tom/Developer/ablative/stack/aion/gate-logs/244-child-visibility/envelope.json` (builder's `output`: summary, changes, verification claims, remaining_work)

Every non-test file changed by the diff (full diff + final state):
- `/Users/tom/Developer/ablative/stack/aion/.worktrees/244-child-visibility/crates/aion-server/src/api/http/children.rs`
- `…/crates/aion-server/src/api/http/history.rs` (changed hunk)
- `…/crates/aion-server/src/api/http/mod.rs` (changed hunks)
- `…/crates/aion-server/src/api/http/router.rs` (changed hunks + final tail)
- `…/crates/aion-cli/src/tail.rs`
- `…/crates/aion-cli/src/tail/live.rs`
- `…/crates/aion-cli/src/main.rs` (changed hunks)
- `…/crates/aion-cli/Cargo.toml`, `…/Cargo.lock` (changed hunks)
- `…/apps/aion-ops-console/src/lib/api/children-read.ts`
- `…/apps/aion-ops-console/src/lib/api/index.ts`
- `…/apps/aion-ops-console/src/lib/config/clients.ts`
- `…/apps/aion-ops-console/src/lib/config/index.ts`
- `…/apps/aion-ops-console/src/features/workflow-detail/hooks/useChildren.ts`
- `…/apps/aion-ops-console/src/features/workflow-detail/components/LegsSection.tsx`
- `…/apps/aion-ops-console/src/features/workflow-detail/swimlane/WorkflowDetailView.tsx`

Test files changed by the diff:
- `…/crates/aion-server/src/api/http/children_tests.rs` (at `e9e19bcac` and at HEAD)
- `…/crates/aion-server/src/api/http/router_tests.rs`
- `…/apps/aion-ops-console/src/features/workflow-detail/__tests__/legs.test.tsx`

Surrounding code read to check the diff against reality (not changed by it):
- `…/crates/aion-server/src/api/http/transcripts.rs` (head/`after_seq`/`last` semantics — the CLI's `head_seq.checked_sub(1)` cursor is CORRECT)
- `…/crates/aion-server/src/stream/transcript_stream.rs` (frame encode, splice contract, lag terminal)
- `…/crates/aion-server/src/stream/socket.rs` (`send_wire_error` frame shape)
- `…/crates/aion-server/src/api/ws_subscription.rs` (subscribe-frame decode — the CLI's subscribe payload shape IS accepted)
- `…/crates/aion-proto/src/events.rs` (`StreamedActivityEvent`, `TranscriptSubscription`)
- `…/crates/aion-core/src/status.rs` (`WorkflowStatus`, `status_from_events`, `run_segment`)
- `…/apps/aion-ops-console/src/lib/api/transcript-stream.ts` + `transcript-stream.test.ts` (the corroborating frame parser)
- `…/apps/aion-ops-console/src/features/transcript/hooks/useTranscript.ts`
- `…/apps/aion-ops-console/src/features/transcript/components/TranscriptPanel.tsx`
- `…/apps/aion-ops-console/src/features/workflow-detail/swimlane/swimlaneHooks.ts` (the only `setInterval` in the feature — a pre-existing 1s clock tick, not data polling)
- `/Users/tom/Developer/ablative/stack/aion/docs/operations/API.md`
- `/Users/tom/Developer/ablative/stack/aion/scripts/battery.sh`, `gates.json`, `CONVENTIONS.toml`, `Cargo.toml` (lint config)

---

## What must happen before this lands

1. **F1/F1b** — decode the real transcript frame, make unknown frames loud, treat
   `transcript_lagged` as per-leg recoverable rather than session-fatal.
2. **F2** — write the R3 red first: a test that feeds a genuine
   `StreamedActivityEvent` JSON to the CLI decoder (committed failing), plus an
   e2e that runs the verb against a parent-with-children fixture and asserts leg
   lines appear.
3. **F3, F5, F7, F8** — every quiet path says why it is quiet; no swallowed sends.
4. **F4** — pin the component that actually ships.
5. **F6** — parent-socket fanout discovery in the CLI, matching `dd5073bf2`.
6. **F9, F10, F11** — typed status (no `Debug`, no invented `Pending`), API.md +
   CLI docs, strict field validation.
7. **F13** — gate each child through the per-workflow gate its siblings use, and
   settle in the contract what an unauthorized child looks like.
8. **F15, F16** — the 2000-event cap either announces its truncation or becomes
   a settled argument; the HTTP endpoint derives from `DEFAULT_HTTP_ADDRESS` and
   gets the override story the other connection settings have.
9. **F17** — expose the transcript subscription on `aion-client` and consume it,
   so this wire contract has one implementation and one place to test it. Doing
   this properly is also the cleanest fix for F1.
10. **F12** — settle the read-shape question before this runs against K9-scale
    histories.
11. **F18** — do not grow `main.rs` further; the `Tail` variant and its dispatch
    arm belong beside `tail.rs`.

Nothing on this list is optional and nothing is deferrable — including the two
NOTE-severity items, which are work with a smaller blast radius, not exemptions.

Then re-gate: the full battery, and the two baseline reds re-dated once the
repo-level remediations land.
