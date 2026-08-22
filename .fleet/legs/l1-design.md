# LEG L1 — the design artifact

**Deliverable:** `docs/design/policy-refused-fallback.md`, committed.
**Blocks:** L2, L3. Neither may start until this is committed.
**Depends on:** nothing.

Read first, in order and in this order of authority:

1. `.fleet/BRIEF.md` §CORRECTION (`:3-27`) — binding, outranks the body.
2. `docs/evidence/policy-refused-census-20260822.md`.
3. `.fleet/plan.md` §0 (missed sites), §1 (re-derived seams), §2 (the
   load-bearing fact), §3 (findings F-1, F-2, F-3).
4. `.fleet/BRIEF.md` body — subject to §0 above.

**Two sites of the body are refuted-but-unswept.** `.fleet/BRIEF.md:86-87`
(R1(b), "deterministic — this provider will refuse this content every time") and
`.fleet/BRIEF.md:116-118` (R1(d), "fail FAST … no in-place retries"). Both are
the exact propositions the correction names as contradicted. **Follow the
correction.** Do not implement either as written, and do not quietly reconcile
them — the artifact states plainly that it supersedes both, and why.

---

## R1 — the artifact decides, with rationale, six things

### R1-a — the detection seam

**WHERE the provider's policy refusal is recognised.**

The brief proposes `AdvisingHarness` (`crates/aion-integration-norn/src/advice.rs`,
composition root `crates/aion-cli/src/harness/compose.rs:52-59`) as the template.
**Plan §2.4 finds that template structurally incapable of the job:** `advise()`
at `advice.rs:139-141` destructures `HarnessError::Harness { detail: String }`.
It sits above the adapter and only ever sees prose. Choosing it forces substring
matching — the exact thing the brief says to prefer against.

The artifact must therefore decide detection **inside** the adapter, and must
name the site. The two structured points, both confirmed on this tree:

| Path | Structured object | Dies at |
|---|---|---|
| norn JSON-RPC error | `JsonRpcError { code, message, data: Option<Value> }` — `crates/aion-integrations/src/jsonrpc/envelope.rs:110-118`, `data` at `:117` | `crates/aion-integration-norn/src/session.rs:288-291` — `data` never read |
| norn stop envelope | `stop: &Value` (`translate/outcome.rs:61`), `reason: &str` (`:67`) | `translate/outcome.rs:83` — `format!("… stop: {stop}")` |
| ACP stop reason | `prompt.stop_reason: StopReason`, variant `Refusal` — `crates/aion-integration-acp/src/translate.rs:291` | `crates/aion-integration-acp/src/outcome.rs:152-154` → `non_completion` `:169-177` |

Decide and argue:

