# The ACP adapter could not have served a single turn

Adversarial review of `crates/aion-integration-acp` (2026-07-30), before it was
committed. The crate's own gates were green — workspace clippy `-D warnings`,
`cargo fmt --check`, 78 in-crate tests, `RUSTDOCFLAGS="-D warnings" cargo doc` —
and it still could not do the one thing it exists for.

Recorded because the *mechanism* by which green tests hid it is reusable, and
because the fix rulings should not have to be rediscovered.

## The critical finding: every successful turn would hang until activity timeout

`pump.rs` returns only on child-stdout EOF or a transport error, and the sole
`event_tx` lives inside that future. So the events stream closes **only when the
agent process exits**.

But the platform's only agent driver — `aion-worker/src/runtime/agent.rs:142-181`,
verified at the bytes — loops on `events.next()` and *only when the stream yields
`None`* does it `break` and call `wait_result`. Its own comment: "End of the event
stream == end of run. Take the result next."

A real ACP agent is a **persistent stdio server**. It answers `session/prompt` with
`end_turn` and keeps running. So the driver blocks forever with a perfectly good
`PromptOutcome` sitting unread in the oneshot.

Proven with a conforming hand-written Python ACP agent driven through the real
`AcpHarness` (real subprocess, real handshake) using the driver's exact loop:

```
agent stays alive (real ACP behaviour):  3 events → TIMED OUT after 8.1s, no result
agent exits after the turn:              ... stream ended → RESULT ok in 46ms
```

### Why every test agreed with the bug

Each existing test avoided the failing path *by construction*:

- `wire_tests.rs:255` does `drop(agent)` to force EOF.
- `wire_tests.rs:682-692` and `tests/acp_live_agent.rs:98-116` break out of the loop
  at the `Stop` event rather than draining to `None`.

The live test's own comment even states the stream "ends when the agent's stdout
closes, which does not happen until the process exits." The author **knew** the
stream does not end; what was missed is that the consumer contract requires it to.

**This is the reusable lesson.** The intent spec said the terminal `session/prompt`
Response is "structurally the same shape `NornSession` already handles". It is — at
the frame level. But Norn's child **exits** after `run/execute`, so its stream
closes; ACP's does not. `aion-integration-norn` is correct for a reason that does
not transfer, and "same shape as the thing that works" carried the difference that
mattered right past a green suite. A test that breaks early cannot catch a bug about
the stream never ending.

## Rulings taken on the rest

Full finding list and per-finding rationale went to the fix lane. The judgement
calls, so they are not re-litigated:

- **Fix end-of-run in this crate, at the pump** — drop the event sender once the
  terminal Response is handled. Do NOT add a new signal to the shared seam: the
  contract is already documented and Norn already honours it.
- **Reasoning becomes durable.** `agent_thought_chunk` was an ephemeral
  `Progress::Note`, so after a run the agent's reasoning existed nowhere. The stated
  defence — per-token frames must not be persisted — is precisely what
  `MessageAccumulator` already solves in the same crate for the same frame shape.
  The flood argument justifies "not one event per token"; it does not justify "no
  durable record". Aion's direction is harness-agnostic agent observability, and
  reasoning that vanishes at end-of-run defeats it.
- **User chunks accumulate too.** Treating each `user_message_chunk` as its own
  durable message was defended by "a user turn is low-volume" — an assumption about
  agent behaviour, not a protocol guarantee. The type is literally `ContentChunk`.
- **A non-JSON-RPC stdout line is recoverable, not fatal.** One `Debugger listening
  on ws://…` line — the exact shape a `npx`/node-launched agent emits — discarded an
  already-received terminal result, emitted no `Stop`, formed no durable message, and
  said nothing at default log levels. Record it verbatim, warn, keep pumping.
- **A failed terminal frame gets a terminal event.** `StopKind::Error { message }`
  already exists in `aion-core`, so emitting it is not the "invention" the doc feared.
- **`PARTIAL_TEXT_MESSAGE_BYTES = 4000` stays.** `aion-integration-norn` ships the
  same shape, so matching it is consistency rather than an invented default — but the
  precedent gets documented at the constant so it stops reading as an arbitrary number.

## Verified sound — worth recording so it is not re-reviewed

