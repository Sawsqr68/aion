# LEG L3 — routing: the declared fallback chain, replay determinism, E2E

**Deliverable:** a policy refusal consults the declared chain FIRST and
re-dispatches on the next queue; the hop is replay-deterministic; R3.2 and R3.3
green.
**Depends on:** L1's committed `docs/design/policy-refused-fallback.md`, and
L2's `HarnessError::PolicyRefused` + reason vocabulary. Branch from L2's head.
**Blocks:** nothing.

**Runs only if L1 did NOT record measured red under R1-f.** If it did, this leg
does not exist — L2 lands alone and that is the designed outcome.

Read first: `docs/design/policy-refused-fallback.md` (authoritative), then
`.fleet/plan.md` §1–§3, then `.fleet/BRIEF.md` §CORRECTION.

Implement the artifact's (d) and (e) decisions and only those.

---

## Scope

`crates/aion-awl` (AST, parser, checker, printer, emitter, MIR lowering),
`crates/aion` (retry/dispatch loop, task-queue resolution),
`crates/aion-server` (bridge, registry) as the artifact requires.

**Out of scope:** the `HarnessError` variant, adapter detection, the Gleam
grammar arms — all L2. The `exhausted:`/`parked:`/`lost:` fallthrough
mislabelling — report only, per `.fleet/BRIEF.md:134-136`.

---

## R2.7 — the AWL declaration

At the same altitude as `retry`, per the artifact's chosen surface (the brief
suggests `fallback <queue>[, <queue>…]`, `.fleet/BRIEF.md:100-106`).

- `ActionConfig` — `crates/aion-awl/src/ast/document.rs:535-541`: `agent`
  (`:535`), `node` (`:537`), `timeout` (`:539`), `retry` (`:541`). Brief's
  `:540-541` **holds**.
- `RetrySpec` — `document.rs:553-580`. Read `:555-559`: *"`retry N` declares N
  FURTHER attempts AFTER the first — N+1 total executions. The lowerings
  translate `count` into the SDK's total-attempt budget."* The fallback chain's
  budget must not be confused with this one.
- **Lowering twins — both, or the two paths drift.** `retry_policy` at
  `crates/aion-awl/src/emitter/stmts.rs:163-180` (brief: `:154-180`; the symbol
  is `:163`) and its direct-lowering twin `build_retry_policy` at
  `crates/aion-awl/src/mir/lower/activity.rs:284` (brief: `:274-285` —
  **MOVED**; that range covers the doc comment, not the symbol). Both funnel
  through `total_attempts` — `stmts.rs:182`ff and its counterpart named at
  `activity.rs:282`. `document.rs:557` names them as twins explicitly.
- Parser, checker, canonical printer (`aion awl fmt`) and schema derivation all
  need the new `ConfigLine`. `aion awl check` must reject a malformed chain with
  a spanned diagnostic, not a panic.

**A queue IS the provider-flavour selector.** `PoolAddress` —
`crates/aion-server/src/worker/registry.rs:116-131` (brief: `:117-123` —
**MOVED**), doc `:121-122`: *"`task_queue` is the pool/flavour selector within
that namespace (norn / claude / cpu / gpu) — a miss is a liveness issue, never a
correctness one."*

**AWL has no model field and this flight does not invent one.** Measured
constraint (evidence doc `:76-89`): `norn auth login` offers ChatGPT alone,
confirmed at both boxes — norn can never serve an Anthropic model whatever
`-m/--model` says, so a provider change is NECESSARILY a queue/harness hop. The
chain design must not assume in-harness model rotation. The destination is proven
to exist: rocketfish's ACP/Claude seats with `ANTHROPIC_MODEL=claude-opus-5`,
verified by a discriminating arm.

## R2.8 — chain consultation: hop first, retry second

🔴 **The chain is consulted FIRST.** A hop is cheaper and likelier than a
same-provider retry (`.fleet/BRIEF.md:18-20`). This ordering is the correction's
central requirement.

The seam:

