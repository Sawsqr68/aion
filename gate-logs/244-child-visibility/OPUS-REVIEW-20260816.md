# LANDING REVIEW — aion board item #244 (child-visibility slice)

Reviewer: Opus landing-review subagent, 2026-08-16, commissioned by Waffles.
Worktree: `/Users/tom/Developer/ablative/stack/aion/.worktrees/244-child-visibility`
Range reviewed: `051b547a6..b0214c0ef` (11 commits, 34 files, +3791/−1034)

## VERDICT: DO-NOT-LAND

Six blockers. The server projection is the strongest part of the slice — well-documented, correctly gated, honestly tested. The two consumer surfaces (CLI tail and ops-console) both contain silent-failure paths that make the slice's headline claim — *you can see your child legs* — quietly untrue in ordinary conditions, and the CLI ships a visible output defect.

Waffles' verification note: B1 and B6 spot-checked at bytes by my own hand post-review (tail.rs `Ok(Value::Null)` + main.rs print_json pipe; children.rs `map_or(&[][..])` on unvalidated run_id) — both confirmed. Verdict accepted.

---

## What is done well

The server route is genuinely good work. `crates/aion-server/src/api/http/children.rs:1-26` carries a real authorization-decision and read-shape rationale rather than a description of the code; the anti-leak omission is deliberate, argued, and pinned by a test. `children_tests.rs` asserts whole-body JSON equality rather than field pokes, so a projection regression cannot slip past. The two RED-pin commits (`ebb0d6b70`, `24d3f4f10`) carry real captured failure output, meeting the red-evidence standard. `tail_follow_e2e.rs` drives the actual shipping binary over real gRPC + WebSocket fixtures with `Notify`-based ordering rather than sleeps — it would fail if post-attach fanout discovery broke. The two large test extractions are faithful: the `fn` inventory of `ws.rs` before equals that of `ws.rs` + `ws_socket_tests.rs` + `ws_unit_tests.rs` after, exactly; `router.rs` likewise, gaining only the new `dev_router` helper. No test was quietly dropped under cover of a refactor.

---

## BLOCKERS

### B1 — `aion tail` prints a bare `null` to stdout on every exit
`crates/aion-cli/src/tail.rs:135` and `:150` return `Ok(Value::Null)`; `crates/aion-cli/src/main.rs:833-835` pipes every `Command::Client` return value through `print_json`. So a normal `aion tail <id>` session ends with human transcript lines followed by a lone `null` line. It also breaks the CLI's stated one-rendering-contract (JSON on stdout): tail writes human prose to stdout via `println!` (`tail.rs:200,267,283,291`, `tail/live.rs:43`) *and* a JSON `null`. The e2e never catches this because it `kill()`s the process before exit and greps for a substring. Either give `tail` a real terminal report (targets followed, events shown, legs closed) or exempt it from `print_json` the way `Command::Awl` is exempted at `main.rs:769-771` — the precedent for a subcommand owning its own reporting contract already exists in this file.

### B2 — Console tells the operator "No activity transcript stream recorded" when the read actually failed
`apps/aion-ops-console/src/features/workflow-detail/components/LegsSection.tsx:156-158,193-195`. `LiveLeg` consumes only `history.isSuccess` and `history.data`. It never reads `history.isError`, and `useLiveWorkflowEvents` is gated off whenever history is not successful. On a failed or still-loading child-history read, `attempts` is empty, `displayed.length === 0`, and the leg renders the flat assertion *"No activity transcript stream recorded."* That is a false authoritative statement produced by an error path — precisely the silent failure the house rules forbid, on the one surface whose entire job is honest visibility. The parent view already models this correctly (`WorkflowDetailView.tsx:74-81` renders a `role="alert"` for `children.isError`); the per-leg reads need the same channel plus a distinct loading state.

### B3 — A child that starts its run after page load never goes live in the console
`useChildren.ts:31` keys the query on `discoveryRevision`, and `WorkflowDetailView.tsx:56` supplies `latestChildStartSequence(live.events)` — the seq of the parent's most recent `ChildWorkflowStarted`. When a *pending* child (server returns `run_id: null`) subsequently starts its own run, the parent emits no new event, so the revision does not change, no refetch occurs, and `refetchOnWindowFocus` is `false` globally (`src/app/providers.tsx:80`). `LiveLeg` gates its history on `leg.runId !== null` (`LegsSection.tsx:156`), so the card is frozen at *"Child start recorded; run not started yet."* until a full remount. The CLI solves exactly this case deliberately, with `monitor_pending` (`tail/live.rs:156-220`) subscribing to the child's own socket to catch its `WorkflowStarted`. The console has no equivalent. The slice ships the case on one surface and not the other.

