# Piece D — envelope-delta persistence

**Lane:** `wa-envelope-delta` · **Branch:** `wa-envelope-delta` off `c58079469` (landed main)
**Brief:** `ablative/docs/briefs/aion-write-amplification-fix-brief-20260817.md`, piece D
**Forensics:** `ablative/docs/tracking/aion-write-amplification-forensics-20260817.md`

## The defect, and what changed

A provider streaming one LLM turn emits three lifecycle frames — `response.created`,
`response.in_progress`, `response.completed` — and each carries the *entire* response
envelope, instructions block included. The Norn adapter passed them through verbatim as
`ActivityEventKind::Raw`, so all three copies became permanent store: ~82 KB each,
**~91% of a transcript's bytes**, before the structural amplifier pieces A/B address even
fires.

The emitter now persists a turn's first frame whole and its later frames as deltas against
it. Events stay events; identity, `worker_seq` and the ephemeral flag are untouched. What
changed is only what the emitter puts *inside* a `Raw` event.

## Reader-first ordering — evidenced by the commit sequence

The wall: the reader lands first and proves it renders old-format, delta and mixed streams,
and the old-format proof runs against a reader this lane has not moved.

| # | Commit | What it is |
|---|---|---|
| 1 | `9b624e456` | **PIN** — the pre-change old-format render, generated from real bytes |
| 2 | `ba906fbc7` | The envelope-delta codec (format + both halves). No consumer, no emitter wired |
| 3 | `d9947ec87` | **READER ARM LANDS** — reconstruction at the serving boundary, three shapes proved |
| 4 | `7da379992` | **RED** — writer measurement on the real pump, committed failing |
| 5 | `55d2a68a3` | **FIX** — the Norn pump compacts |
| 6 | `8a5ac31aa` | Lint fix on the red measurement's helper |
| 7 | `0a3217e32` | Encoder/decoder symmetry on ephemeral frames |

Commit 1 pinned `crates/aion-cli/tests/fixtures/norn_response_turn_old.rendered.txt` by
running the then-current `render_event` over an old-format fixture and writing its output
verbatim — not by hand-authoring an expectation. **That file has not been modified since,
and the test asserting it has not been modified since.** The writer arm (commits 4–5) is
therefore greened against a reader this lane pinned before it touched anything.

## Where reconstruction happens, and why there

Reconstruction is at the **server's transcript serving boundary**, not in each consumer.

The transcript consumers are the CLI's `tail`, the ops console's TypeScript, the MCP
`read_transcript` tool and the live-describe projection. Teaching four codebases one delta
format — one of them in another language — is four implementations and four chances to
drift. Instead all four serving surfaces call one shared module
(`crates/aion-server/src/transcript_resolve.rs`):

- `stream/transcript_stream.rs` — the WebSocket, **one decoder across replay *and* the live
  splice**, so a turn straddling the splice still resolves;
- `api/http/transcripts.rs` — the REST fetch;
- `mcp/tools/transcript.rs` — the MCP tool;
- `api/http/describe_live.rs` — the live-describe tail.

The three windowed surfaces reconstruct **before** they narrow. A turn's base routinely sits
outside the slice a caller asked for while being the only thing that can resolve the deltas
inside it; resolving after the cut would have manufactured the very failure the boundary
exists to avoid.

**Consequence:** the ops console and every other downstream consumer needed *no change*, and
`render_event` in `aion-cli` is byte-for-byte unmodified.

## Lossless by construction, not by assertion

**The encoder never emits a delta it has not proved reversible.** It applies the delta back
to the base and compares **serialized bytes** — not `Value` equality, which ignores object
key order and would have let a key reordering through as "equal". A frame that fails to
reproduce exactly, or whose delta is not smaller, is persisted whole. Compaction therefore
*cannot* change what a reader sees; its worst case is saving nothing.

**The decoder never guesses.** A delta binds to its base by SHA-256 over the base's
serialized bytes. A base that retention trimmed, or that the server's per-event size ceiling
truncated after the encoder read it, produces a *named refusal* rather than a
plausible-looking wrong envelope. The delta document survives the refusal, so the fields that
actually changed are never lost.

Removals are explicit RFC 6901 JSON Pointers rather than the RFC 7386 merge-patch `null`
convention, because these envelopes genuinely carry `"usage": null` on an unfinished turn and
merge-patch cannot distinguish that from a deletion.

Bases are held **one per (stream, agent)**, not one per turn: a turn's frames are contiguous,
so a frame bearing a new response id retires the previous base. What either half retains is
bounded by the number of live streams — no eviction policy, no cap invented, nothing that
grows with the length of a run.

## Red evidence, quoted

`gate-logs/wa-envelope-delta/writer-red.log`, exit **101**, committed in `7da379992`. The
measurement drives the real `NornSession` pump over an in-memory duplex — not a synthetic
call to the codec.

