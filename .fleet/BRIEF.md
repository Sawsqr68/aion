# 🔴 READ THIS FIRST — A PREMISE OF THE BRIEF BELOW IS REFUTED BY MEASUREMENT

The brief that follows is gated FIT TO FLY, but two of its proposals are
CONTRADICTED by the engine's own history and must NOT be implemented as
written. This banner is binding and outranks the text below it wherever they
disagree. The full verdict is
`docs/evidence/policy-refused-census-20260822.md`, which travels in this base;
R1 argues against those measured events, not against a remembered count.

## What is refuted

The brief proposes (b) that `PolicyRefused` be classified **deterministic**
("this provider will refuse this content every time") and (d) that with no
chain declared the act should **fail fast after ONE attempt**. Both are false:

- **Exhibit A** — workflow `4ddee56c` (`aion84_dev`, 2026-08-19), read event by
  event: `seq 9 ActivityFailed act 1 attempt 1` (a `cyber_policy` refusal) then
  `seq 11 ActivityCompleted act 1 attempt 2`. Same act, same content — the
  retry CLEARED it and the flight continued. Under "fail fast after one
  attempt" that flight dies at seq 9.
- **Exhibit B** — aion#65 round 3's leg, independently, on a different repo:
  attempt 1 died at 22m44s on `cyber_policy`; attempt 2, same prompt and same
  tree, ran 11m29s PAST attempt 1's death depth and delivered the leg whole.

A refusal is **stochastic, not deterministic-for-content**.

## What replaces it

- `PolicyRefused` may still be its own class, and it must still stop burning a
  same-provider retry budget. That half of the brief STANDS.
- **Hop first, retry second.** A declared chain hop is cheaper and likelier to
  succeed than a same-provider retry, so the chain is consulted first.
- **With no chain declared, a bounded same-provider retry REMAINS**, because
  measurement says it sometimes clears. What is removed is the waste and the
  mislabelling — not the retry.
- **DO NOT MINT THE BOUND.** The honest number is unknown (2 survivals
  observed, tiny sample). It is a configured value like every other retry
  bound, defaulting to what the act already declares. Minting "1" or "2" here
  is the no-assumed-defaults violation this estate refuses — and "1" is
  affirmatively refuted above.
- If R1 still argues determinism after weighing both exhibits, it must name the
  MECHANISM that makes a refusal which demonstrably cleared on attempt 2
  deterministic. That argument, not the assumption, is what gets gated.

## Two further binding notes

**The grounding tree is not main.** The brief's seams come from `89ab967a`
(`regate/inv87r2`). Re-derive every `file:line` on YOUR tree before use and
report any coordinate that moved. A positional reference fails silently — a
line number still resolves after the code moves — so anchor on the SYMBOL.

**If this flight's own content trips the refusal it is fixing, REPORT THAT as a
first-class finding.** It is evidence for the brief, not an obstacle to route
around. Do not rewrite the work to avoid the trigger; say what tripped and
where.

---

# BRIEF — aion: PolicyRefused error class + declared provider fallback

Tom's ruling, 2026-08-22 09:50 his time, governs this flight: a provider
rejecting an act under its safety policy must not be a flight-killer.
"If there's an error like that, it can automatically retry with a
different one." Today the refusal arrives as `Retryable`, the engine
re-asks the SAME provider the SAME content four times (~25 min), and the
flight dies. Four flights died this way 08-20→08-21 (both #68
investigates, inv73-harness-flight2, beamr flight1). The cure is two
halves: (1) classify the refusal as what it is — deterministic for this
provider-and-content, never retried in place; (2) reroute it to the next
provider in a chain the workflow DECLARED.

Seam provenance: exploration of tree `89ab967a` (branch regate/inv87r2)
— re-derive every line number on your tree; the SEAMS are the contract.

## The defect chain, precisely (all five seams cited)

1. norn adapter wraps the provider error verbatim:
   `crates/aion-integration-norn/src/session.rs:289` →
   `HarnessError::Harness` (`crates/aion-integrations/src/error.rs:78`).
   The provider's structured refusal (openai sse error event, code
   `cyber_policy`) is flattened to prose and never inspected.
2. `HarnessError::is_deterministic()` (`error.rs:223-233`) returns false
   for `Harness` ⇒ `harness_error_to_outcome()`
   (`crates/aion-worker/src/runtime/agent.rs:366-378`) marks it
   RETRYABLE.
3. The retry loop
   (`crates/aion/src/runtime/nif_activity_retry_dispatch.rs:247-330`)
   clones the request mutating ONLY `delivery.attempt` (:264-265) —
   per-attempt provider variation is foreclosed by design (negative test
   `crates/aion-integration-norn/src/harness.rs:639-645`).
4. `failure_stand_down()` (:183-203) wraps exhaustion:
   "exhausted: spent N of N attempts; last refusal: retryable:…".
5. The Gleam grammar (`gleam/aion_flow/src/aion/internal/
   activity_dispatch.gleam:134-160`) has no arm for `exhausted:` — it
   falls through to `ActivityEngineFailure`, so a content refusal
   presents to the operator as an ENGINE fault.

## R1 — THE DESIGN ARTIFACT. Committed before implementation:
`docs/design/policy-refused-fallback.md` in-repo, deciding with rationale:

- (a) **Detection seam**: where the provider's policy refusal is
  recognized. The template is the one existing text-inspection decorator
  `crates/aion-integration-norn/src/advice.rs` (`AdvisingHarness`,
  composition root `crates/aion-cli/src/harness/compose.rs:54-59`) — but
  today it only rewrites messages; RE-CLASSIFICATION is new capability.
  Prefer the provider's STRUCTURED code (`cyber_policy` in the sse error
  event) over substring matching wherever the adapter still holds the
  structured object; name where that object is last available. Cover the
  ACP adapter's equivalent surface
  (`crates/aion-integration-acp/src/outcome.rs:180-204`) or state with
  evidence why ACP cannot express a policy refusal.