1. **Which of the two norn points carries a policy refusal**, or that it is not
   determinable from this tree. `cyber_policy` appears NOWHERE in the repository
   — it is a fact about the provider's wire observed in flight logs, not a symbol
   here, and `crates/aion-integration-norn/src/protocol.rs` (by `:1`, the only
   module that names norn's wire) declares no error schema and no refusal-code
   key. **Do not assume the spelling.** Key on structure.
2. **What to do about `error.code` being useless as a discriminator.**
   `crates/aion-integration-norn/src/advice.rs:273-276` transcribes a real run's
   refusal: code `-32603`, the JSON-RPC generic internal error. If norn reports
   every application failure under one code, the code cannot discriminate and
   `error.data` is the only structured channel.
3. **What happens when `error.data` is `None`.** If norn does not populate it,
   say so as a measured limitation and decide the honest fallback — including,
   legitimately, "norn cannot express a policy refusal structurally today; the
   detection is prose-based here and the artifact records that as a known
   weakness with the exact substring and where it came from." A prose match
   ADMITTED and bounded is acceptable; a prose match presented as structural is
   not.
4. **ACP.** `StopReason::Refusal` is a typed variant and needs no substring
   matching at all. The brief's cited `outcome.rs:180-204` is `prompt_error`, the
   JSON-RPC path — the refusal surface is `outcome.rs:130-177`. Decide whether
   ACP's `Refusal` maps to `PolicyRefused` (it plainly should) and separate it
   from `MaxTokens` / `MaxTurnRequests`, which `non_completion` at `:169-177`
   currently folds together with it.

**Acceptance:** the artifact names, by `file:symbol`, the exact site at which
each adapter recognises a refusal, and states for each whether the decision is
made on a typed value or on text. Where it is on text, the artifact says so in
those words.

### R1-b — the new class and its `is_deterministic()` arm

`HarnessError::PolicyRefused` — enum at `crates/aion-integrations/src/error.rs:14-115`,
`#[non_exhaustive]` at `:15`. The exhaustive match at `:222-232` makes adding a
variant a compile error at that site, by design (`:190-195`). That is the
mechanism; use it.

🔴 **The arm is OPEN.** `.fleet/BRIEF.md:86-87` pre-decides it "deterministic";
that is MISSED SITE 1 and is refuted. The measurement:

- Exhibit A, `4ddee56c`: seq 9 `ActivityFailed` (`cyber_policy`, `kind=Retryable`),
  seq 11 `ActivityCompleted` — **same act, attempt 2 cleared it.**
- Exhibit B, aion#65 round 3: attempt 1 died at 22m44s on `cyber_policy`;
  attempt 2, same prompt and tree, ran 11m29s deeper and delivered whole.

Two independent survivals. **The measured default is `false`.** Per the
correction (`.fleet/BRIEF.md:23-25`), if the artifact still concludes `true` it
must NAME THE MECHANISM by which a refusal that demonstrably cleared on attempt 2
is deterministic. That argument, not the assumption, is what gets judged.

Note the tension the artifact must resolve honestly: `is_deterministic()` is a
BINARY that feeds `harness_error_to_outcome` (`crates/aion-worker/src/runtime/agent.rs:366-375`)
which maps `true`→terminal, `false`→retryable. Neither pole expresses "route
first, then retry under the act's own bound." If the artifact wants that
semantics it must say which of the two poles it takes and where the routing
behaviour is added on top — not pretend the binary carries it.

**Sweep sites** for the new variant (all confirmed present):

- `crates/aion-worker/src/runtime/agent.rs:244-248` and `:327-331` — catch-alls
  on the INTERVENTION path that would swallow a new variant silently.
- `crates/aion-integration-norn/src/advice.rs:139-155` — `advise()`'s match.
- `crates/aion-worker/src/runtime/agent.rs:335-364` — the doc block that
  enumerates the deterministic variants and goes stale the moment one is added.

### R1-c — the reason vocabulary

New `policy_refused:` beside `retryable:` / `terminal:` / `timeout:` /
`cancelled:` / `non_determinism:` (**five, not three** — `activity_dispatch.gleam:135`,
`:138`, `:141`, `:145`, `:151`; engine-side prefixes in
`crates/aion/src/runtime/nif_activity_retry.rs:31`, `:37`, `:76`, `:382`).

🔴 **Plan finding F-1 makes the brief's "either is acceptable" untrue on this
tree.** `crates/aion-server/src/worker/bridge.rs:475-490`:

```rust
let prefix = if error.is_retryable() { "retryable" } else { "terminal" };
Err(format!("{prefix}:{}", error.message))
```

Every worker-originated reason is minted there, from `ActivityErrorKind`'s two
variants and nothing else. A worker cannot produce a third prefix today. So the
artifact must pick exactly one of:

1. **`ActivityErrorKind` gains a variant** — `crates/aion-core/src/error.rs:9-14`.
   This is a wire + TS-codegen event. The type is registered at
   `crates/aion-core/src/generated_types.rs:222` (`push_type::<ActivityErrorKind>`).
   Regenerate; never hand-edit. That file's own doc (`:3-19`) records the
   2026-07-30 break from a forgotten registry entry — heed it.
2. **`bridge.rs:475-490` mints a third prefix** from something other than
   `is_retryable()`. Name what.
3. **The structured-detail route (plan finding F-2)** —
   `ActivityFailure::with_detail(Payload)` at `crates/aion-worker/src/activity.rs:52-57`
   survives into `ActivityError.details` (`:104`), crosses the worker protocol
   (`crates/aion-worker/src/protocol/session.rs:832`), and is **durably recorded**:
   `Event::ActivityFailed { error: ActivityError, … }`, `crates/aion-core/src/event.rs:469-478`.
   `harness_error_to_outcome` (`agent.rs:366-375`) never calls it — the channel
   exists end-to-end and carries nothing today.
   **Caveat the artifact must resolve:** `bridge.rs:490` reads only
   `error.message`, so `details` reaches history but NOT the reason string the
   retry loop (`nif_activity_retry.rs:96`) and the Gleam grammar
   (`activity_dispatch.gleam:134`) parse. Say how the two are reconciled.

Silent mislabelling is not acceptable in any option.

### R1-d — the fallback chain

AWL-declarable at the same altitude as `retry`:

- `ActionConfig` fields — `crates/aion-awl/src/ast/document.rs:535-541`
  (`agent`, `node`, `timeout`, `retry`). Brief's `:540-541` **holds**.
- `RetrySpec` — `document.rs:553-580`. Note `:555-556`: *"`retry N` declares N
  FURTHER attempts AFTER the first — N+1 total executions."*
- Lowering twins — `crates/aion-awl/src/emitter/stmts.rs:163-180`
  (`retry_policy`; brief said `:154-180`, the symbol is at `:163`) and
  `crates/aion-awl/src/mir/lower/activity.rs` — the twin is `build_retry_policy`
  at **`:284`**, body `:284-320`ff, with `total_attempts` named as the emitter's
  counterpart at `:282` (brief: `:274-285` — **MOVED**, and the brief's range
  covers the doc comment rather than the symbol).
- A queue IS the provider-flavour selector: `PoolAddress` —
  `crates/aion-server/src/worker/registry.rs:116-131` (brief: `:117-123` —
  **MOVED**), and its own doc `:121-122` says so: *"`task_queue` is the
  pool/flavour selector within that namespace (norn / claude / cpu / gpu)."*
- AWL has NO model field, and this flight does not invent one. Measured
  constraint (evidence doc `:76-89`): `norn auth login` offers ChatGPT alone,
  confirmed at both boxes — **a provider change is NECESSARILY a queue/harness
  hop**, never an in-harness model rotation. The hop's destination is proven to
  exist: rocketfish's ACP/Claude seats with `ANTHROPIC_MODEL=claude-opus-5`,
  verified by a discriminating arm (setting `claude-sonnet-5` returns
  `claude-sonnet-5`).

Semantics to pin down:

- **Hop first, retry second.** A `PolicyRefused` outcome consults the declared
  chain FIRST and re-dispatches the act on the next queue. A hop is cheaper and
  likelier than a same-provider retry.
- **Chain hops are their own budget** — the chain's length — DISTINCT from the
  retry budget. A policy refusal never burns same-provider retry attempts.
  `failure_stand_down` at `crates/aion/src/runtime/nif_activity_retry_dispatch.rs:194`
  (`let retry_eligible = expired || is_retryable_reason(reason);` — brief's `:194`
  **holds exactly**) must not count a refusal retry-eligible while a chain is
  live.
- **Chain exhausted** ⇒ the act fails `policy_refused:` with every refusing queue
  named.
- 🔴 **No chain declared** ⇒ a **bounded same-provider retry REMAINS**. This
  supersedes `.fleet/BRIEF.md:116-118` (MISSED SITE 2). The bound is a CONFIGURED
  value defaulting to what the act already declares — `RetrySpec.count`
  (`document.rs:561-580`) resolved through `retry_policy_from_config`
  (`crates/aion/src/runtime/nif_activity_retry.rs:206`).
  **DO NOT MINT THE BOUND.** Writing "1" or "2" into the artifact is a
  no-assumed-defaults violation, and "1" is affirmatively refuted by Exhibit A.
  What is removed is the waste and the mislabelling, not the retry.

The seam that makes the waste real, for the artifact's rationale:
`nif_activity_retry_dispatch.rs:261-262` clones the request and mutates ONLY
`delivery.attempt` — every same-provider retry is byte-identical but for a
counter. And `crates/aion-integration-norn/src/harness.rs:637-646`
(`unrecognised_braced_text_is_literal`) pins that `{attempt}` is not an argv
placeholder, so even the spawned command is identical. (The brief calls this a
negative test on "per-attempt provider variation"; the effect is real, the intent
is not stated in the test — cite it for what it shows.)

### R1-e — replay determinism

The re-dispatch must be recorded such that replay re-derives the same queue
sequence.

- `resolve_task_queue` — `crates/aion/src/runtime/nif_activity.rs:108-116`, with
  the three-level precedence documented at `:81-84`: per-activity override
  (`config.task_queue`) → workflow-level declared default
  (`config.workflow_task_queue`) → `start_time_task_queue`. Brief's `:77-113`
  brackets this; the symbol is at `:108`.
- The replay note at `:97-98` is the constraint: *"`start_time_task_queue` MUST
  be derived from recorded history (see `aion_core::start_time_task_queue`),
  never from live or wall-clock state."*
- `config_task_queue_selection` — `:123-141`.

The artifact says **how the fallback hop composes with that precedence**: whether
a hop is a fourth precedence level, an override written into the recorded
dispatch, or a derivation from history. Determinism boundary (CLAUDE.md
invariant 2) and single-writer (invariant 3) both bind: the hop's queue must be
re-derivable from history alone, and nothing may call `EventStore::append`
directly.

### R1-f — measured red is a success outcome

If R1-e concludes the fallback hop cannot be made replay-deterministic without
restructuring beyond this flight's scope, **that conclusion WITH EVIDENCE is the
deliverable.** Commit it in the artifact, state exactly what would be needed, and
report it. L3 is then descoped and L2 lands alone — classification without
routing is still the larger half of the cure, and it fully satisfies the
correction's core requirement that a refusal never burn a same-provider retry
budget mislabelled as transient. No heroics, no silent narrowing, no improvised
partial hop.

---

## Acceptance for L1

1. `docs/design/policy-refused-fallback.md` exists and is committed.
2. Every one of (a)–(e) is decided with rationale, not deferred.
3. (a) names the detection site by `file:symbol` per adapter and states for each
   whether the decision is typed or textual.
4. (b) states the `is_deterministic()` arm and, if `true`, names the mechanism
   that makes a refusal which cleared on attempt 2 deterministic.
5. (c) picks exactly one of the three F-1 options and says how `details` and the
   reason string are reconciled.
6. (d) contains **no minted bound**. Grep the artifact for a bare retry count
   before committing.
7. (e) states how the hop composes with `resolve_task_queue` precedence and
   history-derived replay — or records measured red per (f).
8. The artifact states, in its own words, that it supersedes `.fleet/BRIEF.md:86-87`
   and `:116-118`, and why.
9. `git ls-tree` receipt before declaring done.

## Walls

`.fleet/plan.md` §5 applies in full. For a document-only leg the live ones are:
no minted defaults; no assertion of determinism without a named mechanism; report
any policy refusal your own work trips as a first-class finding with what tripped
and where — do not rewrite the work to dodge it.
