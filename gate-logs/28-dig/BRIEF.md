# aion#28 dig brief — five-run wedge, `invalid correlation id`

Drafted by Vesper 2026-08-17 (read-only prep while the libsql lane holds at
Waffles' desk). Status: RIG COMPLETE, PARKED — fires on a freed run slot per
Waffles' two-heavy-runs box law (02:18Z), sequencing vs the K9 rig at his
word. Nothing here is proven mechanism; sections 3–4 are code facts measured
at HEAD, section 5 is ranked hypothesis.

## DISPATCH-TIME AMENDMENTS (Vesper, 2026-08-17 ~02:2xZ)

**A — reproducing document status.** The reporter (annabelgray89, issue
2026-08-12) never posted the offered document; requested on-issue 2026-08-17:
https://github.com/ablative-io/aion/issues/28#issuecomment-5311042605.
In-estate candidates measured at my hand: market-mirror's
`mm_capture_run.awl` and `mm_watched_run.awl`
(/Users/tom/Developer/ablative/prototyping/market-mirror/workflows/) — both
match custom struct types + `when`/`otherwise`; NEITHER carries a literal
`filter`/`count` step (comments and field names only), so candidate ≠
confirmed. Artemis' corpse artifact
stack/frame/artemis-artifacts/2026-08-12-aion-jit-corpse/ preserves an
IDENTICAL mm_capture_run.awl from the issue's filing date — temporal
adjacency, not proof. Dig plan step 1 becomes: try both candidates at the
issue's own protocol (6+ runs, fresh isolated server, gRPC start); if
neither wedges and the reporter has not answered, the instrumentation legs
(H1–H3) still proceed — a STOP-REPORT on the reproduction leg alone is a
pass, not a failure.

**B — isolation walls (hard).** The builder never touches the live estate
server (pid 74635, port 8080), `~/.aion`, `aion-data/`, or any live worker.
All reproduction runs on the builder's OWN server instance with its own
`AION_HOME` inside the worktree, on ports it allocates. The sibling lane
worktree `.worktrees/libsql-retirement` and its branch are another lane in
flight — never read from, written to, or rebased against. Never edits
gate-logs/battery-0150*/, 85-double-close-proof/, .aion-*-20260731/, or
MARKER-GUIDE-EXAMPLES-LANE.md.

**C — acceptance split.** IF reproduced: mechanism named at the exact
site(s), red-first fix, acceptance = the issue's own ceiling (N runs of the
reproducing document on one server, all green, N well past six). IF NOT
reproduced: the deliverable is the instrumentation landed gated (dispatch
return bytes vs await argument bytes logged; `retained_activity_completions`
surfaced) plus an honest findings report ranking H1–H3 against what the
instruments showed — never a speculative fix. The restart-between-re-flies
crutch stays a documented interim either way, never silent.

## 1. The defect (issue #28, specimen 0.14.1)

Five runs of one real workflow complete; the sixth fails within milliseconds
at its FIRST activity with `{error, <<"await_activity_result: invalid
correlation id">>}`, and every later run of ANY workflow fails the same way
until restart. Same server pid throughout — no crash involved. The wedged
state is preceded by exactly one WARN: `activity completion delivery failed
… "runtime error: process 6 is not live" … correlation_id: "activity:0"`.
No synthetic probe reproduces it (run count, fan-out, volume, slow, payload,
two-step all cleared); the real document — custom struct types, `when`/
`otherwise`, `filter`/`count` — reproduces every time.

## 2. The triage read (goes in the dispatch brief verbatim — Waffles' word)

> Two observations from the issue worth carrying into the dig: correlation
> ids are per-workflow indices ("activity:0"), and the wedged state is
> preceded by exactly one undelivered completion ("process 6 is not live")
> — a leaked/colliding correlation-table entry is the shape. Their
> synthetic probes all cleared; the real document has when/otherwise
> conditionals and filter/count, so the trigger likely lives in a leg that
> leaves a completion undelivered. Operational crutch for the flight if it
> lands before the fix: restart the server between re-flies — I'd rather we
> fix it.

