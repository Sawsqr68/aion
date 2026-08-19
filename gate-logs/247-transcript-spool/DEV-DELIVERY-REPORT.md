# Delivery report — #247 transcript outage spool

## Status

**Not ready to land.** The branch is clean and contains the red-first and implementation commits, but my audit found four ratified requirements that the current implementation does not satisfy. The full workspace battery also stopped on an unrelated SDK-closure freshness failure.

- Worktree: `/Users/tom/Developer/ablative/stack/aion/.worktrees/247-transcript-spool`
- Branch: `247-transcript-spool`
- Final HEAD: `ca9360f2f`
- Worktree: clean
- Pushes: none
- Merges: none
- Live server/worker processes or `~/.aion`: not touched

## Commits

1. **`29863a6ca` — `test(247): transcript outage spool is red`**
   - Red A and Red B committed before the implementation.
   - Commit-recorded Red A output:
     > FAILED: the first event emitted during the outage must be replayed
   - Commit-recorded Red B output:
     > FAILED: left [0, 3], right [0, 1, 2, 3]
   - The pre-outage `worker_seq == 0` control arm is retained.

2. **`6fb7d909a` — `fix(247): spool transcript events across outages`**
   - Adds disk spooling, drain-first replay, server deduplication, sequential server ingestion, and focused loss-class tests.

3. **`ca9360f2f` — `fix(247): make replay dedup collision-safe`**
   - Corrects a silent-loss risk found during audit.
   - `worker_seq` now narrows dedup candidates, but the full bounded event must also match before ingestion treats the delivery as a replay.
   - Adds a test proving that two distinct events with the same stream and `worker_seq` are both retained.

I did not independently recreate the pre-fix checkout to rerun the reds; the quoted red evidence above is the evidence recorded in the red commit itself.

## File-by-file changes

### `crates/aion-worker/src/runtime/liminal_drain.rs`

- Adds a process-incarnation-owned `DiskSpool`.
- Creates a unique temporary spool directory using process ID plus UUID.
- Stores one JSON event per insertion-monotonic filename.
- Flushes and `sync_all`s each spool entry before considering it retained.
- Spool capacity is bounded only by the filesystem; no event-count or byte constant was introduced.
- When a publish fails:
  - the event is appended to the spool;
  - pending spool entries are retried before later live events;
  - the reconnect backoff paces retries.
- A closed producer channel does not abandon pending entries; the drain waits for replay to finish.
- Adds typed `SpoolError::Disk` and `SpoolError::Malformed`.
- Adds `Raw` transcript-gap event construction for disk and malformed-entry failures.
- Removes stale documentation that still described outage events as dropped.
- Deletes the process-owned spool directory on drop; no cross-incarnation adoption is attempted.

### `crates/aion-worker/src/runtime/liminal_drain_tests.rs`

Adds the focused worker fixture and five tests:

- `outage_events_are_spooled_and_replayed_after_reconnect`
  - retains the load-bearing pre-outage control;
  - requires both outage events to appear after recovery.
- `replay_is_strictly_ordered_with_pre_and_post_reconnect_events`
  - requires `[0, 1, 2, 3]`.
- `live_first_mutation_breaks_the_ordering_oracle`
  - pins the `[0, 3, 1, 2]` mutation as invalid.
- `disk_exhaustion_surfaces_a_typed_gap_event`
  - uses a mocked `StorageFull` spool failure.
- `malformed_spool_entry_surfaces_a_typed_gap_event`
  - exercises a mocked malformed entry.

### `crates/aion-server/src/activity_publisher.rs`

- Adds durable replay deduplication using the observability keyspace rather than process-local memory.
- Deduplication is checked both before append and after a concurrent `SequenceConflict`.
- Final audited rule:
  - the four-axis `ActivityStreamKey` selects the stream;
  - `worker_seq` selects candidate entries;
  - full bounded-event equality confirms that the candidate is the same replayed entry.
- Clears server-assigned `store_seq` before equality comparison.
- This prevents a restarted producer counter from silently eating a distinct event while preserving exact-entry idempotency across server incarnations.

### `crates/aion-server/src/activity_publisher_spool_tests.rs`

Adds:

- Exact spool-entry redelivery records one durable row.
- Attempt-axis change creates a distinct dedup population.
- Same stream and same `worker_seq`, but different payloads, records both events.

### `crates/aion-server/src/activity_publisher_tests.rs`

- Updates the failover double-delivery control so both publishers submit the exact same logical event.
- Both acknowledgements resolve to the same durable position.
- Distinct-payload sequence collision behavior is now tested separately rather than being conflated with replay.

### `crates/aion-server/src/worker/liminal_transport.rs`

- Replaces per-event spawned publisher tasks with one ordered, unbounded ingestion queue.
- One consumer invokes `ActivityEventPublisher::publish` sequentially.
- This prevents replay events from racing one another at the server and avoids an invented event-count capacity on accepted replay ingestion.

