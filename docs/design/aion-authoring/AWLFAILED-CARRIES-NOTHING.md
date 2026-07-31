# `{"tag":"AwlFailed"}`: the runtime fault that says nothing

Measured 2026-07-31 for task #83. **Design only — nothing is built.** The
reason it is not built is at the end and it is the important part.

## The defect, restated from the measurement

An AUTHORED failure route records a fully informative detail:

```json
{"outcome":"muted","payload":"{\"reason\":\"message carried no weight\"}","tag":"AwlOutcomeFailure"}
```

A RUNTIME fault records this:

```json
{"tag":"AwlFailed"}
```

Proven on `step-bodies/valid/send_signal`, where `send heartbeat to
coordinator_id` fails because the target is not a live run. The run dies at seq
3 with no `ActivityScheduled` — correct and loud, and completely
undiagnosable. No signal name, no target, no cause. This is the exact shape of
the recurring "it just stopped".

## Scope established: it is NOT specific to the signal path

The task asked this first, so it was answered first.
`gleam/aion_flow/src/aion/awl/error.gleam` carries ten variants, and eight of
them already carry a `String`. The taxonomy is not the problem. Three mappers
are:

| mapper | source | what it produces | what it discards |
| --- | --- | --- | --- |
| `map_engine_error` | `error.EngineError` | `AwlFailed` | **everything** |
| `map_spawn_error` | `error.EngineError` | `AwlChildFailed("detached spawn failed")` | the message |
| `map_timer_error` | `error.EngineError` | `AwlTimerFailed("timer failed")` | the message |

And `error.EngineError` is a one-variant type that exists to carry a reason:

```gleam
pub type EngineError {
  EngineFailure(message: String)
}
```

So the reason IS available at every one of those three sites and is thrown
away at all three. Compare `map_activity_error` and `map_receive_error`
directly above them, which both describe their error in full — the file itself
records why (2026-07-23: a provider stream error surfaced to the operator as
"workflow 5 returned error").

`map_engine_error` has two callers in the emitter, and they are why the
signal case looks specific when it is not:

- `emitter/sends.rs:40` — `signal.send(...) |> awl_error.map_engine_error`
- `emitter/exprs.rs:261` — `workflow.id() |> awl_error.map_engine_error`

Anything else routed through `map_engine_error` later inherits the same
silence by default. **It is a language-wide diagnosis gap, and the fix belongs
at the mapper rather than at the send path.**

## One place, both backends

`crates/aion-awl/src/mir/runtime.rs:170` lowers `MapEngineError` to a call of
`aion@awl@error:map_engine_error/1`. The direct-bytecode backend and the Gleam
reference backend therefore run the SAME function. This is the rare
three-layer-looking defect that is genuinely one-layer: changing the mapper
changes both backends identically, and the differential should stay
byte-identical across the change rather than needing a per-backend fix.

That is worth stating explicitly because the recent pattern in this codebase
has been the opposite — an invariant with several independent enforcers that
diverge silently. Here there is one enforcer, and it is simply wrong.

## The fix

Two halves, with very different risk.

**Half A — zero blast radius.** `map_spawn_error` and `map_timer_error` keep
their existing variants (`AwlChildFailed`, `AwlTimerFailed`, both already
`String`-carrying) and put the engine's own message into them:

```gleam
Error(error.EngineFailure(message)) ->
  Error(AwlTimerFailed("timer failed: " <> message))
```

No tag changes, no wire shape changes, no ABI change. Only the recorded
message text improves, and old records keep their old text and still decode.

**Half B — the actual reported defect, and it is a wire change.**
`map_engine_error` has nowhere to put a message: `AwlFailed` carries none.
Options considered:

1. Give `AwlFailed` a payload. Rejected: the decoder would then require a
   `message` field, and every history already holding a bare
   `{"tag":"AwlFailed"}` would fail to decode. Child error propagation decodes
   `AwlError` out of history, so this breaks the replay of existing runs. A
   durable-execution engine may not do that to a parked run.
2. Reuse an existing message-carrying variant. Rejected: every one of them
   names a failure CLASS (`activity`, `signal`, `child`, `timer`), and a
   workflow-context lookup failure is none of them. Misfiling a fault to reuse
   a field is how a taxonomy stops meaning anything.
3. **Add `AwlEngineFailed(String)`.** `AwlFailed` stays exactly as it is —
   which leaves it doing the one job it is actually still needed for, the
   decoder's fallback for an unrecognised tag (`error.gleam:129`). Old
   histories decode unchanged; new faults carry their reason.

Option 3 is the design. Its blast radius, enumerated so nobody discovers a
piece of it later:

- `gleam/aion_flow/src/aion/awl/error.gleam` — variant, `to_json`, `decoder`,
  and the `map_engine_error` body.
- `gleam/aion_flow/test/aion_awl_codec_test.gleam` — roundtrip and the
  exhaustive tag list.
- `crates/aion/tests/awl_bc4_differential/abi.rs:73-85` — `AWL_ERROR_TAGS` is
  a CLOSED-SET pin on the tag atoms a generated `run/1` may return. A new
  variant must be added there deliberately, which is the pin working.
- The prebuilt `aion@awl@error.erl` copies under
  `crates/aion/tests/fixtures/*/build/` — these carry a compiled snapshot of
  this exact module, and a stale one would make a fixture disagree with the
  SDK.

## Why this is designed and not built

**It changes the durable `AwlError` wire family, and the only instruments that
can prove it did so safely are the BC-4 differential and the aion-rs suite —
both of which are frozen behind the all-clear.**