- **Every inbound peer request gets exactly one response**: permission (decodable and
  not), `fs/*`, `terminal/*`, `elicitation/create`, `unstable_*`, unknown methods, and
  handshake-window requests all reach exactly one write. No path writes twice or with a
  mismatched id, and the terminal oneshot cannot fire twice.
- **`wait_result` cannot return before the text is complete** — single ordered stream,
  and the accumulator is flushed before the outcome is sent.
- **No child leaked** on spawn failure, handshake failure, version refusal, or
  timeout-drop.
- **Schema claims checked against the pinned 1.5.0 source and found true**: ACP v2 lists
  neither `fs/*` nor `terminal/*`; `text`/`resource_link` are the baseline content
  blocks; method names and error codes come from the schema; no type uses
  `deny_unknown_fields`, so `_meta` extras do not break decoding; `ProtocolVersion` is a
  `u16` newtype whose wire form is `1`.
- **House rules**: no bypass attribute anywhere; largest non-test file is `harness.rs`
  at 319 code lines.

## What the fix lane closed, verified at the reviewer's hands

Committed as `11759502`. The gates were re-run independently of the lane's
report — 97 tests, clippy `-D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc`,
`cargo fmt --check` — plus the checks a report cannot substitute for: no bypass
attribute or panic-class call anywhere, every non-test file under 500 code lines,
no construction of `AcpHarness` outside the crate (so `exit_grace` becoming a
required parameter breaks nothing), and every `Cargo.lock` addition traceable to
this crate's dependency tree.

The critical finding is closed at the pump: `event_tx` moved into `PumpState` as
an `Option` and set to `None` immediately after the terminal Response is fully
handled — flush, verbatim frame, `Stop`, outcome, *then* drop. The order matters:
the outcome is in the oneshot before the stream ends, so a driver that sees
`None` and calls `wait_result` finds the result waiting.

**Two paths proven against real subprocesses, with the fixtures committed so
neither proof is one-off** (`crates/aion-integration-acp/tests/fixtures/`):

- `persistent_agent.py` — answers the turn, prints the non-JSON banner mid-turn,
  and keeps running. Turn completes in 0.30s; the agent's own stderr shows it
  shutting down on stdin EOF.
- `deaf_agent.py` — answers the turn and then ignores stdin EOF. With a 2s grace
  the run completes in 2.04s with no orphan process left. This was the **one
  branch the lane could only reason-verify**; it is the branch that stops a deaf
  agent hanging a worker forever, so it is now measured. The live test's terminal
  result is bounded by `grace + margin` rather than a flat timeout, which is what
  makes the bound the assertion.

The recoverable-protocol-error claim was checked at the seam rather than taken on
trust: `JsonRpcConnection::recv` calls `next_line()` before it fails, so the
offending line is already consumed and the framing stays aligned — the loop
cannot spin on the same bad line. The documented claim is true.

## Known limitations, stated rather than discovered later

- **`cwd` is per-harness, not per-attempt.** Concurrent agent activities on one worker
  share a workspace root and can overwrite each other. Per-attempt workspace
  provisioning is a separate design and belongs with
  `docs/briefs/acp-harness-composition-root.md`.
- **A turn that ends `end_turn` having emitted only tool calls succeeds with
  `text: ""`.** That is honest — an agent may legitimately do only tool work.
- **The real-agent path remains unexercised.** No ACP agent binary is installed on this
  box, so actual frame ordering from Claude Code / Gemini CLI / OpenCode, real
  permission-option shapes, and whether any of them chunk user messages or churn
  `messageId` are all unverified. The committed fixtures are hand-written: they validate
  the adapter's logic and the subprocess path and prove nothing about real-agent
  conformance. **`--check` passing is not a turn**, and a fixture turn is not a
  third-party turn — the first real-agent run is still owed.

## Routed elsewhere, second entry

`JsonRpcConnection::recv` (`jsonrpc/transport.rs:114`) drops the offending line and
reports only serde's message, so no adapter can put a bad frame's actual text on a
transcript. The ACP pump records the rejection instead and says so at the site. Fixing it
means changing the shared seam, and it belongs with the `id: null` wedge below.

## Routed elsewhere

`aion-integrations`' `IncomingMessage::from_value` (`jsonrpc/envelope.rs:216-227`)
classifies `{"id":null,"method":"session/request_permission"}` as a **notification**, so
it is recorded and never answered — a wedge. Shared-seam behaviour affecting Norn too,
so not the ACP crate's to change.