- `dispatch_with_retries` — `crates/aion/src/runtime/nif_activity_retry_dispatch.rs:247`,
  loop from `:260`. The clone-and-mutate-only-`attempt` is `:261-262` (**MOVED
  −3** from the brief's `:264-265`) — today nothing but the counter varies
  between attempts, which is why 25 minutes buy nothing. `harness.rs:637-646`
  pins that `{attempt}` is not even an argv placeholder, so the spawned command
  is identical too.
- `failure_stand_down` — `:183-203`. `let retry_eligible = expired ||
  is_retryable_reason(reason);` at **`:194`** (brief's `:194` **holds exactly**).
  A `PolicyRefused` reason must not be counted retry-eligible while a chain is
  live.
- The `exhausted:` terminal at `:197-200`; the call site at `:288-293`.
- `is_retryable_reason` — `crates/aion/src/runtime/nif_activity_retry.rs:96`;
  its doc `:92-95` is the wire-prefix-only doctrine.

**Chain hops are their own budget — the chain's length — DISTINCT from the retry
budget.** A policy refusal never burns same-provider retry attempts. Chain
exhausted ⇒ the act fails `policy_refused:` with **every refusing queue named**.

🔴 **No chain declared ⇒ a bounded same-provider retry REMAINS.**

This supersedes `.fleet/BRIEF.md:116-118` ("fail FAST … no in-place retries"),
which is a refuted, unswept restatement of proposal (d) — see `.fleet/plan.md`
§0, MISSED SITE 2. The correction (`.fleet/BRIEF.md:20-23`) is binding, and
R3.3 (`:147-151`) was already swept to match it, so the body currently
contradicts itself. Follow the correction.

The bound is a CONFIGURED value defaulting to what the act already declares:
`RetrySpec.count` (`document.rs:561-580`) → `retry_policy_from_config`
(`nif_activity_retry.rs:206`) → `RetryPolicy.max_attempts`, read at
`nif_activity_retry_dispatch.rs:196`.

🔴 **DO NOT MINT THE BOUND.** Writing "1" or "2" is a no-assumed-defaults
violation, and "1" is affirmatively refuted by measurement: workflow `4ddee56c`
refused act 1 on attempt 1 and COMPLETED act 1 on attempt 2, same act, same
content (evidence doc `:37-50`). Under a one-attempt collapse it dies at seq 9.
aion#65 round 3 shows it independently (`:52-56`).

What is removed is **the waste and the mislabelling** — a refusal grinding
against a provider that keeps refusing while a declared alternative was never
consulted. **Not the retry.**

## R2.9 — replay determinism

The re-dispatch must be recorded in history such that replay re-derives the same
queue sequence.

- `resolve_task_queue` — `crates/aion/src/runtime/nif_activity.rs:108-116`.
  Precedence documented at `:81-84`: per-activity override (`config.task_queue`)
  → workflow-level declared default (`config.workflow_task_queue`) →
  `start_time_task_queue`.
- `config_task_queue_selection` — `:123-141`.
- The binding constraint, `:97-98`: *"`start_time_task_queue` MUST be derived
  from recorded history (see `aion_core::start_time_task_queue`), never from
  live or wall-clock state."*
- `:93` notes the `start_time_task_queue` fallback is purely additive — preserve
  that property.

CLAUDE.md invariants that bind here and are not negotiable:

- **#2, the determinism boundary.** The hop's queue must be re-derivable from
  history alone. No wall clock, no entropy in workflow-visible paths.
- **#3, single writer.** Both the durability replay handoff and the
  timer/signal/child services append through the one `Recorder`. **Never call
  `EventStore::append` directly.** A `SequenceConflict` is a double-writer bug.
- **#4, status is a projection.** Never a stored mutable field.

**If the hop cannot be made replay-deterministic within this scope:** stop.
That is R1-f's measured-red outcome arriving late. Record it with evidence,
commit it, report it. No heroics, no improvised partial hop, no silent
narrowing.

---

## R3 — green for this leg

### R3.2 — E2E positive (BRIEF.md:143-146)

An act whose worker refuses by policy re-dispatches to the declared fallback
queue and **COMPLETES there**. Assert: history shows one refusal + one hop, zero
same-queue retries; `failure_reason` empty (the run succeeded).

### R3.3 — E2E negative (BRIEF.md:147-151)

No chain declared ⇒ the act exhausts its **CONFIGURED** retry bound and fails
`policy_refused:`.

🔴 **Assert the CLASSIFICATION and that NO CHAIN HOP was attempted, with attempt
accounting in the record. Assert NO PARTICULAR ATTEMPT COUNT.**

🔴 **NEVER write a test asserting the no-chain path fails after exactly ONE
attempt, whatever any text in the brief body says.** That pins, as a passing
assertion, the behaviour that would have killed `4ddee56c`. It is the single
most damaging thing this leg could produce, because a green test is a claim that
the behaviour is correct. **The invariant, not the enumeration.**

Concretely, the shape to write:

- ✅ assert the reason carries the `policy_refused:` classification
- ✅ assert zero `ActivityTaskScheduled` on any queue other than the original
- ✅ assert the attempt count in the record equals the act's configured bound,
  read from the configured value — never compared against a literal
- ❌ never `assert_eq!(attempts, 1)` or any bare integer

### Env gating

If either E2E needs live seats, gate **at runtime** per house rules: read an env
var, emit a `tracing::info!` skip line naming the variable, `return Ok(())`.
**No `#[ignore]` in any spelling.**

### ⛔ Polling, not sleeping

Never one blocking sleep sized to the expected duration. Poll, with a **total
deadline**, and **timeout as its own distinct outcome** — distinguishable in the
assertion from both success and refusal. An agent-backed act takes minutes and a
refusal takes ~22 minutes (evidence doc `:52-53`); a fixed sleep is either a
flake or a 25-minute stall.

### R3.5 — gates

`cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check`
clean. AWL golden/canonical-printer fixtures updated for the new `ConfigLine`.
Full workspace green, two-instrument protocol per venue config.

---

## Walls

`.fleet/plan.md` §5 in full. Live for this leg:

- No lint suppression in any spelling. No `#[ignore]` in any spelling.
- No file over 500 lines of code. **`crates/aion/src/runtime/nif_activity_retry.rs`
  is 905 and `crates/aion-awl/src/ast/document.rs` is 648 — do not add to
  either.** `nif_activity_retry_dispatch.rs` is 609, `nif_activity.rs` is 455.
  New logic goes in new named modules; `mod.rs` holds only `pub mod` and
  re-exports.
- `thiserror` for library errors. Never `.unwrap()` / `.expect()` in library
  code. Lock poison mapped to a typed error.
- Wire/TS changes REGENERATED, never hand-edited. If a type is added, register
  its `push_type::<T>` at `crates/aion-core/src/generated_types.rs:210-234` —
  that file's doc `:3-19` records the 2026-07-30 break from forgetting one.
- 🔴 No minted bound. No minted chain length. No "sensible default" anywhere.
- Do NOT widen scope to the `exhausted:`/`parked:`/`lost:`/`transport-exhausted:`/
  `superseded:` fallthrough mislabelling at
  `gleam/aion_flow/src/aion/internal/activity_dispatch.gleam:156`. Report it as a
  named finding; it deserves its own item.
- Report any provider policy refusal your own work trips as a first-class finding
  with what tripped and where. It is evidence for this very brief, not an
  obstacle to route around. Do not rewrite the work to dodge the trigger.
- **Measured red is a success outcome.** A conclusion with evidence, committed
  and reported, beats a hop that only looks deterministic.

## Landing

L3's head carries both halves and is what merges to the flight base
(`.fleet/BRIEF.md:127`). `git ls-tree` receipt before declaring done.

## The sentence to hold on to

A policy refusal is a routing fact before it is a retry candidate. **Classify it
once, and route around the wall before grinding against it** — and when there is
no wall to route around, retry the bound the author already wrote, not a number
this flight invented.