## Permitted implementation choices

1. **Process-owned temporary directory**
   - Chosen because v1 explicitly cuts durable cross-incarnation spool ownership.
   - PID plus UUID avoids adopting another process’s directory or relying on PID ownership alone.

2. **One file per event**
   - Gives simple insertion ordering and allows an entry to remain present until publish succeeds.
   - Avoids an in-memory outage buffer and has no invented event-count ceiling.

3. **Global drain-first ordering**
   - Stronger than merely fencing each individual stream: while any spool entry is pending, newly received events join behind it.
   - This prevents post-reconnect live traffic from overtaking outage history.

4. **No new registration or dispatch protocol**
   - Spooling is confined to the existing `ActivityEvent` observability path.
   - Registration and dispatch remain independent of spool support.

5. **Durable server dedup**
   - Uses retained observability records, so a server restart does not erase dedup knowledge.

6. **Full-event confirmation after sequence lookup**
   - Added during audit because `worker_seq` is documented as best-effort.
   - Prevents sequence restart or reuse from becoming a new silent-loss mechanism.

7. **No configurable spool cap**
   - No default or constant was invented. Filesystem exhaustion is the operational boundary.

## Ratified requirements not satisfied

These are blockers, not accepted non-goals.

### 1. Server-authored worker-death gap is absent

There is no server implementation that records:

> stream X: last accepted event seq S at time T1, worker incarnation died ~T2, events in between are unrecoverable

The worker spool is deleted on process teardown, but the server-side death detector is not wired to a transcript-gap publisher with the dispatch identity and last accepted sequence.

### 2. Red A/B are not the required live outage fixture

The tests exercise the production drain loop against an in-memory `FakeSink`, but they do **not**:

- run a live streaming command;
- kill and restart a real server;
- observe worker re-registration;
- share the required #246 outage/recovery rig;
- prove adopted-survivor replay under a re-armed generation token.

The unit fixture proves the drain state machine, but it is not the chartered e2e proof.

### 3. Malformed-entry recording depends on a later live event

When `spool.front()` cannot decode an entry, the current implementation borrows stream identity from the next received event. If the producer channel is already closed, it only logs the malformed entry and records no typed stream event.

That violates the floor requiring malformed spool loss to be recorded rather than inferred from absence.

### 4. Disk-exhaustion diagnostic can still be lost while disconnected

`record_spool_failure` attempts to publish a gap diagnostic and discards the resulting `DrainStep`. The test uses a sink that fails once and then accepts the diagnostic, but if the transport remains unavailable, the diagnostic itself is neither retained nor retried.

Therefore the tested fixture passes, but the production arm still contains a silent-loss path.

## Ratified non-goals honored

- No durable cross-incarnation spool adoption.
- No PID-reuse or abandoned-directory ownership scheme.
- No arbitrary spool event/byte cap.
- No operational spool limit with an assumed default.
- No registration or dispatch dependency on spool support.
- No live estate changes.
- No push or merge.

## Verification actually performed

### Passed

- `cargo clippy --workspace --all-targets -- -D warnings`
  - Pass 1: exit 0, 1m30s.
  - Pass 2: exit 0, 20.78s.
- `cargo fmt --all --check`
  - Exit 0.
- `cargo test -p aion-worker --features liminal-transport`
  - 168 unit tests passed.
  - 4 `grpc_registration` tests passed.
  - 1 doctest passed.
  - 0 failed, 0 ignored.
- Focused worker spool module:
  - 5 passed.
- `cargo test -p aion-server --lib activity_publisher`
  - 17 passed.
- `cargo test -p aion-server --lib api::http::transcripts`
  - 9 passed.
- Collision-safe dedup focused tests:
  - 3 spool tests passed.
  - failover exact-redelivery control passed.
- Final worktree cleanliness:
  - clean at `ca9360f2f`.

A macOS linker warning about an oversized `__eh_frame` compact-unwind table appeared during server test links; it did not fail the focused tests or either strict clippy pass.

### Full workspace battery

Command:

```text
cargo test --workspace --all-targets
```

Result: **exit 101** after 5m34s compilation and substantial passing test execution.

Stopping failure:

```text
aion-awl-package --test sdk_closure_freshness

committed_closure_matches_a_fresh_oracle_harvest ... FAILED

committed closure BYTES lag the harvest while the wire surface matches
Divergent behavioural chunks per module:
["aion@agent (AtU8, Code, FunT, ImpT, LitT, LocT)"]
```

The failing SDK-closure files are outside the #247 change set. I did not regenerate them because that would exceed this lane’s charter.

## Final disposition

The implemented spool and replay core is green under its focused suites, and the collision-safe dedup correction is committed. However, because the server-authored worker-death gap, real shared outage fixture, and two no-silent-loss failure arms remain incomplete—and because the full battery is not green—**this branch should not be merged or presented as satisfying the ratified #247 brief.**