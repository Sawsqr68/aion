# Piece A — batched event appends (lane `wa-batching`)

**Brief:** `docs/briefs/aion-write-amplification-fix-brief-20260817.md`, piece A.
**Evidence base:** `docs/tracking/aion-write-amplification-forensics-20260817.md`.
**Worktree:** `stack/aion/.worktrees/wa-batching`, branch `wa-batching` off `dev` @ `c58079469`.
**Never pushed, never merged.**

## The defect, and what changed

Every agent telemetry event reached the durable `O` keyspace through a
transcript DRAIN that handed the sequencer one event at a time
(`activity_publisher.rs:183` -> `aion-store-haematite/src/store.rs:1617`
`append_batch(.., &[one], ..)`). Each hand-off was one storage-tree commit, and
one commit re-persists the WHOLE containing leaf as a new permanent
content-addressed blob. So a run's permanent store cost scaled with its event
COUNT, not its bytes: 2,359 honest bytes bought a 2,303,416-byte blob (~977x),
a 136 KB status sweep left 73.7 MB of permanent store (540x).

The drain now coalesces. N queued events cost **one commit per stream present
in the batch**, not one per event.

## Commits

| Commit | What |
|---|---|
| `55ed634fe` | **RED** — `test(wa-a): the transcript drain costs one store commit per event` |
| `1c82b42da` | **FIX** — `fix(wa-a): coalesce transcript appends — N events, one commit per stream` |
| `6023bad65` | **FIX** — `fix(wa-a): the stock-config test boots state the flush policy` (found by the gate extraction, not by me) |

### The red, quoted

Committed failing, on the real publisher path (the production drain
`publish_declared_transcript`, over a store that counts durable append calls),
full leg output in `gate-logs/wa-batching/red-drain-batching.log`:

```
test activity_publisher::batching_tests::the_transcript_drain_coalesces_events_into_batched_commits ... FAILED

thread '...::the_transcript_drain_coalesces_events_into_batched_commits' (961709) panicked at crates/aion-server/src/activity_publisher_batching_tests.rs:189:5:
64 queued transcript events must cost at most 4 durable commits at a batch size of 16; observed 64 — one commit per event is one whole-leaf rewrite per event, the write amplification of record

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1126 filtered out
```

The same test asserts, before it reaches the commit count, that every queued
event is still retained in arrival order with contiguous `store_seq` — so the
green cannot be bought by dropping events.

## The seam, end to end

- **`ObservabilityStore::append_activity_events(expected_seq, &[event])`** is the
  new append primitive: one atomic, all-or-nothing commit for a slice of
  same-stream events, `store_seq == expected_seq + i`, returning the new head.
  `append_activity_event` survives as a documented one-line convenience over it
  (not a second implementation). A slice that MIXES stream keys is refused
  loudly — splitting it silently would re-introduce the per-event commit this
  method exists to remove, and would assign sequences from the wrong head.
