# Delivery report — #247 transcript outage spool, LEG 2

## Status

**Not ready to land.** Phase 1 completed successfully, and ratified gaps 3 and 4 are now implemented, committed, and green. Ratified gaps 1 and 2 remain unresolved:

1. Server-authored worker-death gap event.
2. Chartered live server-outage/restart/adopted-survivor e2e.

- Worktree: `/Users/tom/Developer/ablative/stack/aion/.worktrees/247-transcript-spool`
- Branch: `247-transcript-spool`
- Final HEAD: **`568c1f619`**
- Worktree: **clean**
- Pushes: none
- Merges: none
- Live server/worker processes and `~/.aion`: not touched

---

## Phase 1 — rebase record

### Command

```text
git rebase --onto e8c1b28bf 7109c1f6d 247-transcript-spool
```

### Result

```text
Rebasing (1/3)
Rebasing (2/3)
Rebasing (3/3)
Successfully rebased and updated refs/heads/247-transcript-spool.
```

### Conflicts

**None.** No manual conflict resolutions were required.

### Rewritten commits

| Original | Rebased | Subject |
|---|---|---|
| `29863a6ca` | `7e3b6ed7d` | `test(247): transcript outage spool is red` |
| `6fb7d909a` | `d832c45ec` | `fix(247): spool transcript events across outages` |
| `ca9360f2f` | `09d31aac6` | `fix(247): make replay dedup collision-safe` |

New base:

```text
e8c1b28bf refactor(cli): keep replay regression lint-clean
```

### Post-rebase touched-crate verification

Performed before any phase-2 implementation:

```text
cargo test -p aion-worker --features liminal-transport
cargo test -p aion-server --lib activity_publisher
cargo test -p aion-server --lib api::http::transcripts
```

Results:

- `aion-worker`: **168 unit tests passed**
- `grpc_registration`: **4 passed**
- Worker doctest: **1 passed**
- Server activity publisher: **17 passed**
- Server transcript HTTP path: **9 passed**
- Failures: **0**

This confirmed the three rebased commits retained their behavior on `e8c1b28bf`.

---

## Phase 2 commits

### `46a9678ea` — `test(247): pin spool diagnostic loss arms`

Committed both missing worker-side regressions red-first.

Recorded red evidence:

```text
disk exhaustion diagnostic was lost during the continuing outage
```

and:

```text
closed producer suppressed the malformed-entry diagnostic
```

Both focused tests exited with status 101 before the fix.

### `7aba99757` — `fix(247): retain spool loss diagnostics until recorded`

Implements gaps 3 and 4:

- Retains process-owned stream identity evidence beside each spooled event.
- Malformed entries use their own retained identity rather than borrowing a later event.
- A closed producer therefore cannot suppress malformed-entry recording.
- Disk-exhaustion and other spool-loss diagnostics remain pending in memory and retry through the existing reconnect backoff until accepted.
- No fixed-capacity diagnostic buffer was introduced.

### `2801bbcb9` — `style(247): format retained diagnostic paths`

Append-only rustfmt correction after `cargo fmt --all --check` identified two layout differences.

### `568c1f619` — `refactor(247): isolate spool front failure handling`

Strict clippy initially rejected `run_drain_with_spool` at 101 lines:

```text
error: this function has too many lines (101/100)
```

The spool-front error arm was extracted into `record_front_failure`. This retained the behavior while satisfying the lint structurally—no lint allowance or bypass was added.

---

# Gap-by-gap disposition

## Gap 1 — server-authored worker-death gap

**Unresolved; blocker.**

There is still no committed server path that records a typed event containing:

- the four-axis dispatch/stream identity;
- the last accepted event sequence;
- the last accepted timestamp;
- the worker-incarnation death time/window;
- the bounded claim that intervening events are unrecoverable.

I investigated the relevant seams:

- `HeartbeatTracker::fail_lost_worker`
- `HeartbeatTracker::park_lost_worker`
- `LostWorkerReport` / `InFlightActivity`
- `PendingActivities`
- `ActivityCompletionSink`
- `ActivityEventPublisher`

The present in-flight liveness record does not retain `run_id`, so it cannot directly reconstruct the full `ActivityStreamKey`. Correct implementation therefore needs to thread the run axis through dispatch tracking and connect the death detector to an asynchronous durable publisher without making transcript support load-bearing for dispatch.

