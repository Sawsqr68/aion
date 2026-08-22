# ADDENDUM — three places the correction below does NOT yet reach

The brief that follows is the CANONICAL text from ablative-docs `02935c2`,
carrying its own binding correction at the top. That correction is authoritative
and this addendum does not repeat it — one truth in two places drifts, so the
correction lives in exactly one place, below.

What the canonical correction covers: R1 (b)'s determinism claim and R1 (d)'s
fail-fast-after-one. What it does NOT yet reach — the premise is load-bearing in
three further places, and leaving them would let the flight enshrine the refuted
behaviour in a TEST, which is the worst place for it to survive:

1. **R3 (3), the green criterion** — "no chain declared ⇒ the act fails
   `policy_refused:` after exactly ONE attempt (the fail-fast proof)."
   ❌ **MUST NOT be written as a test.** A test asserting exactly-one-attempt
   pins the behaviour that would have killed workflow `4ddee56c`. Replace it
   with: no chain declared ⇒ the act exhausts its CONFIGURED bound and then
   fails `policy_refused:`, asserting the CLASSIFICATION and that **no chain hop
   was attempted**, with attempt accounting in the record — and asserting NO
   particular attempt count. Assert the invariant, not the enumeration.

2. **The "cure is two halves" paragraph**, immediately below the canonical
   correction — "(1) classify the refusal as what it is — deterministic for this
   provider-and-content, never retried in place." ❌ Same refuted premise, in the
   brief's own summary of its purpose. Read it as: classify it as what it is — a
   ROUTING fact rather than a transient fault — and stop it burning a
   same-provider retry budget under a transient label. Not: never retried.

3. **The closing sentence** — "never ask the same wall the same question twice."
   ❌ Precisely what measurement refutes: asking twice worked, twice,
   independently. The durable half is the routing insight — classify once, route
   around it rather than grinding on it. Keep that; drop the claim that the
   second ask is futile.

If you find a further site, treat it the same way and NAME it in your report.
A premise this load-bearing does not appear once; a correction that fixes only
where it was first noticed leaves the design rebuildable from the leftovers.

---

# BRIEF — aion: PolicyRefused error class + declared provider fallback

## 🔴 CORRECTION — 2026-08-22, binding, outranks the text below where they
## disagree (refutation by Vesper Lynd, ratified by Waffles)

Two of this brief's proposals are CONTRADICTED by measured engine history
and must NOT be implemented as written: (b)'s claim that a policy refusal
is **deterministic-for-this-content**, and (d)'s **fail-fast after ONE
attempt** when no chain is declared. Exhibit A: workflow `4ddee56c`
(aion84_dev, 2026-08-19) — seq 9 ActivityFailed on a `cyber_policy`
refusal, seq 11 ActivityCompleted, SAME act, attempt 2: the retry cleared
it. Exhibit B: aion#65 round 3's leg — attempt 1 died at 22m44s on
`cyber_policy`; attempt 2, same prompt and tree, delivered the leg whole.
(Exhibit B was this author's own banked evidence, not consulted when (b)
was written — the refutation stands on data I already held.) A refusal is
**stochastic, not deterministic**. What replaces the refuted parts:
`PolicyRefused` remains its own class and must still never burn a
same-provider retry budget mislabelled as transient; the declared chain is
consulted FIRST (hop first, retry second — a hop is cheaper and likelier
than a same-provider retry); with NO chain declared, a bounded
same-provider retry REMAINS, its bound a CONFIGURED value defaulting to
what the act already declares — minting "1" or "2" here is the
no-assumed-defaults violation this estate refuses. If R1 still argues
determinism it must name the mechanism that makes a refusal which
demonstrably cleared on attempt 2 deterministic. Full verdict with the
event-by-event reads: `docs/evidence/policy-refused-census-20260822.md`
in the flight base (`fleet/policyrefused-base` = `8709317ba`).

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