Landing a wire-shape change to a durable record on a tree that cannot be
gated is precisely the thing that must not be done for the sake of momentum.
Half A could land safely today; splitting the change would put a partial fix
of a diagnosis defect on main while its headline case still records nothing,
and would spend the differential run twice. It waits and lands as one motion.

When the all-clear arrives, this lands AFTER the two owed gates, not before
them: a battery that goes red should have one lane's changes under it, not
two.

## Ready-to-build checklist

1. `error.gleam`: add `AwlEngineFailed(String)`; encode/decode it; make
   `map_engine_error` carry `EngineFailure(message)`. Fix `map_spawn_error`
   and `map_timer_error` in the same commit (Half A).
2. `aion_awl_codec_test.gleam`: roundtrip the new variant; extend the
   exhaustive list.
3. `abi.rs`: add the tag to `AWL_ERROR_TAGS`, with a comment saying what it is
   for — the closed set is a real pin and must not be widened casually.
4. Rebuild the prebuilt fixture copies of `aion@awl@error`, or confirm the
   harness rebuilds them.
5. Gates: `gleam test` in `gleam/aion_flow`; `cargo test -p aion-awl`;
   `cargo test -p aion-rs --test awl_bc4_differential` — the differential must
   stay byte-identical, because both backends call the same changed function.
6. Verify at the bytes, not by inspection: re-run the `BC4_DUMP_FAILED` tap on
   `step-bodies/valid/send_signal` and quote the recorded detail. The close
   condition for #83 is that the recorded JSON names the failure, not that a
   test is green.

---

# BUILT AND CLOSED — 2026-07-31

Landed as one motion, after the two owed gates, exactly as the checklist above
required. Item 4b of the released queue.

## The close condition, met at the bytes

`BC4_DUMP_FAILED=step-bodies/valid/send_signal` on the reference trail. The
`WorkflowFailed` event's `details` payload now decodes to:

```json
{"message":"workflow_id: invalid character: found `x` at 0","tag":"AwlEngineFailed"}
```

**The recorded JSON names the failure.** Before this change it was
`{"tag":"AwlFailed"}` — a durable record saying the run failed and nothing
whatever about why, which is the entire defect. An operator reading the trail
can now see that the workflow id was rejected, and where.

## What was actually wrong

`EngineError` has exactly one variant and it carries a `String`. The reason was
always present. `map_engine_error` matched `Error(_)` and returned the
payload-free `AwlFailed`, so the message was dropped one line from the record
it belonged in. Nothing had to be plumbed; something had to stop being thrown
away. `map_spawn_error` and `map_timer_error` did the same with fixed strings
("detached spawn failed", "timer failed") and were fixed in the same commit as
Half A required.

All three now match `Error(error.EngineFailure(message))` rather than `_`, so a
future variant must come here and decide what it says instead of silently
inheriting the old behaviour.

## The tests were pinning the defect

Four existing assertions asserted the reason was discarded, and were green:

- `map_spawn_error_maps_failure_test` fed in `EngineFailure("boom")` and
  required the output to be `AwlChildFailed("detached spawn failed")` — "boom"
  deliberately absent.
- `map_timer_error_maps_failure_test`, the same shape.
- `runtime_codecs.rs:271` and `:311` pinned `{error, awl_failed}` through the
  PRODUCTION host, on both backends.
- `map_engine_error`, the headline case, had **no test at all**.

They now assert the opposite, renamed to say what they check. This is worth
recording plainly: **a test that pins a defect protects it.** Four green
assertions were the reason this stayed invisible while the trail said nothing.

## Gates run

| Gate | Result |
| --- | --- |
| `gleam test` (aion_flow) | 154 passed, 0 failed |
| `cargo test -p aion-awl` | 506 passed, 0 failed |
| `cargo test -p aion-awl-package` | all green incl. freshness + determinism |
| `cargo test -p aion-rs --test awl_bc4_differential` | 26 passed, 0 failed — byte-identical parity holds |
| `cargo clippy -p aion-awl -p aion-awl-package --all-targets -D warnings` | 0 findings |

Parity mattered most: both backends call the changed function, and the
per-fixture parity assertions would have caught a fix that reached only one.

## Two things found on the way, neither of them #83

**1. The committed SDK closure had accumulated TOOLCHAIN drift.** Regenerating
rewrote all 46 modules, including `gleam_stdlib` beams this change cannot
touch. Separated by measurement rather than assumed: a clean-tree regen with
the #83 edits reverted still produced a full rewrite (bundle `2b1cc2af…`), and
re-applying the source edit moved it to `a04dc8ab…`. So the bulk is compiler
drift and the delta between those two hashes is the part that is #83 — which
is also the proof the source edit reached the shipped bytes.

**2. 🔴 The freshness gate is byte-blind, and that is a real hole.**
`committed_closure_matches_a_fresh_oracle_harvest` PASSED against the stale
committed closure. It compares the module SET and each module's EXPORT SET —
never bytes. So it catches an added, removed or re-exported module and is blind
to a changed function BODY.

Which means: **had `error.gleam` been edited without regenerating, nothing
would have caught it.** The Gleam suite proves the new behaviour from source;
the Rust runtime loads the committed beam with the old behaviour. Green suite,
fossil artifact — the same shape as the wire-dead worker SDKs. The two archive
pins DO compare bytes, but they only fire once someone regenerates, so they
cannot detect a closure that silently lags its own source. Filed separately.