The crutch is ACCEPTED as a documented interim (Waffles' ruling ZU-1f),
never silent; the mailbox re-fly is run ≤3 of 5 at each flight, inside the
ceiling, so the flight itself needs no crutch.

## 3. Code map (measured at HEAD, all read-only)

The correlation string's whole life:

- **Minted**: `crates/aion/src/runtime/nif_activity.rs:302` —
  `correlation_id(ordinal)` = `"activity:{ordinal}"`; ordinals are
  per-workflow (`context.next_activity_ordinal()`), so **`activity:0` recurs
  in every run**. Cross-run uniqueness rests ENTIRELY on the pid key beside
  it.
- **Returned**: `nif_activity_dispatch.rs:245/335` — dispatch returns
  `{ok, correlation}` as a fresh binary term
  (`ok_result_term(ctx, correlation.as_bytes())`). The error arm returns
  `{error, reason}`.
- **Threaded**: `gleam/aion_flow/src/aion/internal/activity_dispatch.gleam`
  — `run()` = `result.try(dispatch) |> await`. The error arm is properly
  checked (`parse_error`), so **the string handed to await can only be the
  binary dispatch itself returned**. AWL documents lower onto this same
  glue (`RuntimeFn::WfRun` → `aion@workflow.run`) — no separate AWL path.
- **Parsed**: `nif_activity.rs:306-315` `activity_id_from_correlation` —
  `strip_prefix("activity:")` fails ⇒ the EXACT reported face. This is the
  **only emitter** of that literal, at HEAD and at v0.14.1 (verified:
  `git show v0.14.1:…nif_activity.rs` lines 312/319; face introduced by
  the two-phase split, `git log -S`).
- **Delivery (phase 1)**: `runtime/handle/delivery.rs:208` — completion
  retained in `activity_results` keyed **`(workflow_pid, ordinal)`**, then
  an atom marker wakes the pid. The WARN in the issue is delivery.rs's
  caller `nif_activity_retry_dispatch.rs:48` when the pid is no longer
  live.
- **Take (phase 2)**: `nif_activity_await.rs:175-212`
  `take_runtime_completion` → `runtime.take_activity_result(pid, ordinal)`
  — resolved by the same `(pid, ordinal)` key.
- Diagnostic that already exists: `retained_activity_completions()`
  (delivery.rs:470) — "an engine with no live awaits should report zero."

## 4. The structural facts the dig must weigh

1. **Both key components are small reused integers.** beamr pids count up
   from a small base per VM; ordinals restart at 0 per run. Nothing in the
   key namespace distinguishes run N's `(pid 6, ordinal 0)` from a later
   run that lands on a recycled pid 6.
2. **The face requires a malformed STRING, not a missing entry.** A stale
   or missing map entry cannot produce "invalid correlation id" — that
   face means the workflow passed a non-`activity:`-prefixed string to
   await, i.e. **dispatch's `{ok, Binary}` decoded as something else** by
   the time await read it. The glue cannot substitute the string.
3. **The wedge is engine-wide and permanent** — every subsequent run of
   any workflow fails instantly. That indicts shared VM state (atom table,
   binary heaps, scheduler/process-table), not per-run state.
4. Known neighbours on these exact paths: task #17 (non-atomic
   liveness→insert→enqueue retains unreachable payloads in these maps),
   #196 (beamr badarg deaths — another corrupted-term face), #227 (six
   beamr VMs in one idle server; the wedge boundary is the SIXTH workflow
   process — numerical adjacency, unproven), CORE-007/008 (scheduler
   starvation / wedged-actor recycle).

## 5. Ranked hypotheses (none proven)

- **H1 — term/heap corruption at the beamr boundary after N workflow
  processes.** Face-exact: dispatch returns a good `{ok, <<"activity:0">>}`
  but the binary decodes as garbage in await's `decode_string_arg` /
  prefix check. Explains engine-wide permanence (shared heap/atom state),
  the synthetic-probe immunity (the real document's `when`/`otherwise` +
  `filter`/`count` + custom structs allocate different term shapes), and
  rhymes with #196's badarg class. Dig: log the dispatch-returned bytes
  AND the await-received bytes on every call; wedge a specimen and diff.
- **H2 — stale pid-keyed state adopted by a recycled pid.** The
  `(pid, ordinal)` maps, `activity_errors`, spawn heaps, pending-query
  sentinels (`take_pending_query_sentinel(state, pid)`) are all keyed by
  bare pid. The issue's WARN proves an undelivered completion existed for
  a dead pid at the wedge boundary; #17 proves entries can be retained
  unreachable. A recycled pid inheriting a stale entry produces
  WRONG-RESULT delivery (coinciding-answers class: run N+1's `activity:0`
  takes run N's payload) — a silent-corruption defect in its own right
  even if it is not the reported face. Dig: `retained_activity_completions`
  before/after each run; beamr pid-reuse policy at the source.
- **H3 — per-VM adjacency (#227).** Six VMs at idle; wedge at the sixth
  workflow process. If workflow spawns land on a VM whose process table or
  pid allocator collides with engine-held pid keys minted against another
  VM, keys alias across VMs. Dig: which VM does each run's process spawn
  on; are the handle's pid-keyed maps VM-scoped or global.

## 6. Non-neighbour ruling input (ZU-1g's open question)

Waffles asked: if the #28 dig shows the correlation machinery and the
gate-outcome seam (#34+#31) are NEIGHBOURS, say so — he'd reconsider the
lane split. Measured answer: **not neighbours.** The correlation machinery
lives in `crates/aion/src/runtime/*` + `durability/correlation.rs` (engine
crate, pid/ordinal keyed, replay seam); the gate-outcome seam is
`workflows/repo_gates` + the `aion-integrations` failure taxonomy
(worker-side outcome classification). No shared files, no shared types
beyond `ActivityError` at the wire. The split stands.

## 7. Dig plan (for the dispatched lane)

1. Obtain the real reproducing document from the issue reporter (offered).
2. Reproduce at HEAD; capture `~/.aion/server.log`, and
   `retained_activity_completions` per run (add a debug read if no surface
   exposes it — that is the lane's first commit, gated).
3. Instrument both ends of the correlation string (H1) — dispatch return
   bytes vs await argument bytes, logged with pid + run.
4. Settle pid-reuse policy at beamr's source (H2/H3) and whether the
   handle's maps are VM-scoped (H3).
5. Whatever mechanism falls out: red-first fix; the acceptance case is the
   issue's own five-run ceiling — N runs of the real document on one
   server, all green, N well past six.
6. Until the fix lands: restart-between-re-flies documented interim
   (never silent) for any flight that would run one document >5 times.

## DISPATCH-TIME AMENDMENT D (2026-08-17 04:4x, post-dispatch, reporter's correction)
The candidate hypothesis (mm_capture_run.awl / mm_watched_run.awl) is RETIRED by
the reporter's on-issue correction (annabelgray89, 03:46Z): the reproducing
document is THEIR `due_diligence.awl` (property due-diligence prototype) with a
literal `filter/count` summarise step. Server-side face: 15x "activity completion
delivery failed / process N is not live", workflow_pid 6-9 reused, correlation_id
activity:0, emitter nif_activity_retry_dispatch (retry path). Redacted document +
purpose-made clean capture requested on-issue (04:23:38Z reply, authored by this
lane's own agent). The dig agent read the thread live and holds this knowledge;
this amendment keeps the paper record aligned.