```
---- one_turn_persists_its_instructions_block_exactly_once stdout ----
assertion `left == right` failed: the turn-invariant instructions block belongs in ONE
persisted frame per turn, not in every lifecycle frame of it
  left: 3
 right: 1

---- one_turn_costs_about_one_envelope_not_three stdout ----
a turn is one envelope plus its deltas: 30498 B against a 10143 B first frame
(three full copies would be about 30429 B)

---- the_frames_after_a_turns_first_are_persisted_as_deltas stdout ----
assertion `left == right` failed: the base is persisted whole and the other two frames
as deltas against it
  left: 0
 right: 2
```

`30498` against `30429` is the defect stated arithmetically: the turn cost three whole
envelopes, and the two frames after the first bought nothing but a status field.

### Green, same measurement, same pump

```
one turn: 11387 B persisted; base frame 10143 B; three full copies would be ~30429 B
```

**30,498 B → 11,387 B, a 62.7% reduction.** The two frames after the first cost 1,244 B
between them instead of 20,355 B — **94% off those two frames**. The saving is the size of
what stops repeating, so on the ~82 KB production frames the forensics measured, where the
instructions block is most of the envelope, the ratio is better than this fixture's.

## Three stream shapes, proved at both layers

Mixed history exists forever, so all three are first-class.

**At the boundary** (`crates/aion-server/src/transcript_resolve/tests.rs`, 5 tests) the
*served events* are compared byte-for-byte against the uncompacted stream, so equivalence
does not rest on the renderer discarding something convenient:

- pure old format — served byte-identical, nothing to reconstruct;
- pure delta — served byte-identical to the same turns in old format;
- mixed, old turns then compacted turns in **one** stream — same;
- a read opening after a base — reports the loss, fabricates nothing, and the *other* turn
  still resolves;
- the streaming path — a live delta resolves against a base delivered during replay.

**At the consumer** (`crates/aion-cli/src/tail.rs`, 5 new tests) each shape is rendered
through the **unmodified** `render_event` and compared to a golden generated from the
old-format fixture by that same renderer. Fixtures were generated by the real encoder, not
hand-authored:

| Fixture | Shape |
|---|---|
| `norn_response_turn_old.json` / `.rendered.txt` | the commit-1 pin, untouched |
| `norn_two_turns_old.json` / `.rendered.txt` | control: two turns, all frames full |
| `norn_two_turns_delta.json` | pure delta: 2 bases + 4 deltas |
| `norn_two_turns_mixed.json` | turn A old-format, turn B compacted |

## Battery — canonical gate extraction, final tree

Run per `ablative/docs/GATE-EXTRACTION-STANDARD.md`. The script was **copied verbatim** out
of the standard (`sed`-extracted, not retyped) to
`gate-logs/wa-envelope-delta/battery.sh`, sha256
`67ed5a796013b2cfebfdf4b240d51f8a21e73dec824a2c4f8b148e73578104fb`. `cargo fmt --all` was run
first and produced no source changes. No fmt gate.

**Green = empty extraction.** Result: `gate-logs/wa-envelope-delta/final-battery.jsonl`,
**1 line**.

| Leg | Findings |
|---|---|
| `cargo check --workspace --all-targets --keep-going` | **0** |
| `cargo clippy --workspace --all-targets --keep-going -- -D warnings` | **0** |
| `cargo nextest run --workspace --all-targets --no-fail-fast` | **1** (see below) |

### Instrument proved able to go red

Before trusting the extraction, a deliberate failure was planted and the battery run in full
(`gate-logs/wa-envelope-delta/instrument-proof.jsonl`): an unused variable in
`envelope_delta/encode.rs` and a wrong expected value in one codec test. All three legs
emitted:

```
clippy   unused_variables   (level "warning" from the check leg, x2)
clippy   unused_variables   (level "error" from the clippy leg, x2)
nextest  aion-integrations::…::the_repeated_instructions_block_is_persisted_once_per_turn
```

Both plants were reverted (`git checkout --`) before the final run; the final tree is clean
against HEAD.

### The one finding, verbatim

```json
{"type":"nextest","test":"aion-rs::awl_bc4_differential$covered::covered_set_is_byte_identical","message":"test failed"}
```

```
thread 'covered::covered_set_is_byte_identical' panicked at
crates/aion/tests/awl_bc4_differential/covered.rs:467:5:
covered differential found divergences:
differential report: 68 succeeded, 37 failed-path, 0 cancelled, 3 timer-parked,
3 excluded, 0 refused, 0 AGREED-REFUSAL, 10 DIVERGENCES, 0 INFRA
  DIVERGENCE dag-fork/valid/after_multi_diamond event[9]  /data/activity_id: reference=2 direct=1
  DIVERGENCE dag-fork/valid/after_multi_diamond event[10] /data/activity_id: reference=1 direct=2
  (+8 byte-level divergences, all in the same two swapped events)
```

**This finding is NOT attributable to this lane, and the evidence is:**

1. **The divergent case CHANGES between runs.** The instrument-proof run failed on
   `fork_named_homogeneous` with **2** divergences; the final run failed on
   `after_multi_diamond` with **10**. A deterministic regression reproduces the same
   divergence; this does not.
2. **Three isolated re-runs of that exact test passed**, rc=0, zero divergences
   (`gate-logs/wa-envelope-delta/flake-probe-{1,2,3}.log`). Both failures occurred during a
   full-workspace run on a heavily contended box (a sibling lane's clippy plus the live
   estate `aion` server at ~54% CPU); nextest also reported the test "running for over 60
   seconds".