An incomplete prototype was not committed. All provisional server changes were restored, and the final tree contains no partial server-gap wiring.

### Red/green evidence

None committed. This gap remains open.

---

## Gap 2 — chartered live outage/recovery e2e

**Unresolved; blocker.**

No committed test yet proves the chartered sequence:

1. A real streaming command emits a pre-outage transcript event.
2. A real server is killed mid-stream.
3. The command continues emitting during the outage.
4. A real server restarts.
5. The surviving worker re-registers.
6. The survivor is adopted/re-armed under the new generation token.
7. The outage events drain before post-reconnect live events.
8. The observability read API returns a strictly monotonic stream.

The original `FakeSink` state-machine tests remain and pass, but they are not being represented as the required e2e proof.

The rebased `e8c1b28bf` tree also does not contain a dedicated #246 transcript outage/recovery rig ready for direct extension. The local `246-adoption` branch was inspected read-only; its delta from `e8c1b28bf` concerned responder/sibling-spawn collision handling rather than a reusable transcript server-restart fixture. Nothing was merged or copied from that branch.

### Red/green evidence

None committed. This gap remains open.

---

## Gap 3 — malformed entry must record after producer closure

**Resolved.**

### Red

Test:

```text
malformed_spool_entry_records_after_the_producer_is_closed
```

Pre-fix result:

```text
Error: "closed producer suppressed the malformed-entry diagnostic"
```

Committed in `46a9678ea`.

### Green

`DiskSpool` now holds a process-owned map from spool path to the original `ActivityEvent`. When decoding the on-disk payload fails, `SpoolError::Malformed` carries that event evidence.

The error path therefore authors:

```text
aion/transcript-gap/malformed-entry
```

from the corrupt entry’s own stream identity even when the producer channel has already closed.

Green implementation: `7aba99757`.

Final focused result:

```text
malformed_spool_entry_records_after_the_producer_is_closed ... ok
```

---

## Gap 4 — disk-exhaustion diagnostic must survive disconnection

**Resolved.**

### Red

Test:

```text
disk_exhaustion_diagnostic_survives_a_continuing_outage
```

The fixture keeps the sink unavailable for both:

- the original transcript publish; and
- the initial diagnostic publish.

Pre-fix result:

```text
Error: "disk exhaustion diagnostic was lost during the continuing outage"
```

Committed in `46a9678ea`.

### Green

`record_spool_failure` is now asynchronous and retains its encoded typed diagnostic until `EventSink::publish` succeeds. Failed publishes:

- remain pending;
- use the existing `RedialBackoff`;
- retry after reconnect;
- are never discarded through a dropped `DrainStep`.

Green implementation: `7aba99757`.

Final focused result:

```text
disk_exhaustion_diagnostic_survives_a_continuing_outage ... ok
```

---

# Implementation choices

## Process-owned identity evidence

The malformed-entry solution retains an in-memory `BTreeMap<PathBuf, ActivityEvent>` owned by the same `DiskSpool` incarnation.

Why:

- The worker still knows the exact event identity when it writes the file.
- If the file later becomes malformed, that evidence remains independently available.
- No identity is inferred from the spool filename.
- No unrelated later transcript event is borrowed.
- The evidence is deleted only after the corresponding spool file is successfully removed.
- It introduces no abandoned-directory or cross-incarnation ownership mechanism.

This remains consistent with the v1 cut: the spool and its identity evidence belong only to the current worker process.

## Retry one diagnostic until accepted

A spool-write failure means the event itself cannot be retained on that filesystem. The code therefore holds the resulting typed diagnostic and retries it synchronously until the transport accepts it.

Why:

- There is no second disk device whose successful write could honestly be assumed.
- A best-effort single publish recreates the silent-loss defect.
- No arbitrary diagnostic queue size or byte cap is introduced.
- Backoff uses the existing reconnect configuration rather than inventing a new constant.

## Extracted front-error helper

`record_front_failure` isolates attribution and publication of errors returned by `spool.front()`.

Why:

- It keeps malformed-entry evidence handling together.
- It leaves the main drain loop below the strict clippy line floor.
- It avoids a lint suppression.
- It does not change replay ordering or transport behavior.

---