- (b) **The new class**: `HarnessError::PolicyRefused` (enum at
  `error.rs:16-113`, `#[non_exhaustive]`). The exhaustive
  `is_deterministic()` match gets its arm decided ON PURPOSE (the
  crate's own doc :190-194 demands exactly this): deterministic — this
  provider will refuse this content every time. Sweep the listed match
  sites: `agent.rs:244-248`, `:327-331` (catch-alls that would swallow
  it silently), `advice.rs:138-155`, and the stale-able doc at
  `agent.rs:335-364`.
- (c) **The reason vocabulary**: new `policy_refused:` prefix beside
  `retryable:`/`terminal:`/`timeout:`
  (`crates/aion/src/runtime/nif_activity_retry.rs:31-96`, render sites
  `crates/aion-server/src/worker/bridge.rs:474-480` +
  `declared_body.rs:481-486`). DESIGN DECISION, argued in the artifact:
  whether `ActivityErrorKind` (`crates/aion-core/src/error.rs:8-14`)
  gains a variant — that is a wire + TS-codegen event
  (`generated_types.rs:222`) — or stays binary with `policy_refused:`
  carried in the prefixed reason. Either is acceptable; silent
  mislabelling is not.
- (d) **The fallback chain**: AWL-declarable, at the same altitude as
  `retry` (`ConfigLine`, `crates/aion-awl/src/ast/document.rs:540-541`;
  lowering twins `emitter/stmts.rs:154-180` +
  `mir/lower/activity.rs:274-285`) — e.g. `fallback <queue>[, <queue>…]`
  naming task queues, since a queue IS the provider-flavour selector
  (`PoolAddress`, `registry.rs:117-123`) and AWL has NO model field
  (model lives in the worker binary; that stays true — this flight does
  not invent one). Measured constraint (Vesper, 08-22, at the boxes):
  norn is structurally ChatGPT-only (`norn auth login` offers ChatGPT
  alone) — a provider change is NECESSARILY a queue/harness hop; the
  chain design must not assume any in-harness model rotation. Semantics to pin down in the artifact: a
  PolicyRefused outcome consults the chain and re-dispatches the act on
  the next queue; chain hops are their own budget (the chain's length),
  DISTINCT from the retry budget — a policy refusal never burns
  same-provider retry attempts (`failure_stand_down` :194 must not
  count it retry-eligible). Chain exhausted ⇒ the act fails as
  `policy_refused:` with every refusing queue named. No chain declared
  ⇒ today's behavior minus the waste: fail FAST as `policy_refused:`
  (no in-place retries), not after N identical attempts.
- (e) **Replay determinism**: the re-dispatch must be recorded in
  history such that replay re-derives the same queue sequence
  (`resolve_task_queue` precedence + `start_time_task_queue` replay
  note, `crates/aion/src/runtime/nif_activity.rs:77-113` — the design
  says how the fallback hop composes with that).

## R2 — IMPLEMENTATION, per the committed design only

Both halves land together: classification (adapter → `PolicyRefused` →
deterministic, never retried in place) and reroute (declared chain
honored end-to-end). The Gleam grammar gains its arms — `parse_error/1`,
`encode_error/1` (`activity_dispatch.gleam:134-195`),
`describe_activity_error/1` (`awl/error.gleam:160-175`), `mock.gleam:80-90`
— so `policy_refused:` presents as itself, never as "activity engine
failure". While there, do NOT widen scope to the pre-existing
`exhausted:`/`infrastructure:` fallthrough mislabelling — REPORT it as a
named finding (it deserves its own item), don't fix it silently.

## R3 — GREEN, DEFINED PRECISELY

1. Unit: a synthetic provider policy refusal (structured code) classifies
   `PolicyRefused`; an ordinary harness error still classifies `Harness`
   retryable — both directions tested.
2. E2E (env-gated per house rules if it needs live seats): an act whose
   worker refuses by policy re-dispatches to the declared fallback queue
   and COMPLETES there; history shows one refusal + one hop, zero
   same-queue retries; `failure_reason` empty (the run succeeded).
3. E2E negative: no chain declared ⇒ the act fails `policy_refused:`
   after exactly ONE attempt (the fail-fast proof; attempt accounting in
   the record).
4. The Gleam surface: a `policy_refused:` failure presents with that
   prefix, not "activity engine failure" (assertion on the projected
   `failure_reason`, `crates/aion-core/src/filter.rs:176-206` path).
5. Full workspace gates green, two-instrument protocol per venue config.

## Process walls — binding

- No lint suppressions in any spelling; no `#[ignore]` in any spelling;
  runtime env-gates only. No file over 500 lines. `thiserror` errors;
  never `.unwrap()`/`.expect()` in library code. Wire/TS changes, if the
  design chooses them, regenerate — never hand-edit — generated types.
- ⛔ Never one blocking sleep sized to expected duration; poll + total
  deadline + timeout-as-distinct-outcome.
- Report path per the workflow's injected wire contract; `git ls-tree`
  receipt before the leg declares done.
- Measured red = success outcome under the F6 wall: if the design pass
  concludes the fallback hop cannot be made replay-deterministic without
  restructuring beyond this scope, that conclusion with evidence is the
  deliverable — committed, reported, no heroics.

## The sentence to hold on to

A refusal that is deterministic for this provider and this content is
not a retry candidate — it is a routing fact. **Classify it once, route
around it once, and never ask the same wall the same question twice.**
