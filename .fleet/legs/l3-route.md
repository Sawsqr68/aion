# LEG `l3-route` — authored fallback and routing

**Leg id:** `l3-route` (the only leg; the split is fixed at dispatch)
**Branch from:** `fleet/policyref-l3-base` @ `c0c04142`
**Depends on:** L2's ratified result (`fleet/policyref-l2-result` = `68568af4d`),
already landed in this tree.
**Blocks:** nothing.

---

## 0. What you are standing on, and what wins

L1 designed. L2 classified and **it is landed in your tree** — you CONSUME it and
never re-derive it. L3 ROUTES.

Read, in this order, before writing a line:

1. `docs/design/policy-refused-fallback.md` — **THE ARTIFACT. It wins every
   disagreement**, including with this brief. Sections R1-d, R1-e, "Why the
   authored list is immutable for a run", and "Implementation boundaries and
   gates" are your specification. If this brief and the artifact disagree,
   follow the artifact and **say so in your report**.
2. `.fleet/plan.md` — the seams re-derived by symbol on this tree, with
   coordinates. Every file:line in this brief came from there.
3. This brief.

Coordinates in this brief were read off this tree. Re-derive anything you touch
by SYMBOL anyway; the symbol is the contract, the line number is a convenience.

---

## 1. Scope

`crates/aion-core` (the settlement predicate, the new `Event` variant),
`crates/aion` (recorder method, retry loop, dispatch seam, cursor, recovery),
`crates/aion-awl` (lexer, parser, AST, checker, printer, both lowerings),
`gleam/aion_flow` (the activity builder and dispatch config),
`tools/tree-sitter-awl` + the ops console's authoring assets and editors
(the keyword vocabulary gates), and tests.

**Out of scope — CONSUME, do not duplicate.** Adapter recognition
(`crates/aion-integration-norn/src/session.rs:297`,
`crates/aion-integration-acp/src/outcome.rs:176`), `HarnessError::PolicyRefused`
(`crates/aion-integrations/src/error.rs:99`), `ActivityErrorKind::PolicyRefused`
(`crates/aion-core/src/error.rs:14`), `harness_error_to_outcome`
(`crates/aion-worker/src/runtime/agent.rs:379`), the bridge's
`policy_refused` mapping (`crates/aion-server/src/worker/bridge.rs:478`), and
Gleam's `policy_refused:` parse/encode
(`gleam/aion_flow/src/aion/internal/activity_dispatch.gleam:138`, `:191`).

**Report-only, do not fix:** the `exhausted:`/`parked:`/`lost:` engine-side
prefix fallthrough (artifact lines 38-39), and errata E-1/E-2/E-3 in §9.

---

## 2. 🔴 NON-NEGOTIABLES — carried verbatim where marked

### 2.1 The settlement terminal predicate — binding counsel finding, verbatim

> 🔴 THE SETTLEMENT TERMINAL PREDICATE (L2 counsel OPEN-2, ruled):
> "L2 must not implement it; L3 cannot omit it."
> Five sites read `!error.is_retryable()` as "settled" (audit-time
> coordinates — RE-DERIVE each by symbol on this tree):
> `crates/aion/src/durability/cursor.rs:343`,
> `crates/aion/src/runtime/nif_collect_settlement.rs:339` and `:430`,
> `crates/aion/src/runtime/fan_out.rs:339`,
> `crates/aion/src/runtime/nif_activity_retry.rs:349`.
> A `policy_refused:` reason is NOT retryable — but with a live fallback
> chain it is NOT SETTLED either. Every one of those five sites must decide
> the three-valued question on purpose; leaving any on the two-valued read
> silently settles a mid-chain refusal.

### 2.2 `is_retryable_reason` — binding counsel finding, verbatim