### B4 — CLI attach race drops legs and children with no notice
`tail.rs:129-150` performs discovery, stream enumeration, and per-target backfill over HTTP, and only *then* builds the stream client and attaches sockets. `tail/live.rs:86` uses `client.subscribe_workflow(...)`, which is live-only — confirmed at `crates/aion-client/src/ops.rs:396-406`, with the cursor-bearing variant `subscribe_workflow_from` sitting right beside it at `:415`. Any `ActivityStarted` or `ChildWorkflowStarted` recorded in the window between the HTTP reads and the socket attach is therefore never observed by either path, and the affected leg simply never appears. With per-target sequential backfill (`tail.rs:250-280`) that window scales with fanout. There is no notice and no comment acknowledging it.

Fix shape: `observe_attempt` (`tail/live.rs:237-240`) is idempotent against the `current` map seeded from the HTTP enumeration, so replaying from the beginning would self-deduplicate. The one obstacle is the continued-as-new guard at `tail/live.rs:102-114`, which would trip on a replayed earlier `WorkflowStarted` — that guard needs to compare against the selected run rather than bail on first mismatch.

### B5 — Backfill truncation notice states something untrue
`tail.rs:265-272` fires when `events.len() < head_seq` and attributes the shortfall to *"the server's published transcript window limit."* But `head_seq` is defined at `crates/aion-server/src/api/http/transcripts.rs:80-81` as *"Stream head at read time: the next durable `store_seq`"* — one past the last record's seq, **not** a retained-record count. When retention has trimmed the front of a stream, `events.len() < head_seq` holds with no window limit involved, and the CLI reports a retention loss as a paging limit. The message also renders `head_seq` as a total event count in "the last N of M events", which is wrong for any trimmed stream.

Compounding: `last` is **not** clamped by `MAX_TRANSCRIPT_WINDOW_LIMIT` at all. `transcripts.rs:167-175` applies `last` unbounded; only `limit` is clamped (`:176-178`). So the CLI's use of the constant at `tail.rs:260` borrows a value whose premise is a different parameter, and `crates/aion-cli/README.md` ("Retained reads use the server's published 2000-event transcript window") states a server guarantee that the server does not make. Either clamp `last` server-side, or stop describing the CLI's own choice as a published server window. `docs/operations/API.md:66` has the same gap — it documents the clamp on `limit` while leaving `last` silently unbounded.

### B6 — An unknown `run_id` answers `200 {"children": []}`
`children.rs:96-104`: a caller-supplied `run_id` is passed straight to `run_segment` without existence validation, and `crates/aion-core/src/status.rs:174-179` returns `&[]` for a run it cannot find. An operator who mistypes a run id — or addresses a run that was never recorded — is told, with a success status, that the run has no children. That is indistinguishable from the truthful empty answer and is a silent wrong answer to invalid input. Sibling routes on this surface answer `not_found` for an absent workflow; an absent run deserves the same.

---

## IMPORTANT

**I1 — The core run-scoping behaviour is not pinned.** Every fixture in `children_tests.rs` seeds exactly one parent run (`seed_parent`, `:178-198`). The "latest run resolution" assertion at `:72-83` is vacuous — with one run, explicit and implicit selection cannot differ — and replacing `run_segment(&parent_history, run)` at `children.rs:104` with a whole-history scan would leave all three tests green. A two-generation parent whose runs have *different* children is the missing fixture, and it is the test that makes the word "run" in this API mean anything.

**I2 — `legs.test.tsx` pins a component nothing ships.** `LegsSection` and its `LegView` type (`LegsSection.tsx:20,23`) are exported from production source and rendered by exactly one caller: the test at `legs.test.tsx:37`. The application renders `LiveLegsSection` (`WorkflowDetailView.tsx:72`). The test does exercise the shared `LegCard`/`LegHeader`/`AttemptHeading` presentation, so it is not a pure husk — but it pins none of the wiring (`useChildren`, `attemptsFromRows`, the `fallback` path at `LegsSection.tsx:165-175`, `liveStatus`), and it leaves a dead exported component in shipped source purely as a test scaffold. Test the shipped container with a stubbed reader; delete the fixture wrapper.

