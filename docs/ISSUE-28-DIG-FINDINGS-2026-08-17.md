# Issue #28 dig findings — 2026-08-17

## Status

The reported `due_diligence.awl` was not available to this lane at the time of
this report. The reporter corrected the candidate identification on the issue:
the reproducer is **not** either market-mirror workflow, and its literal
`filter`/`count` step is in `summarise`. A redacted document and a purpose-made
fresh-server log were requested in issue comment `5311771758`; neither had
arrived by 04:40Z.

Accordingly this is the charter's **not reproduced** branch. No mechanism fix
was attempted. The gated instruments required by the charter landed, and the
two explicitly named in-estate candidates were exercised only as candidates,
not represented as the reporter's reproducer.

## Isolation and protocol

All live work used one builder-owned server and two builder-owned AWL workers.
The server used an explicit `AION_HOME` under this worktree's ignored `target/`
tree, the in-memory store, and loopback-only addresses:

- HTTP: `127.0.0.1:62101`
- gRPC: `127.0.0.1:62102`
- reserved but unused outbox port: `127.0.0.1:62103`
- server version: `0.16.0+be64038bc4aec7f4dd4be789150f1ce15cfc7ad8-dirty`

The candidate worker documents preserved the declared action contracts but
served deterministic local Ruby commands. They performed no network access and
wrote no estate state. The live estate server, `~/.aion`, repository-root
`aion-data/`, live workers, the libsql sibling worktree, and every named
forbidden artifact were untouched.

## Reproduction results

### `mm_capture_run.awl`

- Eight consecutive runs on the same fresh server.
- `poll_cycles = 1`, so each run exercised its first remote activity and the
  conditional completion leg without an authored pace delay.
- Result: **8/8 green**.
- Exact reported face `await_activity_result: invalid correlation id`: **0**.
- `activity completion delivery failed`: **0**.
- `process N is not live`: **0**.

### `mm_watched_run.awl`

- Eight consecutive runs on the same server immediately after the capture set.
- Each run exercised `check_freeze` and `watch_once`, including the custom
  structs and `when`/`otherwise` control flow.
- Result: **8/8 green**.
- Exact reported face `await_activity_result: invalid correlation id`: **0**.
- `activity completion delivery failed`: **0**.
- `process N is not live`: **0**.

Across all sixteen candidate runs, workflow PIDs advanced monotonically from 1
through 16. No PID was reused. Correlations were `activity:0` for every first
activity and `activity:1` for the watched workflow's second activity.

These runs do **not** clear the defect: the reporter explicitly ruled both
documents out, and the local workers did not reproduce the reporter's retrying
activity failure.

## Instrumentation

The instrumentation is dormant at the default `info` filter and uses the
existing tracing configuration at `debug` level. Each event records:

- boundary (`dispatch_return_live`, `dispatch_return_recorded`, in-VM variants,
  `await_argument`, and the post-resolution await outcome);
- workflow PID, workflow ID, and run ID;
- correlation text and its byte vector;
- `retained_activity_completions` at that boundary.

The sixteen candidate runs emitted 24 dispatch-return and 48 await-argument
events before the post-resolution extension was added. Every dispatch/await
pair carried identical ASCII bytes:

- `activity:0` → `[97, 99, 116, 105, 118, 105, 116, 121, 58, 48]`
- `activity:1` → `[97, 99, 116, 105, 118, 105, 116, 121, 58, 49]`

The first await observation occurred before delivery with retained count 0;
the wake re-entry observed count 1; the activity then completed green. After
rebuilding, one further run on a second fresh isolated home emitted
`await_completed` with the same bytes and `retained_activity_completions = 0`,
proving the post-resolution event exposes the consumed state.

## Hypothesis ranking after source audit and candidate instruments

### H1 — term/heap corruption at the beamr boundary

**Still open for the real reproducer; unsupported by the candidate runs.**

The candidates showed byte-for-byte identity between Rust's dispatch return and
the decoded await argument through sixteen workflows and 24 activities. That
is evidence that the ordinary term boundary is healthy for these two documents,
not evidence against a document-shape-dependent fault in `due_diligence.awl`.
The exact reproducer is required to settle H1.

### H2 — stale PID-keyed state adopted by a recycled PID

**Ordinary within-VM PID recycling is falsified by current beamr source.**

beamr 0.18.2 keeps `next_pid` in scheduler-local shared state, initializes it to
1, allocates with monotonic `fetch_add`, and does not return exited PIDs to a
free list. Aion also documents this invariant in `runtime/nif_state.rs`. The
only theoretical same-scheduler reuse is `u64` wraparound. The candidate server
confirmed monotonic PIDs 1–16.

A stale retained completion remains a defect worth detecting and draining, but
it cannot alias a later workflow through normal same-scheduler PID reuse. The
reporter's PIDs 6–9 came from a log spanning multiple restarts, so their repeated
numbers do not contradict this result.

### H3 — per-VM adjacency / cross-VM global maps

**Falsified for the normal engine topology at this HEAD.**

One `Engine` constructs one `RuntimeHandle`, which constructs one beamr
`Scheduler`; workflow starts and recovery clone that same handle rather than
selecting among a local VM pool. Activity maps are fields of that runtime
handle, and `EngineNifState` is installed as scheduler-private data. Separate
schedulers can both mint small PIDs, but their maps are separate. Historical
comments state that process-wide NIF globals previously caused cross-engine PID
cross-wiring and were replaced by this per-engine state.

The issue's observed boundary at the sixth workflow therefore has no current
source counterpart in a six-VM idle pool.

## Interim and non-goals

Until a fix based on the real reproducer lands, any operational flight that
runs one document more than five times should document a server restart between
re-flies. This is an explicit interim, never a silent recovery behavior.

This lane did not:

- invent or land a speculative mechanism fix;
- claim either market-mirror candidate as the reporter's workflow;
- exercise the reporter's retry failure path without the real document and
  worker behavior;
- change correlation identity, retained-map keying, beamr PID allocation, or
  gate-outcome code;
- treat the correlation machinery and the gate-outcome seam as neighbours.

## Verification performed

- `aion awl check` on both candidate workflows and both isolated worker docs.
- Eight live runs of each candidate on one isolated server: 16/16 green.
- Log measurement for exact invalid-correlation, delivery-failure, and dead-PID
  faces: all zero.
- Instrumented byte comparison across 24 activities: no mismatch.
- One rebuilt live run on a second fresh home: `await_completed` reported the
  matching bytes and `retained_activity_completions = 0`.
- `cargo test -p aion-rs runtime::nif_activity --lib`: 52 passed, 0 failed,
  0 ignored.
- `cargo fmt --all -- --check` and `git diff --check`.

No broader test suite, soak against `due_diligence.awl`, or fix acceptance run is
claimed.