- **haematite adapter** maps that to ONE `EventStore::append_batch` of N
  payloads (upstream-documented as "atomically append many payloads … as one
  tree commit") instead of N calls carrying a batch of one.
- **`ActivityEventPublisher::publish_all`** groups a batch by stream key, keeps
  arrival order, and commits each group at one `expected_seq` under the
  UNCHANGED read-head -> append -> on-conflict-re-read-head-and-retry loop.
  `publish` is now that method over a slice of one, so single-writer law,
  `SequenceConflict` semantics, ephemeral filtering, and the per-stream
  retention cap (split at the cap, one marker, remainder live-only) produce the
  same durable states as before. A conflict leaves the WHOLE batch unwritten and
  retries it at the observed head; the retry budget still counts conflicts, not
  iterations.
- **`ActivityEventPublisher::drain`** is the ONE coalescing loop. Both production
  transcript seams run their queue through it: the liminal observability tap
  (the seam the forensics measured) and the declared-command executor. A refusal
  is warned once per failed BATCH with an EXACT refused count (per-event
  outcomes are tracked so the count is not an upper bound), never once per line
  of output.

## Durability contract (explicit)

**Unchanged for callers.** `publish` / `publish_all` return only after the
events are durably committed. An event that has been acked with a `store_seq`
is exactly as durable as it was before this lane.

**What can be lost, and why that is acceptable.** `max_batch_hold_ms` extends
one window that already existed and was already lossy: transcript events are
handed to an in-process channel (`TRANSCRIPT_QUEUE_CAPACITY`, 4096) and
everything queued there is lost if the server process dies. The hold lengthens
that pre-existing window by at most its own value, and `max_batch_events`
bounds how many events can be waiting in it beyond what the channel already
held. **Nothing that was durable becomes non-durable**; a strictly bounded
amount of not-yet-durable transcript stays not-yet-durable for strictly bounded
longer. Concretely, at the recommended 200 ms a kill -9 can cost up to ~200 ms
more of transcript than it did yesterday.

That trade is acceptable HERE and would not be elsewhere, because the `O`
keyspace is observability and never replay authority: workflow correctness
lives on the `E`-stream, whose append path this lane does not touch. Losing the
last fraction of a second of an agent's transcript costs transcript, never a
workflow decision — the same reason the drain already logs and drops an event
the store refuses rather than failing the activity. `max_batch_hold_ms = 0`
reproduces the old timing exactly: the drain then commits whatever is already
queued the moment it can, coalescing only what genuinely arrived together.

The live transcript tail is fanned out at commit time, so the hold also bounds
how far the ops-console tail lags — at 200 ms, below the threshold at which a
streaming transcript reads as delayed.

## Configured, never assumed — and the values for the operator's ruling

`observability.max_batch_events` and `observability.max_batch_hold_ms` are
REQUIRED with **no shipped default**. `Default` leaves both `None`; the boot
path (`state.rs::required_transcript_batch_policy`, beside
`required_cluster_broadcast_capacity`) refuses to build the transcript
publisher without them, naming the missing key and saying it has no default.
`TranscriptBatchPolicy::UNBATCHED` (one event per commit, no hold) exists only
for the `from_parts*` embedder constructors that bypass config validation and
cannot refuse — it is the IDENTITY policy, i.e. today's behaviour, never a
tuning value.

They are deliberately NOT checked at config-PARSE time: a parsed `ServerConfig`
is not yet a running server, and embedders inspect one without booting.

**Recommended values — Tom's ruling, not mine. Both scaffold templates ship
these with the reasoning inline, so a scaffolded server boots; the deployed
config must be edited by hand (see the operational note below).**

| Key | Proposed | Reasoning from the measured data |
|---|---|---|
| `max_batch_events` | **64** | Transcript events average 12–17 KB (forensics: 2,921 events / 47.4 MB = 16.2 KB; 1,714 events / 26.6 MB = 15.5 KB). 64 x ~16 KB ≈ **1 MB in flight** in the single drain buffer — negligible against a server that today rewrites a 73 MB leaf per event. Upper bound on the structural multiplier's collapse: a 1,714-event `norn_dev` run goes from 1,714 commits to ≥27, i.e. its ~2.98 GB of permanent store to roughly **tens of MB**. Raising it further buys less each time (the leaf rewrite is already amortised) while raising what one refused commit loses. |
| `max_batch_hold_ms` | **200** | Under load the queue backs up and batches fill regardless of the hold — the hold is what helps LOW-RATE producers. The sweep specimen emitted 338 events over 25 s (13.5/s): at 0 ms it coalesces only what happens to queue up; at 200 ms it groups ~3 events per commit (338 -> ~113 commits); at 1,000 ms it groups ~13 (338 -> ~25 commits). 200 ms buys a 3x cut on sweeps with a crash window and a console lag both under a quarter second. **If Tom prefers, 1,000 ms is a ~13x cut on sweeps for a 1 s window** — that is the whole trade, stated. |

Env overrides `AION_OBSERVABILITY_MAX_BATCH_EVENTS` /
`AION_OBSERVABILITY_MAX_BATCH_HOLD_MS` are wired (the hold parses a
NON-negative integer, because 0 is a real setting).

## Operational consequence — read before the next estate restart

**A server whose config does not carry both keys will refuse to start**, with a
message naming the key. That is the designed, loud behaviour of a
required-no-default knob (`websocket.cluster_broadcast_capacity` behaves the
same way today). The deployed estate config must gain the two keys BEFORE the
aion server is next restarted on this branch. Three integration tests that
booted from a literally empty config were updated for exactly this reason
(commit `6023bad65`) — the gate found them, and their failure was the product
behaving as designed, not a defect.

## Battery — canonical gate extraction

Run per `docs/GATE-EXTRACTION-STANDARD.md` (Tom, 2026-08-17), copied verbatim
into `gate-logs/wa-batching/gate-battery.sh` (extracted from the doc
programmatically, not retyped) and run from the worktree root on the FINAL
tree. `cargo fmt --all` was run before it; there is no fmt gate.

**Instrument proof (required before trusting it).** A deliberate failure was
planted in `activity_publisher_batching_tests.rs` (an unused binding plus a
falsified ceiling) and the extraction emitted it on all three legs, then the
probe was reverted:

```
{"type":"clippy","level":"warning","file":"crates/aion-server/src/activity_publisher_batching_tests.rs","line":204,"column":9,"lint":"unused_variables","message":"unused variable: `deliberate_instrument_probe`"}
{"type":"clippy","level":"error","file":"crates/aion-server/src/activity_publisher_batching_tests.rs","line":204,"column":9,"lint":"unused_variables","message":"unused variable: `deliberate_instrument_probe`"}
{"type":"nextest","test":"aion-server::aion_server$activity_publisher::batching_tests::the_transcript_drain_coalesces_events_into_batched_commits","stdout":"...panicked at ..."}
```

That same probe run also surfaced four REAL findings
(`update_check_start_binding` booting from an empty config), fixed in
`6023bad65` — the instrument earned its keep on its proving run.

**FINAL BATTERY — GREEN: EMPTY EXTRACTION.**

```
$ cd stack/aion/.worktrees/wa-batching
$ sh gate-logs/wa-batching/gate-battery.sh \
    > gate-logs/wa-batching/final-battery-findings.jsonl \
    2> gate-logs/wa-batching/final-battery-stderr.log
script rc=0
$ wc -l < gate-logs/wa-batching/final-battery-findings.jsonl
       0
```

Zero finding lines out of all three legs (check / clippy -D warnings /
nextest), which is the pass condition under this style. The artefacts are
committed: `gate-logs/wa-batching/final-battery-findings.jsonl` (empty by
construction) and the script actually executed,
`gate-logs/wa-batching/gate-battery.sh`.

Under this style rc codes are not the verdict, but for the record the script
exited 0 and the run was made on the final tree — HEAD `6023bad65`, working
tree clean apart from this report and the gate logs.

That the nextest leg genuinely runs and reports (rather than being silently
empty) is established by the instrument proof above: the SAME script, on the
same tree with only the probe added, emitted a `{"type":"nextest",...}` line
for the falsified assertion plus four real `update_check_start_binding`
failures.

**Earlier, superseded battery.** Before Tom's amendment arrived this lane had
run `cargo check` / workspace + per-crate `cargo clippy -- -D warnings`
(rc=0 on all five). Those logs were deleted rather than kept, because under the
canonical style rc codes are not a verdict and a stale second battery invites
being read as one. The extraction above is the only battery of record.

## Zone wall — what was touched, and the one collision

The brief's zone for piece A is the publisher path, the store adapter append
path, and `aion-store` observability. Those are all changed. Beyond them this
lane also touched:

- `worker/liminal_transport.rs` and `worker/declared_body_transcript.rs` — the
  two transcript DRAIN call sites. The fix is inert without them: they are
  where the per-event commit actually happened. ~15 lines each, both now
  delegating to the publisher's own drain.
- `state.rs`, `config/{sections,defaults,env,load,mod}.rs`, both scaffold
  templates — the required-config plumbing the brief mandates.
- ~40 test/e2e files, one or two lines each, stating the now-required flush
  policy in their server config (the same treatment
  `cluster_broadcast_capacity: Some(64)` already has in those files).

**No overlap with the live `wa-envelope-delta` lane.** Verified by set
intersection of the two branches' changed-file lists against the common base
`c58079469`: **empty**. That lane's files (`aion-integrations/*`,
`aion-integration-norn/*`, `aion-cli/src/tail.rs`, `transcript_resolve*`,
`api/http/transcripts.rs`, `stream/transcript_stream.rs`) were not opened here,
and `aion-integrations/tests/mock_harness.rs` was never touched. The zone split
held.