**I3 — Transient child invisibility is conflated with authorization denial.** `children.rs:133-140` treats `NotFound` and `NamespaceDenied` identically as silent omission. Children inherit the parent's namespace at spawn (`crates/aion/src/runtime/nif_child.rs:212`), so a genuine cross-namespace denial should be rare in production; the realistic production trigger for `NotFound` on a child the parent's own durable history just named is **attribution lag**, not denial. That means an authorized, same-namespace child can be silently dropped from the list — permanently for `aion tail`, which reads children exactly once. The test that covers this path (`children_tests.rs:88-127`) achieves the condition by not registering the child in `StaticWorkflowNamespaces`, which `crates/aion-server/src/namespace/resolver.rs:330-332` documents as a *fixture* for tests — so the test proves the route handles the wire outcome, not that the production trigger is what the doc says it is. The anti-leak posture is defensible; the two conditions need to be distinguished internally (log the `NotFound` case at minimum), and `children.rs:6-13` should state plainly that a lagging attribution is indistinguishable from a denial.

**I4 — Sequential N+1 whole-history reads, re-run per new child.** `children.rs:107-153` loops children serially, awaiting a namespace-guard `describe` scope and a full `read_history` for each. The module doc at `:16-25` argues the whole-history read shape well, but says nothing about serialization. Worse, `useChildren`'s `discoveryRevision` re-runs the entire projection on *every* new `ChildWorkflowStarted`, so a workflow fanning out to N legs triggers N successive re-projections, each reading O(N) full histories — O(N²) history bytes for one page. The doc's cost argument needs to cover this, and the loop should at minimum be concurrent.

**I5 — Unbounded, un-delayed reconnect loop on repeated lag.** `tail/live.rs:288-361`: a `Lagged` item sets `reconnect = true` and loops immediately with no backoff and no attempt ceiling. A leg that persistently outruns the broadcast buffer produces a tight reconnect-storm against the server. The lag-recovery design is right; the loop needs a delay.

**I6 — Untested wire-request encoding on both consumers.** The server declares `#[serde(deny_unknown_fields)]` on `ChildrenRequest` (`children.rs:41`), so a single misspelled key is a hard 400. Nothing on either consumer asserts the emitted body: `children-read.test.ts` stubs `fetchImpl` and ignores the request entirely (`:14-19`), and the e2e's `children` handler takes no argument (`tail_follow_e2e.rs:235-237`) — note that `transcripts` at `:239` does bind its request and then immediately `drop`s it, which is the shape of an assertion someone meant to write.

**I7 — Backfill/interleave path has no coverage at all.** The e2e's fixtures return `{"children": []}`, `{"streams": []}`, and `head_seq: 0`. So `discover_sources`' display-name-else-workflow-type fallback (`tail.rs:185`), the pending-vs-started split (`:187-205`), timestamp-ordered interleaving (`:281`), `mark_superseded` against real data, and the B5 truncation notice are all unexercised. `mark_superseded` has a unit test; the rest has nothing.

**I8 — `'Paused'` added to `WORKFLOW_STATUSES` with no test.** `client-normalize.ts:176`. A real latent bug fix — before it, `requireWorkflowStatus` threw for any paused workflow, affecting every console read that normalizes a status, not just children. Landing a cross-cutting behaviour change inside a child-visibility slice with no pinning test and no mention in any commit body is exactly how a fix gets silently reverted later.

---

## LOWER SEVERITY (still requires action — nothing here is optional)