3. **This lane touches zero files in `crates/aion`** —
   `git diff --name-only main...HEAD -- crates/aion/` is empty.
4. **`aion-rs`'s dependency closure contains none of the crates this lane modified** —
   `cargo tree -p aion-rs --edges normal,build` names no `aion-integrations`,
   `aion-integration-norn`, `aion-server` or `aion-cli`.
5. The divergence is a parallel-fork `activity_id` ordering swap between the reference and
   direct execution paths — an AWL/engine concern with no transcript, envelope or
   observability surface in it.

An earlier `cargo nextest run --workspace --no-fail-fast` (without `--all-targets`) on this
tree ran **5118 tests, 5118 passed, 0 failed** — that test is not in that target set, which
is precisely the blind spot the `--all-targets` standard exists to close.

**Verdict: this lane's gates are green. One inherited, load-sensitive nondeterministic flake
in the AWL BC4 differential is reported here rather than absorbed.** It is a real defect
worth someone's lane; it is not this one's, and this lane does not claim to have fixed or
diagnosed it beyond the attribution above.

No `#[allow]`, no `#[expect]`, no `#[ignore]`, no `_var` renames, no `#[cfg(any())]`, no
skips anywhere in this lane.

## Zone wall — held, no collision

D's declared files are the event-emission layer and the transcript reconstruction layer; A's
are `crates/aion-server/src/activity_publisher.rs` and
`crates/aion-store-haematite/src/store.rs`.

**Neither of A's files is touched.** Verified:
`git diff --name-only main...HEAD | grep -E "activity_publisher|aion-store-haematite"` → no
matches. **No collision to report.**

One contact point for the merge, not a collision: this lane adds a `mod` declaration to
`crates/aion-server/src/lib.rs` and a dependency line to `crates/aion-server/Cargo.toml`. If
lane A adds config plumbing to either file the overlap is textual, in a different region, and
not a semantic conflict.

`aion-integrations` moved from a **dev**-dependency of `aion-server` to a real one, since a
default build now links the codec. That is a deliberate, stated change to what a stock
`aion-server` links; the SDK's own dependency discipline (never reach into aion's platform
layers) is unaffected — `sha2` is a leaf crate already in the workspace lock.

## Non-claims — explicitly NOT proved

- **No live e2e was run.** Every measurement is from in-process tests driving the real
  `NornSession` pump and the real serving-boundary module. No isolated in-worktree server was
  built or run. The live estate, `~/.aion` and estate `aion-data/` were never touched (a live
  `aion` server was observed running on this box and left alone). The end-to-end path
  *emitter → store → HTTP/WS → CLI render* is proved by composition of unit proofs at each
  hop, **not** by an observed running system.
- **No store-side byte measurement.** The reduction reported is of the bytes the emitter
  hands the publisher. What that becomes on disk is governed by the leaf-rewrite amplifier
  pieces A and B address; this lane's saving multiplies with theirs but is not measured
  through them.
- **`aion-integration-acp` is not compacted.** Only the Norn adapter's pump was wired. ACP
  was inspected and does not emit full provider response envelopes in this shape; if it ever
  does, its frames are persisted whole — the status quo, not a regression.
- **Nothing is claimed about aion#32** or about the 100–540× storage multiplier. Piece D
  reduces honest content; the structural amplifier is untouched by it.
- **The ops console was not rebuilt or visually verified.** It needed no change because
  reconstruction is server-side and no wire type moved; that is an argument from the design,
  not a pixel observation.

## Known limitation, named and bounded

A read whose window **opens inside a turn** cannot reconstruct that turn: the base is older
than the window. The reader reports it, names the turn and the missing record's `worker_seq`,
and serves the delta document intact — it never fabricates an envelope, and frames whose base
*is* in the window still resolve, so one turn's loss does not spread.

Frequency is bounded by *at most one turn per window opening edge* — the windowed surfaces
resolve before narrowing, so this only bites when the underlying read itself begins mid-turn
(a resumed cursor, or retention having trimmed the base). A durable fix needs a store-side
index from `worker_seq` to `store_seq` so the boundary could fetch a base outside its window,
or turn-aligned windows. Both are out of this lane's scope and are **not** done here.

A second, operator-reachable case: if `observability.max_event_bytes` is configured below a
full envelope's size, bases are truncated server-side after the encoder diffed against them.
The digest check catches this and every delta of that turn reports `BaseDigestMismatch` —
detected and named, never a silently wrong render. The shipped default (256 KB) is
comfortably above the measured ~82 KB envelope.

## Recommended follow-on (for the operator's ruling, not done here)

The two frames this lane compacts carry no information the terminal frame lacks. If
render-equivalence with pre-change history were ever relaxed, `response.created` and
`response.in_progress` could be reduced to lifecycle notes and dropped entirely, taking a
turn's cost to one envelope. That is a **semantic** change to the transcript and was
deliberately not taken: render-equivalence is this lane's acceptance criterion.