> 🔴 CONFIRMED BY L2's JUDGE (hand-off note): `is_retryable_reason`
> (`nif_activity_retry.rs:96-98` audit-time) still tests only the
> `retryable:` prefix, so on the tree you stand on a `policy_refused:`
> reason settles without retry or hop. Artifact R1-d step 5 ("no-chain →
> existing configured retry policy, refusal stays nonterminal while budget
> remains, terminal on exhaustion with `policy_refused:` reason — never
> relabeled") plus hop-first-retry-second is YOURS to make true.

Re-confirmed on this tree: `crates/aion/src/runtime/nif_activity_retry.rs:96`
tests only `RETRYABLE_REASON_PREFIX` (`:32`), and `failure_stand_down`
(`crates/aion/src/runtime/nif_activity_retry_dispatch.rs:194-202`) drops a
`policy_refused:` reason into its `_` arm as an immediate terminal failure.

### 2.3 Single writer

> 🔴 **Single writer.** `ActivityFallbackRouted` is appended ONLY through the
> workflow's existing Recorder held by `RetryRecorderSeam`. NEVER
> `EventStore::append` directly. The recorder guard re-reads the run segment
> under the recorder lock (settled ordinal / closed epoch / already-recorded
> hop stands the task down).

### 2.4 The immutability premise

> 🔴 **Immutability premise**: the authored fallback list is read from THE RUN'S
> OWN BOUND MODULE VERSION, never the currently deployed package (artifact
> §"Why the authored list is immutable" — current-deploy reading is a
> long-lived-workflow replay break invisible in every test).

### 2.5 The four hard walls

- 🔴 **No numeric retry bound is minted anywhere.** The design deliberately has
  none. The no-chain path uses the act's existing resolved `RetrySpec` via
  `retry_policy_from_config`. Do not invent a default, a cap, or a "sensible"
  chain length.
- 🔴 **`resolve_task_queue` gains NO fourth precedence level.** Fallback is a
  post-failure override of the recorded initial choice, applied outside that
  function.
- 🔴 **Hop ordering is durable** (artifact R1-e order 1-5); replay after a crash
  at EACH boundary chooses the same recorded queue — and is tested at each.
- 🔴 **Terminal `policy_refused:` reason lists queues by RECORDED sequence**, not
  by recomputing what might have happened.

---

## 3. R1 — the settlement terminal predicate

**R1.1** Add to `crates/aion-core/src/error.rs`, beside `is_retryable` (`:33-40`):

```rust
/// Whether this failure SETTLES its activity: only `Terminal` does.
///
/// The complement of `is_retryable` is NOT this question. `ActivityErrorKind`
/// is three-valued: `PolicyRefused` is neither retryable by the ordinary
/// budget nor settling, because the engine may still route it to a declared
/// fallback queue or spend a configured same-provider retry. The engine's
/// FINAL disposition for a refusal is always recorded as `Terminal`, so
/// settlement code asks this, never `!is_retryable()`.
pub fn is_settled(&self) -> bool {
    matches!(self.kind, ActivityErrorKind::Terminal)
}
```

Leave `is_retryable` exactly as it is. Say on **both** doc comments that the two
are not complements, or the next reader restores the bug.

**R1.2** Convert all seven sites. Each gets a comment stating the decision for
that site — not a shared blanket comment.

| # | file:line | symbol | change | the decision to write down |
|---|---|---|---|---|
| 1 | `crates/aion/src/durability/cursor.rs:343` | `HistoryCursor::resolve_activity` (fn `:297`) | `!is_retryable()` → `is_settled()` | Walk past. A refusal is one execution in an open trail, exactly as a retryable failure is. |
| 2 | `crates/aion/src/runtime/nif_collect_settlement.rs:339` | `recorded_terminal_event` (fn `:332`) | same | Not a member's terminal. First-match scan: it would settle a member that will hop AND shadow the real terminal behind it. |
| 3 | `crates/aion/src/runtime/nif_collect_settlement.rs:430` | `recorded_race_winner` (fn `:407`) | same | Cannot win a race. Its doc at `:399-406` binds it to site 2; move them together. |
| 4 | `crates/aion/src/durability/recorder/fan_out.rs:339` | `ordinal_is_resolved` (fn `:326`) | same | Ordinal not resolved. Its doc at `:318-325` binds it to site 2. **Path erratum — see §9 E-1.** |
| 5 | `crates/aion/src/runtime/nif_activity_retry.rs:349` | `activity_settled` (fn `:336`) | same | Loop keeps going. This is the guard `record_retry_event` re-reads under the recorder lock (`nif_activity_retry_dispatch.rs:570`); left two-valued, the loop stands itself down after recording its own refusal and the hop is never written. |
| 6 | `crates/aion/src/durability/executor.rs:277` | `ensure_terminal_activity_error` (fn `:276`, called `:209`) | `is_retryable()` → `!is_settled()` | Reject `PolicyRefused`. The guard asserts a failure really is terminal before AD records a terminal resolution; today a refusal passes. **Not in the brief's five — see §9 E-3.** |
| 7 | `crates/aion/src/durability/replay_inspect.rs:598` | `ensure_terminal_activity_error` (fn `:597`, called `:549`) | same | Reject `PolicyRefused`. Same guard on the mocked-fork replay path. **Not in the brief's five — see §9 E-3.** |

Widen sites 6 and 7's error messages to name the offending kind.

**R1.3** `crates/aion/src/durability/resolver.rs:222-235`
(`resolution_from_matched`) is **already** three-valued — it gates on
`matches!(error.kind, ActivityErrorKind::Terminal)`. **Do not change it.** Add a
test asserting its coupling with site 1: once the cursor walks past refusals,
this arm can never see one.

---

## 4. R2 — the AWL surface

### R2.1 Lexer

`crates/aion-awl/src/lexer/tokens.rs`: add `Keyword::Fallback`. `ALL` at `:230`
becomes `[Self; 58]`; add the `as_word` arm (`:315` region) and the `from_str`
arm (`:379` region).

🔴 **`crates/aion-awl/src/parser/stream.rs:423` — add `K::Fallback` to
`soft_keyword`, beside `Node | Timeout | Retry | Advisory | Agent`. Not
optional.** `fallback` is already an ordinary name in the committed corpus:
`outcome fallback:` at
`crates/aion-awl/tests/fixtures/rev2/step-bodies/valid/predicates_and_operators.awl:34`,
and in `.../step-bodies/invalid/all_non_bool.awl:11` and
`.../invalid/any_non_list.awl:10` (whose expected diagnostics would otherwise
change). Hard-reserving it breaks all three.

### R2.2 AST

`crates/aion-awl/src/ast/document.rs`: `ConfigLine` (`:506`) gains
`pub fallback: Option<FallbackSpec>` **after** `retry` (`:541`).

```rust
/// An ordered `fallback q1, q2, …` chain on an action config line.
///
/// Entries are TASK QUEUES, not model names, and the list is authored order.
/// There is deliberately no count and no bound here: the chain is exactly as
/// long as the author wrote it.
pub struct FallbackSpec {
    /// Span covering the complete `fallback` clause.
    pub span: Span,
    /// Queue entries in authored order, each with its own source span.
    pub entries: Vec<ConfigValue>,
}
```

Reuse the existing `ConfigValue` (`:544`) so every entry keeps a span for
diagnostics. Re-export from `ast/mod.rs:17`.

### R2.3 Parser

`crates/aion-awl/src/parser/workers.rs`:

- `opens_config` lookahead (`:265-277`) gains `Keyword::Fallback`.
- `parse_config_line` (`:291`) gains a `Fallback` arm beside the `Retry` arm
  (`:335-344`), with the same duplicate-key refusal shape.
- The three "expected a config key" diagnostics (`:347-352`, `:356-361`, and
  `crates/aion-awl/src/parser/action_body.rs:57` plus its module doc at `:16`)
  gain the word.

🔴 **The list-continuation rule, exactly.** The config line is itself
comma-separated (continuation at `workers.rs:361-364`). Disambiguation:

- A fallback entry is **`TokenKind::Identifier` only**. Do **not** use
  `expect_name` (`stream.rs:379-393`) — it accepts soft keywords as names, so
  `fallback norn, retry 3 every 1s` would parse `retry` as a second queue.
- After a comma inside the list: an identifier continues the list, a
  `TokenKind::Keyword(_)` ends it and is dispatched as the next config key.
- The rule is uniform for the first entry too, so it is one sentence, not two.
- **Disclosed limit:** a task queue whose name is a soft keyword (`node`,
  `timeout`, `retry`, `advisory`, `agent`, `fallback`, `filter`, `map`, `sort`,
  `count`, `every`, `backoff`, `empty`, `present`, `absent`) cannot be written
  in a fallback list. The diagnostic says exactly that and names the word — it
  must not say "expected an identifier".
- `fallback` with no entry is a **parse error** naming the missing queue, so the
  checker never sees an empty list.

### R2.4 Checker

Two rules, in `crates/aion-awl/src/checker`:

1. **Duplicate entries are refused** (artifact line 207). Exact string equality;
   anchor the diagnostic on the SECOND occurrence's span and name the first.
2. **`fallback` is declaration-only**, like `retry`. Add an `else if` arm at
   `crates/aion-awl/src/checker/calls.rs:66-106`, beside the `retry` arm
   (`:98-105`), in the existing sentence shape: *"a call site may pin `node` and
   `timeout` only — `fallback` stays on the action declaration; the chain is the
   action's declared routing, which one call site cannot change."*

🔴 **Do NOT check** that an entry names a `worker` block declared in this
document: a queue may be served by an out-of-band worker SDK, which the tree
makes a first-class permanent case (`ast/document.rs:405-409`). 🔴 **Do NOT
refuse** an entry equal to the declaring worker's own name: the artifact makes
`fallback norn, claude` with an initial queue of `norn` the named example
(line 211) and rules it a **runtime skip**, not a check-time refusal.

### R2.5 Printer

`crates/aion-awl/src/printer/document.rs:484-503` renders the config line as
comma-joined parts in a fixed order (`advisory`, `agent`, `node`, `timeout`,
`retry`). Append `fallback` **after `retry`**, rendered `fallback a, b, c`.
`aion awl fmt` idempotence and `crates/aion-awl/tests/canonical_emission.rs` are
the gates.

### R2.6 Both lowerings — they must stay in step

The tree already states the mirror obligation twice
(`emitter/stmts.rs:161-162`, `mir/lower/activity.rs:281-286`). Update both doc
comments in the same edit.

**Reference emitter** — `crates/aion-awl/src/emitter/stmts.rs`. Beside
`retry_policy` (`:152`), add `fallback_chain` producing
`activity.fallback(<act>, ["norn", "claude"])`. Apply it in the per-key merge at
`stmts.rs:84-104`.

**Direct/MIR lowering** — `crates/aion-awl/src/mir/lower/activity.rs`.
`apply_action_config` (`:195`) applies keys in order: `retry` (`:205-217`),
`timeout` (`:218-229`), `task_queue` (`:231-240`), `node` (`:241-252`). Apply
`fallback` **after `retry`, before `timeout`**, so MIR order matches the
reference merge order. Update the doc at `:189-194`.

`crates/aion-awl/src/mir/runtime.rs` gains `RuntimeFn::ActFallback`: declaration
beside `ActRetry` (`:40-43`); `Self::ActFallback => ("aion@activity",
"fallback", 2)` (`:160-163`); `DurableFamily::Activities` (`:231-234`); the
`all_runtime_fns` row (`:270-273`); and a new ordinal **appended at the end** of
the `variant_ordinal` staleness anchor (`:378-381` region), never inserted.

The list lowers as a BEAM list of binaries. No new MIR `Stmt` kind.

🔴 **No `total_attempts` analogue.** There is no count to translate. This is
"no numeric retry bound" at the lowering layer.

### R2.7 Gleam SDK

`gleam/aion_flow/src/aion/activity.gleam`: the opaque `Activity(i, o)` record
(`:69-83`) gains `fallback: List(String)` — empty list means no chain, matching
the SDK's "absence is intentional data" convention (`:85-89`).

🔴 Gleam records are rebuilt field-by-field in **every** builder. All nine must
gain the field: `new` (`:97`), `retry` (`:118`), timeout (`:137`), heartbeat
(`:158`), labels (`:186`), `task_queue` (`:218`), `node` (`:247`),
`execution_tier` (`:278`), and the new `fallback` builder. Add a
`selected_fallback` accessor beside `selected_task_queue` (`:348`). Missing one
is a compile error — but there are nine and they are easy to half-finish.

`gleam/aion_flow/src/aion/internal/activity_dispatch.gleam::config` (`:209-239`)
gains `#("fallback", …)`: a JSON array, or `json.null()` for the empty list, so
both engine conventions ("absent means no chain", "explicit null means no
selection", `nif_activity.rs:105-107`) hold.

`crates/aion/tests/sdk_internal_privacy.rs` gates the SDK's public surface;
update it with the reason for the new export.

### R2.8 Editor vocabulary — a hard gate

`crates/aion-awl/src/lexer/grammar_vocabulary_tests.rs` `include_str!`s
`tools/tree-sitter-awl/grammar.js` at compile time and compares its `const
keywords` (`grammar.js:33-43`) against `Keyword::ALL`. **Adding
`Keyword::Fallback` fails that test** until `'fallback'` is added to the grammar
array. Then
`apps/aion-ops-console/src/features/authoring/lib/highlight-assets.test.ts`
fails until `apps/aion-ops-console/src/features/authoring/assets/awl.wasm` is
rebuilt from the source grammar. `editors/zed-awl` and `editors/nvim-awl` back
onto the same grammar. All four are in scope.

---

## 5. R3 — the event and the recorder

### R3.1 `Event::ActivityFallbackRouted`

`crates/aion-core/src/event.rs`, after `ActivityAdvisoryExhausted` (`:491-502`)
— the closest structural precedent (activity-scoped, non-terminal,
status-invisible):

```rust
ActivityFallbackRouted {
    envelope: EventEnvelope,
    activity_id: ActivityId,
    /// One-based execution attempt whose policy refusal caused this hop.
    attempt: u32,
    /// Queue the refusing execution was dispatched on.
    from_task_queue: String,
    /// Queue the next execution is dispatched on. THE durable answer —
    /// recovery reuses this and never chooses again.
    to_task_queue: String,
    /// ZERO-BASED position consumed from the AUTHORED list. The ORIGINAL
    /// authored index, not the post-skip index: a skipped same-queue entry
    /// still owns its position.
    fallback_index: u32,
}
```

No `#[serde(default)]` on any field — the variant is new, so no recorded history
can lack them.

### R3.2 Every exhaustive match. This list is complete for non-test code.

| file:line | arm | decision |
|---|---|---|
| `crates/aion-core/src/event.rs:734` region | `Event::envelope()` | — |
| `crates/aion-core/src/status.rs:82`, `:87`, `:141`, `:146` | status projection | **non-terminal, advances nothing**; sits with `ActivityAdvisoryExhausted` |
| `crates/aion-core/src/current_step.rs:100-136`, module doc `:15-32` | current-step projection | **advances nothing**; the hop does not open a step, the `ActivityStarted` after it does |
| `crates/aion-server/src/stream/selector.rs:138`, `:143` | stream family | activity family |
| `crates/aion/src/durability/cursor.rs:632` region | `event_kind` | — |
| `crates/aion/src/durability/replay_inspect.rs:698` region | `event_kind` | — |
| `crates/aion/src/durability/resolver.rs:287`/`:291`, `:327`/`:330` | family + `event_kind` | not a command-issued resolution |
| `crates/aion/src/engine/delegated.rs:474`, `:477` | delegated fan-out | — |
| `crates/aion/src/lifecycle/continue_as_new.rs:164`, `:200` | carry-over | groups with `ActivityAdvisoryExhausted` at `:200` — not carried |
| `apps/aion-ops-console/src/features/workflow-detail/lib/timeline.ts:63`, `:138`, `:468` | timeline row + kind union | activity row |
| `apps/aion-ops-console/.../lib/timelineVariants.ts:86`, `:199` | variant table | its `:237` comment records the last drift |
| `apps/aion-ops-console/src/types/generated/index.ts` | — | 🔴 **REGENERATED, never hand-edited** |

`Event` is already registered for TS export
(`crates/aion-core/src/generated_types.rs:231`) — regenerate only, add no
registration. `Event` does not cross the proto wire; `crates/aion-proto` is
untouched.

### R3.3 Recorder method — the only append path

`crates/aion/src/durability/recorder.rs`, beside
`record_activity_advisory_exhausted` (`:744-758`):

```rust
pub async fn record_activity_fallback_routed(
    &mut self,
    recorded_at: DateTime<Utc>,
    activity_id: ActivityId,
    attempt: u32,
    from_task_queue: String,
    to_task_queue: String,
    fallback_index: u32,
) -> Result<(), DurabilityError>
```

🔴 The hop is written by a new
`RetryRecord::FallbackRouted { … }` variant on the enum at
`crates/aion/src/runtime/nif_activity_retry_dispatch.rs:513-520`, appended via
`record_retry_event` (`:538`) **and nothing else. Never `EventStore::append`.**

`record_retry_event` already performs, under `seam.recorder.lock()` and in this
order: the epoch gate (`:557`, `is_epoch_open`), the run-segment scope (`:566`,
`current_run_segment`), and the settlement re-read (`:570`, `activity_settled`).

🔴 **Add the third guard the artifact demands:** in the `FallbackRouted` arm,
scan the run segment for an existing `ActivityFallbackRouted` with the same
`activity_id` **and** `attempt`, and return `RetryRecordOutcome::Settled` if one
exists. Without it, a detached completion task that lost a race writes a second
hop for one refusal and recovery derives the wrong list position from the count.

`recorder.rs` is **1995 lines at base**. It may gain only this one method and
its doc; disclose that in the report.

---

## 6. R4 — the retry loop

`crates/aion/src/runtime/nif_activity_retry_dispatch.rs` and
`crates/aion/src/runtime/nif_activity_retry.rs`.

**R4.1 The reason predicate.** In `nif_activity_retry.rs`, beside
`RETRYABLE_REASON_PREFIX` (`:32`), `is_parked_reason` (`:57`) and
`is_worker_lost_reason` (`:88`):

```rust
const POLICY_REFUSED_REASON_PREFIX: &str = "policy_refused:";
pub(super) fn is_policy_refused_reason(reason: &str) -> bool { … }
```

🔴 **Leave `is_retryable_reason` (`:96`) alone.** A `policy_refused:` reason is
genuinely not `retryable:`-class; widening that predicate routes refusals
through the ordinary budget, which is exactly what "hop first" forbids. Amend
its doc at `:91-95` to name the new class as handled ahead of it.

**R4.2 The refusal arm.** In `dispatch_with_retries` (`:247`), in the loop body
**after** the worker-loss arm (`:282-287`) and **before** `failure_stand_down`
(`:288`) — the same position and shape the worker-loss class already occupies.
Three outcomes:

- **Chain live** → record nonterminal refusal → record hop → bump execution
  attempt → record `ActivityStarted` → `continue` on the new queue.
- **Chain declared and exhausted** → record nonterminal refusal → return
  `RetryLoopTerminal::Failed(<policy_refused: … recorded queue sequence …>)`.
  🔴 A declared chain's exhaustion is TERMINAL; it does **not** then fall back
  to same-provider retry. Artifact R1-d step 4 vs step 5: step 5 applies only
  "if no fallback chain was declared".
- **No chain declared** → record nonterminal refusal, then fall through to the
  ordinary retry path with the refusal treated as retry-eligible (R4.3).

**R4.3 `failure_stand_down` (`:183`).** Treat the policy-refusal class as
retry-eligible **only in the no-chain case**, and make its exhaustion message
begin `policy_refused:`. Keep the existing
`"exhausted: spent {attempt} of {max} attempts; last refusal: {reason}"`
(`:197-200`) for `retryable:`-class exhaustion and add a
`policy_refused:`-prefixed twin for refusals.

🔴 Artifact R1-d step 5: *"do not relabel it `exhausted:` or an engine fault."*
🔴 **No numeric bound is minted** — `policy.max_attempts` comes from
`retry_policy_from_config` (`nif_activity_retry.rs:280-315`) off the act's own
`RetrySpec`.

**R4.4 Refusal records carry `PolicyRefused`.** `RetryRecord::AttemptFailed`
(`:515`) hardcodes `ActivityErrorKind::Retryable` at `:580`. Artifact R1-d step
1 (lines 216-217) and step 5 (line 226) both require the nonterminal refusal
record to carry `ActivityErrorKind::PolicyRefused`. Pass the kind explicitly (or
add a sibling variant) — never default it.

**R4.5 Same-provider retry accounting, separated from execution identity.**
Artifact R1-d lines 235-241: `ActivityStarted.attempt` stays a monotonically
increasing identity for **every** execution including a hop, and the authored
budget must no longer be inferred from it.

```
same_provider_attempt = execution_attempt − hops_recorded_for_this_ordinal
```

where the subtrahend counts `ActivityFallbackRouted` events for the ordinal in
the current run segment. Give `failure_stand_down` the
`same_provider_attempt`; keep `execution_attempt` on
`ActivityStarted`/`ActivityFailed`. A hop increments the execution attempt and
not the budget attempt — "a queue hop is an execution but is not a
same-provider retry."

Derive the base count once from the seam's history at loop entry (the same read
`activity_settled_elsewhere` (`:440`) already performs) and track both counters
in-loop thereafter, so a loop restarted after a crash resumes with the right
budget.

**R4.6 The delivery's queue.** The loop builds each delivery as
`let mut delivery = request.clone(); delivery.attempt = attempt;` (`:261-262`).
Add a loop-local `current_queue`, initialised from `request.task_queue`,
reassigned on a hop, and stamped as `delivery.task_queue = current_queue.clone()`.
🔴 **Do not modify `ActivityDispatch`** (`crates/aion/src/activity/bridge.rs:29-73`).

**R4.7 Who records the FINAL terminal: nobody in the loop.** The loop returns
`RetryLoopTerminal::Failed(reason)`; `spawn_completion_task` (`:51-60`) delivers
it; the await seam records it. `take_runtime_completion`
(`crates/aion/src/runtime/nif_activity_await.rs:207-226`) calls
`activity_error(error.message)` (`crates/aion/src/runtime/nif_activity.rs:232-238`),
which builds `ActivityErrorKind::Terminal` with the message verbatim — so the
final event is `Terminal` with a `policy_refused:` message, exactly as artifact
R1-c (lines 197-201) and R1-d step 4 require. 🔴 **This is already true. Verify
it by test; do not edit the await seam.**

---

## 7. R5 — routing derivation, recovery, and the cursor

### R5.1 New module `crates/aion/src/runtime/nif_activity_fallback.rs`

All of L3's routing derivation lives here, so the already-oversized files do not
grow.

- `fallback_chain_from_config(config: &str) -> Vec<String>` — decodes
  `config.fallback`. Malformed JSON, absent, `null`, or non-array-of-string ⇒
  empty chain (no hop), mirroring `labels_from_config`/`resolve_task_queue`'s
  documented "never fail a dispatch over routing metadata" stance
  (`nif_activity.rs:100-107`) — with a `tracing::warn!`, **never a silent drop**.

  🔴 This function's doc comment carries the immutability argument: the config
  string is produced by the BEAM module the run is bound to
  (`ActivityCall.config`, `nif_activity_dispatch.rs:168`, decoded at
  `decode_dispatch_args` `:133-141`), each `.aion` version is a distinct
  content-hash-named immutable module, and **recovery on this tree is replay** —
  `open_this_delivery` (`:246`) is reached by re-executing workflow code from the
  bound module. So the authored list is the run's own, always. State why the
  "read it off the current deploy" reading is NOT taken: it is identical in every
  test and diverges only for a workflow mid-hop when a new version lands, which
  is the invisible long-lived replay break the artifact names (lines 330-336).

- `next_hop(chain, initial_queue, current_queue, consumed) -> Option<(usize, String)>`
  — pure. Walk from position `consumed`; **skip** any entry equal to the current
  queue (artifact lines 208-211); return the first distinct queue with **its
  original authored index**. Total, deterministic, no clock, no fleet state.
- `recorded_hops(history, activity_id) -> Vec<&Event>` and
  `recorded_hop_queue(history, activity_id) -> Option<String>` (the latest
  `to_task_queue`, or `None`).
- `refused_queue_sequence(history, activity_id, initial_queue) -> Vec<String>` —
  the recorded initial queue followed by each hop's `to_task_queue` in recorded
  order. 🔴 **This is what the terminal `policy_refused:` message lists** —
  artifact line 302-303: "by the recorded sequence, not by recomputing what
  might have happened."

### R5.2 🔴 The re-dispatch queue override — the sharpest seam in this leg

On recovery, workflow code re-executes and `dispatch_activity_with_context`
(`crates/aion/src/runtime/nif_activity_dispatch.rs:301`) re-resolves the queue at
`:350` via `resolve_task_queue(&call.config, start_time_task_queue)` — yielding
the **INITIAL** queue, not the hopped-to one — then stamps it onto both the
recorded `ActivityStarted` (`open_this_delivery` → `record_started`, `:281-295`)
and the live `ActivityDispatch` (`:385-397`).

**Left as-is, a crash after a hop re-dispatches on the refusing queue and
history records it. That is the exact replay break R1-e exists to prevent.**

Fix, immediately after `:350` — deliberately **outside** `resolve_task_queue`:

```rust
let task_queue = super::nif_activity::resolve_task_queue(
    &call.config, start_time_task_queue.as_deref());
// R1-e: a recorded hop is a POST-FAILURE OVERRIDE of the recorded initial
// choice, never a fourth precedence level inside resolve_task_queue. The
// override reads history, so replay re-derives it identically.
let task_queue = super::nif_activity_fallback::recorded_hop_queue(
    context.history(), &activity_id).unwrap_or(task_queue);
```

🔴 `resolve_task_queue` (`crates/aion/src/runtime/nif_activity.rs:108`) and
`config_task_queue_selection` (`:124`) are **NOT modified**. Their four
documented levels (`:77-107`) stand exactly as they are.

Apply the same override on the fan-out paths, which resolve the queue at three
sites in `crates/aion/src/runtime/nif_collect.rs`: `fan_out_items:401`,
`fresh_scheduled_activity:431`, and `fan_out_items_recovered:450`. The last
already reads the queue back from the recorded `ActivityScheduled` (NSTQ-3) and
is the established precedent — the hop simply takes precedence over that read
there. Mirror `nif_collect_settlement.rs::scheduled_task_queue:461`.

🔴 **`Event::ActivityScheduled` is NOT re-recorded on a hop.** The artifact's
order lists only `ActivityFallbackRouted` then `ActivityStarted`, and
`ActivityStarted` carries no `task_queue` field — which is precisely why the hop
needs its own event. `resolve_node` (`nif_activity.rs:159`) is untouched: node
affinity is within-pool and a hop changes the pool.

### R5.3 The durable order and its five crash boundaries

Artifact R1-e lines 277-283:

1. record the policy-refused `ActivityFailed` for the current queue;
2. derive the next list position from the recorded initial queue plus prior
   `ActivityFallbackRouted` events and the immutable authored list;
3. record `ActivityFallbackRouted` before the next dispatch;
4. record the next `ActivityStarted`;
5. put the delivery on the recorded `to_task_queue`.

Steps 1, 3, 4 are appends through `record_retry_event`; step 2 is pure; step 5
is the loop's next iteration. Boundaries: **before 1**, **between 1 and 3**,
**between 3 and 4**, **between 4 and 5**, **after 5**.

- **Before 1** — nothing recorded; the trail dangles. Replay exhausts and
  re-dispatches live on `recorded_hop_queue`'s answer (or the initial queue),
  at the next attempt via `next_delivery_attempt`. Unchanged.
- **Between 1 and 3** — refusal recorded, no hop. Step 2 re-derives from the
  same inputs and reaches the same answer (artifact line 285).
- **Between 3 and 4** — 🔴 recovery **reuses** `to_task_queue`. It does not
  derive again and does not consult live worker availability (artifact lines
  286-287). This is the boundary the event exists for.
- **Between 4 and 5** — hop and start recorded, attempt dangling.
  `dangling_attempt` (`nif_activity_retry.rs:379`) already handles this shape;
  the queue comes from the recorded hop.
- **After 5, before any terminal** — if recovery derives that no distinct hop
  and no configured same-provider retry remains, it produces the same
  `RetryLoopTerminal::Failed` disposition with a `policy_refused:` message and
  the await seam records the `Terminal`. The ordinal must not dangle and no
  extra execution may be invented (artifact lines 288-293).

### R5.4 Cursor span

`HistoryCursor::resolve_activity` (`crates/aion/src/durability/cursor.rs:297-367`)
walks the span by `activity_id` and skips everything else in place
(`_ => {}` at `:361`). Two changes:

1. Site 1's predicate (R1.2) — walk past a `PolicyRefused` failure exactly as
   past a `Retryable` one.
2. 🔴 **Consume `ActivityFallbackRouted` into the span** — a new match arm
   pushing to `matched`, beside the `ActivityStarted` arm (`:308-313`). Artifact
   lines 295-297 require it. Left to the `_ => {}` skip, a straggler hop event
   stays matchable by a later command.

`replay_inspect.rs` walks recorded history directly with no natives; beyond site
7 it needs only its `event_kind` arm (`:698` region).

---

## 8. R6 — tests. The artifact's list is binding.

**T1 — declared chain.** New `crates/aion/tests/fallback_chain_e2e.rs`.
Precedent: `crates/aion/tests/activity_retry_e2e.rs` (350 lines) — real embedded
engine, in-process `ActivityDispatcher` stub, packaged example rebuilt from
committed source. Use a `RefusingDispatcher` returning `policy_refused:…` on
queue A and succeeding on queue B.

🔴 **Audit the claim from HISTORY, not from the dispatcher's call log** — that is
what the artifact means by auditable (lines 302-305). Assert: exactly one
`ActivityFailed` with `kind == PolicyRefused`; exactly one
`ActivityFallbackRouted` with `from_task_queue == A`, `to_task_queue == B`,
`fallback_index == 0`; **zero** second `ActivityStarted` on queue A; terminal
`ActivityCompleted`; workflow `Completed` with an empty final failure reason.

**T2 — no chain.** Same harness, always-refusing dispatcher, an act with
`retry N` and no `fallback`. Assert: **zero** `ActivityFallbackRouted`; every
interim `ActivityFailed` carries `kind == PolicyRefused`; the terminal
`ActivityFailed` carries `kind == Terminal` with a message starting
`policy_refused:`; the message does **not** start `exhausted:`.

🔴 **Do not assert a particular attempt count** (artifact line 363) — asserting
one mints the numeric bound the design deliberately withholds. Assert
`attempts >= 2` only, to prove retry happened at all.

**T3 — replay after EACH crash boundary chooses the same recorded queue.** All
five boundaries in R5.3, **one case each** — not a single "crash somewhere"
test. Precedents: `crates/aion/tests/engine_reload_recovery.rs` (a crash-window
sweep over a real store) and `crates/aion/tests/recovery_restart.rs`. Each case
drives to the boundary, drops the engine, rebuilds on the same `InMemoryStore`,
and asserts the re-dispatch's queue equals the recorded one and that no
duplicate `ActivityFallbackRouted` appears.

🔴 The "between 3 and 4" case is load-bearing: it must show the queue coming
**from the event**, which means choosing an authored list and an initial queue
such that a fresh derivation would pick differently if the event were ignored.

**T4 — settlement predicate, per site.** Seven focused unit tests, one per site
in R1.2, each feeding a history whose only failure is a nonterminal
`PolicyRefused` and asserting the site does not settle (1-5) or rejects it
(6-7). Plus the coupling test for `resolution_from_matched` (R1.3). These are
the ruling's teeth: a site silently reverted fails exactly one of these.

**T5 — AWL surface.** Parse/print round-trip of `fallback a, b`; `aion awl fmt`
idempotence; duplicate-entry refusal anchored on the second span; call-site
`fallback` refusal; empty-list parse refusal; the soft-keyword collision
diagnostic; and 🔴 `fallback norn, retry 3 every 1s` parsing as **two keys, not
three queues**. Fixtures under `crates/aion-awl/tests/fixtures/rev2/`, following
the existing valid/invalid layout. Confirm the three existing `fallback`-as-name
fixtures (R2.1) still pass unchanged.

**T6 — both lowerings agree.** Precedent:
`crates/aion/tests/awl_retry_counting_e2e.rs`, which runs one document through
the reference emitter and the direct MIR lowering against a real engine and
asserts identical behaviour. The fallback twin asserts both backends produce the
same `config.fallback` array and the same hop sequence.

**T7 — the immutability premise, made visible.** A run started on module version
v1 (authored `fallback a, b`), a v2 of the same logical workflow deployed
mid-run with a **different** list, then a crash and recovery: the recovered run
hops per **v1**. Precedent and fixture:
`crates/aion/tests/engine_reload_recovery.rs` + `common/reload_fixture.rs`,
which already builds two content-hash-distinct versions of one workflow. This is
the "invisible until the moment it matters" defect (artifact lines 330-336).

**Test discipline.** 🔴 Any live-seat test is runtime-gated on an env var, logs a
`tracing::info!` skip line, and returns `Ok(())`. **No `#[ignore]`, in any
spelling.** 🔴 No blocking sleep sized to an expected duration: poll under a
total deadline, with timeout represented as a **distinct outcome**, not a failed
assertion.

---

## 9. Errata — report, do not stop

**E-1 — brief path error.** `.fleet/BRIEF.md:27` names settlement site 4 as
`crates/aion/src/runtime/fan_out.rs:339`. No such file exists. The symbol is
`ordinal_is_resolved` at **`crates/aion/src/durability/recorder/fan_out.rs:339`**
(fn at `:326`). Line exact, directory wrong.

**E-2 — artifact symbol error.** Artifact R1-d line 204 says the list goes
"beside `retry` on `ActionConfig`". There is no `ActionConfig` in
`crates/aion-awl`. The referent is
`crates/aion-awl/src/ast/document.rs::ConfigLine` (struct `:506`, `retry` field
`:541`). Same class as the already-ruled `outcome_into_payload` /
`run_result_to_outcome` erratum (`.fleet/BRIEF.md:62-65`).

**E-3 — the brief's "five sites" undercounts by two.** The three-valued question
is asked at **seven** places. Five spell it `!error.is_retryable()`; two spell it
`if error.is_retryable() { Err }` and are identical in kind. They are sites 6
and 7 in R1.2 and are in scope, because the artifact's rule is unqualified
(lines 182-184): *"settlement/cursor code gains an explicit terminal predicate
under which only `Terminal` settles an activity."* Leaving them admits a
nonterminal `PolicyRefused` as a recorded terminal resolution — the same defect
as the five, in the opposite direction.

**Known artifact erratum, already ruled by L1 counsel OPEN-1 ("report, do not
stop"):** the norn secondary-site name `outcome_into_payload` does not exist;
the referent is `run_result_to_outcome`. Not an L3 work item — recorded so you
do not re-derive the confusion.

---

## 10. Process walls

- 🔴 **No lint suppressions in any spelling** — no `#[allow]`, `#[expect]`,
  `#[deny]`, `#[ignore]` on tests, `_var` renames, or `#[cfg(any())]`
  dead-code hiding. If clippy fires, fix the code.
- `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo fmt --check` must both pass clean before any commit.
- 🔴 **No hand-edited generated types.** Regenerate
  `apps/aion-ops-console/src/types/generated/index.ts`.
  `crates/aion-proto/src/worker.rs` and the `.proto` are hand-authored surfaces
  by design and are **not** touched by this leg.
- 🔴 **No new dependencies.**
- Files already over 500 lines at base may gain **only what the compiler
  demands**, disclosed in the report (L2's precedent). Known at base:
  `crates/aion/src/durability/recorder.rs` (1995),
  `crates/aion/src/durability/cursor.rs` (1715),
  `crates/aion/src/runtime/nif_activity_retry.rs` (905),
  `crates/aion/src/runtime/nif_collect_settlement.rs` (531). New routing logic
  goes in `crates/aion/src/runtime/nif_activity_fallback.rs`, not into these.
- 🔴 Report at the workflow's injected wire path, with the `git ls-tree` receipt
  **PASTED INTO the report body**.
- Where the artifact and this brief disagree, follow the artifact and **say so
  in your report**.