# Non-goals and floors preserved

The committed work does **not** introduce:

- Durable cross-incarnation spool adoption.
- Reuse or claiming of a prior process’s spool directory.
- PID-only ownership.
- A fixed event-count limit.
- A fixed spool-byte limit.
- A diagnostic-buffer capacity.
- A default operational spool cap.
- A new registration protocol.
- A dispatch dependency on spool support.
- Changes to server ingestion dedup semantics.
- Changes to the four-axis activity stream identity.
- Pushes, merges, or live-estate changes.

Old-worker/new-server and new-worker/old-server protocol behavior was not changed by the leg-2 worker fixes.

---

# Verification actually performed

## Phase-1 post-rebase verification

Passed:

```text
cargo test -p aion-worker --features liminal-transport
cargo test -p aion-server --lib activity_publisher
cargo test -p aion-server --lib api::http::transcripts
```

Counts:

- Worker unit: 168 passed
- Worker registration: 4 passed
- Worker doctest: 1 passed
- Activity publisher: 17 passed
- Transcript HTTP: 9 passed

## Focused red runs

Both failed as required before implementation:

```text
disk_exhaustion_diagnostic_survives_a_continuing_outage
malformed_spool_entry_records_after_the_producer_is_closed
```

Failure messages are quoted above.

## Final focused spool module

Passed:

```text
cargo test -p aion-worker --features liminal-transport runtime::liminal_drain::tests
```

Result:

```text
7 passed; 0 failed
```

This includes:

- outage replay;
- strict ordering;
- mutation control;
- basic disk-exhaustion gap;
- continuing-outage diagnostic retention;
- malformed-entry gap;
- closed-producer malformed-entry gap.

## Final touched-crate suites

Passed:

```text
cargo fmt --all --check
cargo test -p aion-worker --features liminal-transport
cargo test -p aion-server --lib activity_publisher
cargo test -p aion-server --lib api::http::transcripts
```

Final counts:

- Worker unit: **170 passed**
- Worker registration: **4 passed**
- Worker doctest: **1 passed**
- Activity publisher: **17 passed**
- Transcript HTTP: **9 passed**
- Failures: **0**

The macOS oversized `__eh_frame` compact-unwind linker warning appeared on server test links but did not fail them.

## Strict clippy

The first post-implementation clippy attempt found the legitimate 101-line function failure. It was fixed structurally in `568c1f619`.

Then passed twice:

```text
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets -- -D warnings
```

Pass 1: exit 0  
Pass 2: exit 0

## Full workspace battery

Command:

```text
cargo test --workspace --all-targets
```

Result: **exit 101**.

Stopping failure:

```text
worker_shell::tests::cancellation_reaps_the_entire_spawned_process_group
Error: ParseIntError { kind: Empty }
```

State at stop:

```text
285 passed; 1 failed
```

The single stopping test was then rerun exactly:

```text
cargo test -p aion-cli --bin aion \
  worker_shell::tests::cancellation_reaps_the_entire_spawned_process_group \
  -- --exact --nocapture
```

Rerun result:

```text
1 passed; 0 failed
```

The complete workspace battery was **not** rerun after that isolated pass, so the full battery is not claimed green.

## Not performed

- Gap-1 server death-gap tests.
- Gap-2 real outage/restart/adoption e2e.
- Full `aion-server` suite.
- `js-deps` passes.
- A second full workspace battery after the isolated flaky-test pass.
- Any live estate test.
- Any push or merge.

---

# Final history

```text
568c1f619 refactor(247): isolate spool front failure handling
2801bbcb9 style(247): format retained diagnostic paths
7aba99757 fix(247): retain spool loss diagnostics until recorded
46a9678ea test(247): pin spool diagnostic loss arms
09d31aac6 fix(247): make replay dedup collision-safe
d832c45ec fix(247): spool transcript events across outages
7e3b6ed7d test(247): transcript outage spool is red
e8c1b28bf refactor(cli): keep replay regression lint-clean
```

## Final disposition

The branch now truthfully resolves the two worker-side no-silent-loss defects identified by the leg-1 audit, and it is clean at `568c1f619`. However, the server-authored worker-death gap and the required real outage/restart/adopted-survivor proof remain absent.

**Do not merge or present this branch as satisfying the ratified #247 brief.**