**One collision to report, with a different lane:** the `libsql-retirement`
worktree has an uncommitted edit to
`crates/aion-server/tests/deploy_restart_e2e.rs`, a file this lane also edits
(one line: the flush policy in its server config). Trivially resolvable, but it
is a genuine textual overlap and whoever lands second should expect it.

## Non-claims (what this lane does NOT do)

- **It does not fix the 100–540x leaf sizing.** That is piece B (format-2
  migration). This lane removes the SECOND, independent multiplier — the number
  of commits — and cuts it by up to `max_batch_events`. A run's remaining store
  cost is still one whole-leaf rewrite per COMMIT.
- **It reclaims nothing.** No superseded node is deleted; that is piece C.
- **It does not touch aion#32.** The read-side leak is Apollo's lane.
- **It does not batch across streams.** A commit addresses exactly one `O`
  stream, so a batch spanning K agents costs K commits (proved by test). This is
  a property of the store, not a shortcut.
- **Concurrent `publish` callers do not coalesce.** Coalescing happens in the
  drain, which is where the production amplifier is; a group-commit design for
  concurrent direct callers was considered and rejected, because it would have
  greened a test while leaving the serialized production drain at one commit per
  event. The predecessor's stopped-state test asserted exactly that wrong
  property (N concurrent `publish` calls) and was replaced for that reason; its
  uncommitted rustdoc edit to `aion-store/src/observability.rs` was reverted and
  rewritten to land WITH the API it describes, since at red it named a method
  that did not exist.
- **The env-override arms are not covered by a test.** `config::env::overlay`
  reads the real process environment, which no test in this crate mutates; the
  two new arms follow the existing sibling arms exactly. Stated, not claimed.
- **No live estate anything.** Nothing was run against the estate server,
  `~/.aion`, or `stack/aion/aion-data`. Every store in every test is a
  per-test temp directory created by the test itself.