- `tail.rs:306,309,314` — three `HeaderValue::from_str(...)?` with no `.context()`. All three surface the same opaque `"failed to parse header value"`, naming neither the header nor the offending value.
- `tail/live.rs:22` — `mpsc::channel::<String>(256)`. Backpressure semantics are correct (the send awaits, nothing is lost), but 256 is an undiscussed magic constant in a codebase whose rules call out assumed defaults by name.
- `LegsSection.tsx:232` — `Number(row.activityId)` where `AttemptNavigatorRow.activityId` is typed `string` (`attemptNavigator.ts:52`). A non-numeric id yields `NaN` silently, rendering "Activity NaN" and constructing a transcript target with a `NaN` coordinate. The same file's sibling module validates with `Number.isSafeInteger` (`children-read.ts:102`); be consistent.
- `LegsSection.tsx:245-260` — `liveStatus`'s `default:` branch silently returns the stale REST status for an unrecognized outcome. `outcome` is typed `string`, not a union, so this will absorb any future terminal outcome without a sound.
- `LegsSection.tsx:184-192` — a `TranscriptPanel` (and thus a live subscription) is opened for *every* attempt including superseded ones, not just the current. A leg with ten retries opens ten sockets per leg.
- `useChildren.ts:32-35` — `signal.throwIfAborted()` is called but the `signal` is never threaded into `listChildren`/`fetch`. The query is not actually cancellable; the check reads as though it were.
- `children-read.ts:96` — `nullableString` rejects `""`. A workflow renamed to an empty display name would throw an `ApiError` and take down the whole legs panel rather than falling back to `workflowType`.
- `tail_follow_e2e.rs:344,357` — `.unwrap_or_default()` silently substitutes the Unix epoch, while `:370` and `:385` in the same file use `.ok_or(...)?` for the identical concern.
- `tail_follow_e2e.rs:271,278,293,303,306` — `let Ok(..) else { return; }` in socket fixtures. These cannot cause a false pass, but an encode failure surfaces as *"tail did not attach its parent workflow socket"*, which will send whoever debugs it to the wrong file.
- `children.rs:230-263` — the `current_attempt` unit test covers only `ActivityCompleted`. `ActivityFailed`, `ActivityCancelled`, and the mismatched-attempt guard at `:206-208` are untested. The duplicate-child skip at `:116-121` and the error-propagation arm at `:141` are untested too.
- `crates/aion-server/src/api/http/mod.rs:3-31` — the module-layout doc comment enumerates the surface but omits the newly added `children`.

---

## Scope discipline

Everything in the diff serves the slice **except** the test extractions, which are 977 of the 3791 added lines: `router_tests.rs` (330), `ws_socket_tests.rs` (345), `ws_unit_tests.rs` (316). Verified faithful, so the risk is low — but the 500-LOC rule counts code excluding tests and comments, and neither `ws.rs` (1005 total, ~350 code) nor `router.rs` (733 total) was actually in violation. The extractions were not required, and commit `de0b0b95b` justifies them with a bare subject line and an empty body. Either restore the justification to the commit message or drop them from this landing.

Two changes look out of scope but are legitimately load-bearing and should stay: `DEFAULT_HTTP_ADDRESS`/`MAX_TRANSCRIPT_WINDOW_LIMIT` promoted to `pub` (`config/defaults.rs`) for the CLI default, and `scoped_engine` widened to `pub(super)` (`history.rs:148`). The `docs/operations/API.md` corrections to the two transcript rows are accurate against `transcripts.rs` — except for the unbounded `last`, per B5.

## Commit chain

Honest in shape, uneven in substance. The two RED commits carry captured failure output — standard met properly. Six of eleven commits have empty bodies. The head commit `b0214c0ef` is a mega-commit bundling the client contract move, strict decoding, lag recovery, fanout discovery, pending-child following, continued-as-new announcements, truncation notices, endpoint defaults, clap-arg relocation, typed nullable status, per-child gating, docs, and the console leg card — it resolves both RED pins plus unrelated refactors in one blob.

Traceability note: `ebb0d6b70` pins `tail::live::tests::decodes_the_server_streamed_activity_event_frame` red, but at HEAD that decoder lives in `aion-client` as `transcript_ws::decode_frame` with a differently-named test (`transcript_ws.rs:198`). The relocation is right architecturally; the head commit body should say so, since the named red test no longer exists at the pinned address.

The head commit claims the console ships "full transcript error channels." The `TranscriptPanel` channel is pre-existing; the per-leg history/live error channel is absent — that is B2. Do not land a commit message that claims coverage the code does not have.

## Gate-log verification

Both logs re-read by the reviewer rather than taken on trust; they say what was reported. `rebased-battery-take4.log`: `rust:fmt` 0 (line 5), `rust:clippy` 0 (519), `rust:test` 0 (7862), `rust:test-all-features` 101 (14992) with `coverage control ok: 272 targets started, 272 reached a result` and sole failing target `-p aion-store-haematite --lib`; posture legs 4/4. `take5-tail.log`: `adjudication:haematite-solo` exit 0 (line 202); manifest 15/16 with `gates:costly-announced` exit 1 — reviewer records the interleaving-race explanation as plausible and unrelated (slice touches no scripts/, no timer/failover code), not verified by the reviewer; the 3-run mechanism proof is Waffles', banked in the day file.
