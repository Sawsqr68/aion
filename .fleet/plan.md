# PLAN — policyref-l3-route: authored fallback and routing (leg 3 of 3)

Planning counsel, 2026-08-22. Tree: `fleet/policyref-l3-base` @ `c0c04142`.
Standing on L2's ratified result (`fleet/policyref-l2-result` = `68568af4d`),
which is **landed in this tree** and is consumed, never re-derived.

Governing documents, in force in this order:

1. `docs/design/policy-refused-fallback.md` — **the artifact. It wins every
   disagreement**, including with `.fleet/BRIEF.md` and with this plan.
2. `.fleet/BRIEF.md` — the L3 dispatch brief; both binding counsel findings are
   carried in it verbatim and are reproduced verbatim below.
3. This plan — coordinates and seam decisions, re-derived on THIS tree.

Every file:line below was re-derived by symbol on this tree by reading the
file. Where the brief's audit-time coordinate differs, both are named.

---

## 0. Errata against the brief and the artifact — report, do not stop

Three. None is an L3 work item; all are recorded so the builder does not
re-derive the confusion.

**E-1 — brief path error, `fan_out.rs`.** `.fleet/BRIEF.md:27` names settlement
site 4 as `crates/aion/src/runtime/fan_out.rs:339`. There is no such file. The
symbol lives at **`crates/aion/src/durability/recorder/fan_out.rs:339`**
(`ordinal_is_resolved`, fn at `:326`). The line number is exact; only the
directory is wrong. `find crates/aion/src -name "fan_out*"` returns
`durability/recorder/fan_out.rs` and its `_tests.rs` sibling and nothing else.

**E-2 — artifact symbol error, `ActionConfig`.** Artifact R1-d line 204 says the
`fallback` list goes "beside `retry` on `ActionConfig`". There is no
`ActionConfig` anywhere in `crates/aion-awl`. The referent is
**`crates/aion-awl/src/ast/document.rs::ConfigLine`** (struct at `:506`, its
`retry: Option<RetrySpec>` field at `:541`). Same class of erratum as the
already-ruled `outcome_into_payload` / `run_result_to_outcome` one
(`.fleet/BRIEF.md:62-65`), and handled the same way.

**E-3 — the brief's "five sites" undercounts by two.** The three-valued
settlement question is asked at **seven** places on this tree, not five. Five
spell it `!error.is_retryable()`; two spell it `if error.is_retryable() { Err }`
and are identical in kind — both are "this failure is about to be treated as an
activity TERMINAL, assert it really is one". They are named as sites 6 and 7 in
§1 and are in scope, because the artifact's own words are the governing rule and
they are unqualified: *"settlement/cursor code gains an explicit terminal
predicate under which only `Terminal` settles an activity"* (artifact line
182-184). Leaving 6 and 7 on the two-valued read admits a nonterminal
`PolicyRefused` as a recorded terminal resolution — the same defect as the five,
in the opposite direction.

---

## 1. Binding counsel finding 1 — THE SETTLEMENT TERMINAL PREDICATE

> 🔴 THE SETTLEMENT TERMINAL PREDICATE (L2 counsel OPEN-2, ruled):
> "L2 must not implement it; L3 cannot omit it."
> A `policy_refused:` reason is NOT retryable — but with a live fallback
> chain it is NOT SETTLED either. Every one of those five sites must decide
> the three-valued question on purpose; leaving any on the two-valued read
> silently settles a mid-chain refusal.

### 1.1 Why the two-valued read is now wrong — measured on this tree

`crates/aion-core/src/error.rs:33-40`:

```rust
impl ActivityError {
    pub fn is_retryable(&self) -> bool {
        matches!(self.kind, ActivityErrorKind::Retryable)
    }
}
```

`ActivityErrorKind` (`:9-17`) is now three-valued — L2 landed `PolicyRefused`
at `:14`. So `is_retryable()` returns **false** for `PolicyRefused`, exactly as
the artifact requires (line 182: *"`ActivityError::is_retryable()` stays
false"*). Every site that reads `!is_retryable()` as "settled" therefore reads a
recorded mid-chain `PolicyRefused` as an activity TERMINAL, today, on this tree.
That is the silent settlement the ruling names.

### 1.2 The predicate

Add ONE function beside `is_retryable`, in `crates/aion-core/src/error.rs`:

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

`is_retryable` is left exactly as it is. The two are not complements and the doc
comment on each must say so, or the next reader restores the bug.

### 1.3 The seven sites, each decided on purpose

Every one is `!error.is_retryable()` → `error.is_settled()` (sites 1–5) or
`error.is_retryable()` → `!error.is_settled()` (sites 6–7). The mechanical edit
is trivial; the *decision* is what each site is for, and the builder writes that
decision into each site's comment.

| # | file:line (this tree) | symbol | brief's audit coord | decision |
|---|---|---|---|---|
| 1 | `crates/aion/src/durability/cursor.rs:343` | `HistoryCursor::resolve_activity` (fn `:297`) | `cursor.rs:343` ✔ exact | **Walk past.** A `PolicyRefused` `ActivityFailed` is one execution in an open trail, exactly as a `Retryable` one is. Stopping here resolves the command against a mid-chain refusal and hands workflow code a superseded attempt's error. |
| 2 | `crates/aion/src/runtime/nif_collect_settlement.rs:339` | `recorded_terminal_event` (fn `:332`) | `:339` ✔ exact | **Not a member's terminal.** First-match scan: a refusal record would both settle a member that will hop AND shadow the genuine terminal behind it — the two failure modes aion#47 names, in a third spelling. |
| 3 | `crates/aion/src/runtime/nif_collect_settlement.rs:430` | `recorded_race_winner` (fn `:407`) | `:430` ✔ exact | **Cannot win a race.** Its own doc (`:399-406`) requires both scans to agree: "a race settled from a record the settlement scan walks past is the divergence in a second spelling." Site 2 and site 3 must move together. |
| 4 | `crates/aion/src/durability/recorder/fan_out.rs:339` | `ordinal_is_resolved` (fn `:326`) | `runtime/fan_out.rs:339` — **E-1, path wrong, line exact** | **Ordinal not resolved.** Its doc (`:318-325`) states the mirror obligation against site 2 explicitly. A refusal record that resolved the ordinal here would drop the durable write for the hop's eventual completion. |
| 5 | `crates/aion/src/runtime/nif_activity_retry.rs:349` | `activity_settled` (fn `:336`) | `:349` ✔ exact | **Loop keeps going.** This is the guard the retry loop re-reads under the recorder lock before every append (`nif_activity_retry_dispatch.rs:570`). If a refusal settled here, the loop would stand ITSELF down after recording its own refusal, and the hop would never be written. This is the site whose two-valued read is most directly fatal. |
| 6 | `crates/aion/src/durability/executor.rs:277` | `ensure_terminal_activity_error` (fn `:276`), called `:209` from the `LiveActivityOutcome::Failed` arm | **not in brief — E-3** | **Reject `PolicyRefused`.** The guard exists so AD cannot record a terminal resolution for a non-terminal failure. `PolicyRefused` is non-terminal; today it passes. Invert to `!error.is_settled()`, and widen the message to name the kind. |
| 7 | `crates/aion/src/durability/replay_inspect.rs:598` | `ensure_terminal_activity_error` (fn `:597`), called `:549` from the `MockOutcome::ActivityFailed` arm | **not in brief — E-3** | **Reject `PolicyRefused`.** Same guard for the mocked-fork replay path. A test that mocks a policy refusal at a fork must be told it has not described a terminal, not silently given one. |

### 1.4 One site is already three-valued — verify, do not change

`crates/aion/src/durability/resolver.rs:222-235` (`resolution_from_matched`)
already gates on `matches!(error.kind, aion_core::ActivityErrorKind::Terminal)`
and returns `DurabilityError::HistoryShape` for anything else. It is correct as
written **and it is a coupled invariant with site 1**: it can only ever see a
`PolicyRefused` as the matched `last` event if the cursor stopped on one. Site 1
fixed ⇒ this arm is unreachable for refusals. The builder adds a test asserting
that coupling, and does not touch the code.

---

## 2. Binding counsel finding 2 — `is_retryable_reason`, confirmed

> 🔴 CONFIRMED BY L2's JUDGE (hand-off note): `is_retryable_reason`
> (`nif_activity_retry.rs:96-98` audit-time) still tests only the
> `retryable:` prefix, so on the tree you stand on a `policy_refused:`
> reason settles without retry or hop. Artifact R1-d step 5 ("no-chain →
> existing configured retry policy, refusal stays nonterminal while budget
> remains, terminal on exhaustion with `policy_refused:` reason — never
> relabeled") plus hop-first-retry-second is YOURS to make true.

**Re-confirmed on this tree.** `crates/aion/src/runtime/nif_activity_retry.rs:96`:

```rust
pub(super) fn is_retryable_reason(reason: &str) -> bool {
    reason.starts_with(RETRYABLE_REASON_PREFIX)   // ":32" — "retryable:"
}
```

and `failure_stand_down` (`nif_activity_retry_dispatch.rs:183`, decision at
`:194-202`):

```rust
let retry_eligible = expired || is_retryable_reason(reason);
match policy {
    Some(policy) if retry_eligible && attempt < policy.max_attempts => None,
    Some(policy) if retry_eligible => Some(Failed(format!("exhausted: …"))),
    _ => Some(Failed(reason.to_owned())),          // ← a policy_refused: reason lands HERE
}
```

A `policy_refused:` reason falls to the `_` arm and is delivered as a terminal
failure at the first refusal — no retry, no hop. Confirmed.

The bridge really does produce that prefix: `crates/aion-server/src/worker/
bridge.rs:476-481` maps `ActivityErrorKind::PolicyRefused => "policy_refused"`
and formats `"{prefix}:{message}"`. The in-VM SDK spells the same string at
`gleam/aion_flow/src/aion/internal/activity_dispatch.gleam:191`. So the prefix
is on the wire from both tiers and is the string L3 routes on.

---

## 3. Decisions

### D1 — the authored list rides the dispatch `config` JSON, not the package contract

**Decision.** The fallback list is carried in the activity dispatch `config`
JSON as a `"fallback": ["<queue>", …]` array (JSON `null`/absent = no chain),
built by the run's own workflow module. It is **not** added to
`aion_package::ActionContract`.

**Why this satisfies the immutability premise, structurally.** Artifact
§"Why the authored list is immutable for a run" (lines 313-336) requires the
list be read from *the run's own bound module version, never the currently
deployed package*. `config` reaches the engine as `ActivityCall.config`
(`crates/aion/src/runtime/nif_activity_dispatch.rs:168`), decoded from the NIF
arguments at `decode_dispatch_args` (`:133-141`) — i.e. it is a value produced
by the BEAM module the run is executing. Each `.aion` version is a distinct,
content-hash-named module and a run is bound to the one it started with, so the
string cannot change under a running workflow. **Recovery is replay on this
tree** — there is no separate recovery re-dispatcher with its own config source;
`open_this_delivery` (`:246`) is reached by re-executing workflow code from the
bound module after the cursor exhausts. So the run's own module emits the config
again, byte-identical, on every recovery. `engine_reload_recovery.rs` already
gates the surrounding rule ("every restart path resolves a run's RECORDED
package version, never a fresh 'latest'").

**Why not the contract.** `advisory` and `agent` are on `ActionContract` because
they are declaration-owned CLASS markers that a call site must not be able to
forge (`nif_activity_advisory.rs:1-16`). `fallback` is not a class marker; it is
a routing/retry policy, and on this tree **every** routing/retry policy rides
the dispatch config — `retry`, `timeout`, `heartbeat`, `labels`, `task_queue`,
`workflow_task_queue`, `node`, `tier`
(`activity_dispatch.gleam:209-239`; `resolve_task_queue` reads
`config.task_queue` at `nif_activity.rs:108`). Putting `fallback` on the
contract would also change `ActionContract::canonical_bytes`
(`crates/aion-package/src/contract.rs:270-300`), whose optional tail was already
consumed by the advisory marker and which is under a bumped `.v5` identity
domain — i.e. it would force a `.v6` bump and rehash every package, for a value
that is already immutable-per-run by construction. **Rejected as unnecessary
identity churn.**

⚠️ The builder writes this argument into the doc comment on the config reader.
The "read it off the current deploy" reading is the natural one and is the exact
long-lived-workflow replay break the artifact names; the reason it is not taken
must be legible at the site.

### D2 — hop-first, retry-second lives in `dispatch_with_retries`

Not in `failure_stand_down`. `failure_stand_down` is a pure classifier with no
history, no recorder, no seam; the hop needs all three. The refusal arm mirrors
the existing worker-loss arm's shape (`nif_activity_retry_dispatch.rs:282-287`),
which is the established precedent for "a reason class the loop handles before
the ordinary stand-down".

### D3 — no fourth precedence level; the hop is a post-failure override at the dispatch seam

`resolve_task_queue` (`crates/aion/src/runtime/nif_activity.rs:108`) is **not
touched**, and neither is `config_task_queue_selection` (`:124`). Its four
levels stand exactly as documented at `:77-107`. The hop is applied *after* it,
at the dispatch seam, as an override of the recorded initial choice — artifact
R1-e lines 248-259 in as many words.

### D4 — `fallback` is a SOFT keyword; list entries are plain identifiers

Detail and the ambiguity argument in §4.1.

### D5 — same-provider retry budget is execution attempt minus recorded hops

Detail in §5.3.

---

## 4. The AWL surface

### 4.1 Syntax and the comma ambiguity — decided

Surface form, as the artifact spells it (line 211): `fallback norn, claude`,
sitting on the action's config line beside `retry`.

The config line is itself comma-separated
(`crates/aion-awl/src/parser/workers.rs:291-380`, continuation at `:361-364`),
so `fallback norn, claude` is prima facie ambiguous: after `norn`, a comma could
open a new key or continue the list.

**It is not ambiguous, because config keys are lexer keywords.**
`Keyword::ALL` (`crates/aion-awl/src/lexer/tokens.rs:230`) reserves `node`,
`timeout`, `retry`, `advisory`, `agent`; the config-line parser dispatches on
`TokenKind::Keyword(_)` and nothing else. So after a comma the next token is
either a `TokenKind::Keyword` (⇒ new key) or an identifier (⇒ next entry).

Two consequences the builder must implement exactly:

- **Add `Keyword::Fallback`** — `ALL` becomes `[Self; 58]` (`:230`), plus arms in
  `as_word` (`:315` region) and `from_str` (`:379` region).
- **Add it to `soft_keyword`** (`crates/aion-awl/src/parser/stream.rs:423`),
  beside `Node | Timeout | Retry | Advisory | Agent`. 🔴 **Not optional.**
  `fallback` is already used as an ordinary AWL name in the committed corpus —
  `outcome fallback:` at `crates/aion-awl/tests/fixtures/rev2/step-bodies/valid/
  predicates_and_operators.awl:34`, and in two `invalid/` fixtures
  (`all_non_bool.awl:11`, `any_non_list.awl:10`) whose expected diagnostics
  would otherwise change. Hard-reserving it breaks all three.
- **Entries are `TokenKind::Identifier` only.** `expect_name`
  (`stream.rs:379-393`) accepts soft keywords as names, so using it for list
  entries would swallow `fallback norn, retry 3 every 1s` — `retry` would parse
  as a second queue. The list parser therefore accepts a plain identifier and
  nothing else, for the first entry as well as the rest, so the rule is one
  sentence rather than two. **Disclosed limit:** a task queue whose name is a
  soft keyword (`node`, `timeout`, `retry`, `advisory`, `agent`, `fallback`,
  `filter`, `map`, `sort`, `count`, `every`, `backoff`, `empty`, `present`,
  `absent`) cannot be written in a fallback list. The diagnostic says exactly
  that and names the word — it does not say "expected an identifier".

Other parser touch points: the `opens_config` lookahead
(`workers.rs:265-277`) gains `Keyword::Fallback`; the three "expected a config
key" diagnostics (`workers.rs:347-352`, `:356-361`, and
`parser/action_body.rs:57`, plus its module doc at `:16`) gain the word.

AST: `ConfigLine` (`ast/document.rs:506`) gains
`pub fallback: Option<FallbackSpec>` after `retry` (`:541`), where
`FallbackSpec { span: Span, entries: Vec<ConfigValue> }` reuses the existing
`ConfigValue` (`:544`) so each entry keeps its own span for diagnostics.
`ast/mod.rs:17` re-exports it.

### 4.2 Checking

Three rules, in `crates/aion-awl/src/checker`:

1. **Duplicates are refused** (artifact line 207: *"Duplicate entries are
   rejected during AWL checking"*). Anchored on the span of the SECOND
   occurrence, naming the first. Comparison is exact string equality on the
   queue name.
2. **`fallback` is declaration-only**, exactly like `retry`. A call-site config
   line carrying it is refused at `checker/calls.rs:66-106` — a new `else if`
   arm beside the `retry` arm (`:98-105`), with the same sentence shape: *"a
   call site may pin `node` and `timeout` only — `fallback` stays on the action
   declaration; the chain is the action's declared routing, which one call site
   cannot change."* The four existing arms' messages all read "may pin `node`
   and `timeout` only", so nothing else there changes.
3. **An empty list is refused** at parse (`fallback` with no entry is a parse
   error naming the missing queue), so the checker never sees `Some(vec![])`.

**Not checked, deliberately:** that an entry names a `worker` block declared in
this document. A queue may legitimately be served by an out-of-band worker SDK —
the tree makes that a first-class permanent case (`ast/document.rs:405-409`,
`ActionContract::worker_owed`). A resolution check here would refuse valid
documents. **Also not checked:** an entry equal to the declaring worker's own
name. The artifact makes that the named example (line 211: *"This makes
`fallback norn, claude` deterministic even when the initial resolved queue is
already `norn`"*) and rules it a RUNTIME skip, not a check-time refusal.

### 4.3 Printing

`crates/aion-awl/src/printer/document.rs:484-503` renders the config line as
comma-joined parts in a fixed order: `advisory`, `agent`, `node`, `timeout`,
`retry`. `fallback` is appended **after `retry`** (mirroring the AST field
order and the artifact's "beside `retry`"), rendered `fallback a, b, c`.
`aion awl fmt` idempotence and the canonical-emission corpus
(`crates/aion-awl/tests/canonical_emission.rs`) are the gates.

### 4.4 Lowering — both backends, one authored list

The two lowerings must stay in step; the tree says so twice already
(`emitter/stmts.rs:161-162`, `mir/lower/activity.rs:281-286`).

**Reference emitter** — `crates/aion-awl/src/emitter/stmts.rs`. Beside
`retry_policy` (`:152`), a `fallback_chain` producing the Gleam call
`activity.fallback(<act>, ["norn", "claude"])`. Applied in the same per-key
merge the direct lowering mirrors (`stmts.rs:84-104`).

**Direct/MIR lowering** — `crates/aion-awl/src/mir/lower/activity.rs`.
`apply_action_config` (`:195`) applies keys in a fixed order —
`retry` (`:205-217`), `timeout` (`:218-229`), `task_queue` (`:231-240`), `node`
(`:241-252`). `fallback` is applied **after `retry` and before `timeout`**, so
the MIR order matches the reference emitter's merge order; the doc comment at
`:189-194` that states the mirror is updated in the same edit.

`crates/aion-awl/src/mir/runtime.rs` gains `RuntimeFn::ActFallback`:
declaration beside `ActRetry` (`:40-43`), the mapping
`Self::ActFallback => ("aion@activity", "fallback", 2)` (`:160-163`),
`DurableFamily::Activities` (`:231-234`), the `all_runtime_fns` row (`:270-273`),
and a new ordinal in the `variant_ordinal` staleness anchor (`:378-381` region —
appended at the end of the numbering, never inserted).

The list is a BEAM list of binaries, built the way the existing list-valued
arguments are; no new MIR `Stmt` kind.

**No `total_attempts` analogue.** There is no count to translate — the list is
lowered as authored. This is the artifact's "no numeric retry bound" (lines
243-246) at the lowering layer.

### 4.5 Gleam SDK

`gleam/aion_flow/src/aion/activity.gleam`: the opaque `Activity(i, o)` record
(`:69-83`) gains `fallback: List(String)` (empty list = no chain — the SDK's
own "absence is intentional data" convention, `:85-89`). 🔴 Gleam records are
rebuilt field-by-field in **every** builder, so `new` (`:97`), `retry` (`:118`),
the timeout/heartbeat/labels builders (`:137`, `:158`, `:186`), `task_queue`
(`:218`), `node` (`:247`), `execution_tier` (`:278`) each gain the field, plus
the new `fallback` builder and a `selected_fallback` accessor beside
`selected_task_queue` (`:348`). Missing one is a compile error, not a silent
bug — but there are nine of them and they are easy to half-finish.

`gleam/aion_flow/src/aion/internal/activity_dispatch.gleam::config` (`:209-239`)
gains `#("fallback", fallback_config(activity.selected_fallback(v)))` — a JSON
array, or `json.null()` for the empty list so the engine's "absent means no
chain" and "explicit null means no selection" conventions
(`nif_activity.rs:105-107`) both hold.

`sdk_internal_privacy.rs` gates what the SDK exports; the new public builder is
intentional and that test is updated with the reason.

### 4.6 Editor vocabulary — a hard gate, not a nicety

`crates/aion-awl/src/lexer/grammar_vocabulary_tests.rs` `include_str!`s
`tools/tree-sitter-awl/grammar.js` at compile time and compares its `const
keywords` (`grammar.js:33-43`) against `Keyword::ALL`. Adding `Keyword::Fallback`
**fails that test** until `'fallback'` is added to the grammar array.
`apps/aion-ops-console/src/features/authoring/lib/highlight-assets.test.ts`
then fails until the vendored
`apps/aion-ops-console/src/features/authoring/assets/awl.wasm` is rebuilt from
the source grammar. `editors/zed-awl` and `editors/nvim-awl` back onto the same
grammar. All four are in scope; the module doc at
`grammar_vocabulary_tests.rs:1-22` explains why.

---

## 5. Event, recorder, and the retry loop

### 5.1 `Event::ActivityFallbackRouted`

`crates/aion-core/src/event.rs`, added after `ActivityAdvisoryExhausted`
(`:491-502`), which is the closest structural precedent (an activity-scoped,
non-terminal, status-invisible record):

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

All five fields are artifact R1-e's minimum (lines 261-268). No `#[serde(default)]`
on any of them: the variant is new, so no recorded history can lack them.

**Exhaustive-match census — every arm a new activity Event variant must gain.**
Derived by tracing `ActivityAdvisoryExhausted` and `ActivityAdoptionOffered`
across the whole repo; this list is complete for non-test code:

| file:line | what |
|---|---|
| `crates/aion-core/src/event.rs:734` region | `Event::envelope()` arm |
| `crates/aion-core/src/status.rs:82`, `:87`, `:141`, `:146` | status projection — **non-terminal, advances nothing**, sits with `ActivityAdvisoryExhausted` |
| `crates/aion-core/src/current_step.rs:100-136` + module doc `:15-32` | current-step projection — **advances nothing**; the hop does not open a new step, the `ActivityStarted` that follows it does |
| `crates/aion-server/src/stream/selector.rs:138`, `:143` | stream selector family |
| `crates/aion/src/durability/cursor.rs:632` region | `event_kind` |
| `crates/aion/src/durability/replay_inspect.rs:698` region | `event_kind` |
| `crates/aion/src/durability/resolver.rs:287`/`:291`, `:327`/`:330` | family classification + `event_kind` |
| `crates/aion/src/engine/delegated.rs:474`, `:477` | delegated-engine event fan-out |
| `crates/aion/src/lifecycle/continue_as_new.rs:164`, `:200` | carry-over classification — groups with `ActivityAdvisoryExhausted` at `:200` (not carried) |
| `apps/aion-ops-console/src/features/workflow-detail/lib/timeline.ts:63`, `:138`, `:468` | timeline row + kind union |
| `apps/aion-ops-console/.../lib/timelineVariants.ts:86`, `:199` | variant table (its `:237` comment records the last time this drifted) |
| `apps/aion-ops-console/src/types/generated/index.ts` | **regenerated, never hand-edited** |

`Event` is already registered for TS export
(`crates/aion-core/src/generated_types.rs:231`), so no registration is added —
only regeneration. `Event` does not cross the proto wire (no
`ActivityAdvisoryExhausted` arm exists in `aion-proto`), so nothing there
changes.

### 5.2 Recorder method — the only append path

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

> 🔴 **Single writer.** `ActivityFallbackRouted` is appended ONLY through the
> workflow's existing Recorder held by `RetryRecorderSeam`. NEVER
> `EventStore::append` directly. The recorder guard re-reads the run segment
> under the recorder lock (settled ordinal / closed epoch / already-recorded
> hop stands the task down).

Concretely: the hop is written by a new `RetryRecord::FallbackRouted { … }`
variant on the enum at `nif_activity_retry_dispatch.rs:513-520`, appended via
`record_retry_event` (`:538`) and nothing else. That function already performs
two thirds of the guard the artifact demands, in this order and under
`seam.recorder.lock()`: the epoch gate (`:557`, `is_epoch_open`), the run-segment
scope (`:566`, `current_run_segment`), and the settlement re-read (`:570`,
`activity_settled`). **The third third is new:** an already-recorded hop for
`(activity_id, attempt)` must also stand the task down. `record_retry_event`
gains that check in the `FallbackRouted` arm — scan the run segment for an
existing `ActivityFallbackRouted` with the same `activity_id` and `attempt` and
return `RetryRecordOutcome::Settled` if one exists. Without it a detached
completion task that lost a race writes a second hop for one refusal, and
recovery derives the wrong list position from the count.

`recorder.rs` is **1995 lines at base**. It is over 500 already; per
`.fleet/BRIEF.md:84-86` it may gain only what the compiler demands (one method,
~25 lines with its doc), and the builder discloses that in the report.

### 5.3 The retry loop

`crates/aion/src/runtime/nif_activity_retry_dispatch.rs`. Five changes.

**(a) The reason predicate.** In `nif_activity_retry.rs`, beside
`RETRYABLE_REASON_PREFIX` (`:32`), `is_parked_reason` (`:57`) and
`is_worker_lost_reason` (`:88`):

```rust
const POLICY_REFUSED_REASON_PREFIX: &str = "policy_refused:";
pub(super) fn is_policy_refused_reason(reason: &str) -> bool { … }
```

`is_retryable_reason` (`:96`) is **left alone** — a `policy_refused:` reason is
genuinely not `retryable:`-class, and widening that predicate would route
refusals through the ordinary budget, which is exactly what "hop first" forbids.
The doc comment at `:91-95` ("Every other prefix … is non-retryable and behaves
as before") is amended to name the new class as handled ahead of it.

**(b) The refusal arm in `dispatch_with_retries`** (`:247`), placed in the loop
body after the worker-loss arm (`:282-287`) and **before** `failure_stand_down`
(`:288`) — the same position and the same shape the worker-loss class already
occupies. Its outcomes:

- **Chain live** → record nonterminal refusal, record hop, bump attempt, record
  `ActivityStarted`, `continue` with the new queue.
- **Chain declared and exhausted** → record nonterminal refusal, return
  `RetryLoopTerminal::Failed(<policy_refused: … recorded queue sequence …>)`.
- **No chain declared** → record nonterminal refusal, then fall through to the
  ordinary retry path with `retry_eligible` true (see (c)).

**(c) `failure_stand_down`** (`:183`) takes the policy-refusal class as
retry-eligible **only in the no-chain case**, and its exhaustion message must
begin `policy_refused:`. Artifact R1-d step 5 (lines 225-228) is explicit: *"do
not relabel it `exhausted:` or an engine fault."* The existing `"exhausted: spent
{attempt} of {max} attempts; last refusal: {reason}"` format (`:197-200`) is
kept for `retryable:`-class exhaustion and a `policy_refused:`-prefixed twin is
added for refusals. **The accounting stays inside the message; no numeric bound
is minted** — `policy.max_attempts` comes from `retry_policy_from_config`
(`nif_activity_retry.rs:280-315`) off the act's own `RetrySpec`, as the artifact
requires (lines 243-246).

**(d) Refusal records are `PolicyRefused`, not `Retryable`.**
`RetryRecord::AttemptFailed` (`:515`) hardcodes
`ActivityErrorKind::Retryable` at `:580`. Artifact R1-d step 1 (lines 216-217)
and step 5 (line 226) both require the nonterminal refusal record to carry
`ActivityErrorKind::PolicyRefused`. So `AttemptFailed` gains an explicit kind
(or a sibling variant) — either way the kind is passed, never defaulted.

**(e) Same-provider retry accounting, separated from execution identity.**
Artifact R1-d lines 235-241: `ActivityStarted.attempt` stays a monotonically
increasing identity for every actual execution *including a hop*, and the
authored budget must no longer be inferred from it.

Derivation, purely from history:

```
same_provider_attempt = execution_attempt − hops_recorded_for_this_ordinal
```

where `hops_recorded_for_this_ordinal` counts `ActivityFallbackRouted` events
for the ordinal in the current run segment. `failure_stand_down` is given
`same_provider_attempt`; `ActivityStarted`/`ActivityFailed` keep
`execution_attempt`. A hop increments the execution attempt and not the budget
attempt, which is precisely "a queue hop is an execution but is not a
same-provider retry". The loop derives the base count once from the seam's
history at entry (the same read `activity_settled_elsewhere` (`:440`) already
performs) and tracks both counters in-loop thereafter, so a loop restarted after
a crash resumes with the right budget.

**Delivery of the queue.** `dispatch_with_retries` currently builds each
delivery as `let mut delivery = request.clone(); delivery.attempt = attempt;`
(`:261-262`). It gains a loop-local `current_queue` initialised from
`request.task_queue` and reassigned on a hop, stamped as
`delivery.task_queue = current_queue.clone()`. `ActivityDispatch`
(`crates/aion/src/activity/bridge.rs:29-73`) is **not** modified.

**Who records the FINAL terminal.** Nobody in the loop. The loop returns
`RetryLoopTerminal::Failed(reason)`; `spawn_completion_task` (`:51-60`) delivers
it; the await seam records it. `take_runtime_completion`
(`nif_activity_await.rs:207-226`) calls `activity_error(error.message)`
(`nif_activity.rs:232-238`), which builds `ActivityErrorKind::Terminal` with the
message verbatim. So the final event is `Terminal` with a `policy_refused:`
message — artifact R1-c lines 197-201 and R1-d step 4 line 221-223, already true
on this tree with no change. **The builder verifies this by test and does not
edit the await seam.**

### 5.4 The chain reader

New module `crates/aion/src/runtime/nif_activity_fallback.rs`, holding the
whole of L3's routing derivation so no existing file has to grow much:

- `fallback_chain_from_config(config: &str) -> Vec<String>` — decodes
  `config.fallback`. Malformed JSON, absent, `null`, or non-array-of-string ⇒
  empty chain (no hop), mirroring `labels_from_config`/`resolve_task_queue`'s
  documented "never fail a dispatch over routing metadata" stance
  (`nif_activity.rs:100-107`) — with a `tracing::warn!`, never a silent drop.
  Carries D1's argument in its doc comment.
- `next_hop(chain, initial_queue, current_queue, consumed) -> Option<(usize, String)>`
  — the pure function: walk the authored list from position `consumed`, **skip**
  any entry equal to the current queue (artifact line 208-211), return the first
  distinct queue with **its original authored index**. Total, deterministic, no
  clock, no fleet state. Unit-tested exhaustively on its own.
- `recorded_hops(history, activity_id) -> Vec<&Event>` and
  `recorded_hop_queue(history, activity_id) -> Option<String>` — the latest
  `ActivityFallbackRouted.to_task_queue` for the ordinal, or `None`.
- `refused_queue_sequence(history, activity_id, initial_queue) -> Vec<String>` —
  the recorded initial queue followed by each hop's `to_task_queue` in recorded
  order. **This is what the terminal `policy_refused:` message lists**, per
  artifact line 302-303: *"by the recorded sequence, not by recomputing what
  might have happened."*

---

## 6. Recovery and the replay cursor

### 6.1 The durable order (artifact R1-e lines 277-283) and its five crash boundaries

1. record the policy-refused `ActivityFailed` for the current queue;
2. derive the next list position from the recorded initial queue plus prior
   `ActivityFallbackRouted` events and the immutable authored list;
3. record `ActivityFallbackRouted` before the next dispatch;
4. record the next `ActivityStarted`;
5. put the delivery on the recorded `to_task_queue`.

Steps 1, 3 and 4 are appends through `record_retry_event`; step 2 is pure; step
5 is the loop's next iteration. The crash boundaries are therefore: **before 1**,
**between 1 and 3**, **between 3 and 4**, **between 4 and 5**, and **after 5**.

- **Before 1** — nothing recorded; the trail dangles at the previous
  `ActivityStarted`. Replay exhausts the cursor and re-dispatches live on the
  queue `recorded_hop_queue` reports (or the initial queue if no hop yet), at
  the next attempt via `next_delivery_attempt`. Unchanged behaviour.
- **Between 1 and 3** — the refusal is recorded, no hop. Step 2 re-derives from
  the same inputs (recorded initial queue + recorded hops + the bound module's
  authored list) and reaches the same answer. Artifact line 285.
- **Between 3 and 4** — the hop is recorded. Recovery **reuses**
  `to_task_queue`; it does not derive again and does not consult live worker
  availability. Artifact lines 286-287. This is the boundary the whole event
  exists for.
- **Between 4 and 5** — hop and start both recorded; the attempt dangles.
  `dangling_attempt` (`nif_activity_retry.rs:379`) already handles this shape;
  the queue comes from the recorded hop.
- **After 5, before any terminal** — if recovery derives that no distinct hop
  and no configured same-provider retry remains, it must produce the same
  `RetryLoopTerminal::Failed` disposition with a `policy_refused:` message, and
  the await seam records the `Terminal`. Artifact lines 288-293: the ordinal
  must not dangle and no extra execution may be invented.

### 6.2 The re-dispatch queue override — the sharpest seam in this leg

🔴 On recovery, workflow code re-executes and
`dispatch_activity_with_context` (`crates/aion/src/runtime/nif_activity_dispatch.rs:301`)
re-resolves the queue at `:350` via `resolve_task_queue(&call.config,
start_time_task_queue)` — which yields the **INITIAL** queue, not the hopped-to
one. It then stamps that value onto both the recorded `ActivityStarted`
(`open_this_delivery` → `record_started`, `:281-295`) and the live
`ActivityDispatch` (`:385-397`). **Left as-is, a crash after a hop re-dispatches
on the refusing queue and history records it — the replay break the artifact's
R1-e exists to prevent.**

The fix, and it is deliberately outside `resolve_task_queue` (D3): immediately
after `:350`, override with the recorded hop.

```rust
let task_queue = super::nif_activity::resolve_task_queue(
    &call.config, start_time_task_queue.as_deref());
// R1-e: a recorded hop is a POST-FAILURE OVERRIDE of the recorded initial
// choice, never a fourth precedence level inside resolve_task_queue. The
// override reads history, so replay re-derives it identically.
let task_queue = super::nif_activity_fallback::recorded_hop_queue(
    context.history(), &activity_id).unwrap_or(task_queue);
```

The same override applies on the fan-out paths, which resolve the queue at three
sites in `crates/aion/src/runtime/nif_collect.rs`: `fan_out_items:401`,
`fresh_scheduled_activity:431`, and `fan_out_items_recovered:450`. The last one
already reads the queue back from the recorded `ActivityScheduled` (NSTQ-3) and
is the established precedent for exactly this move — the hop simply takes
precedence over the recorded `ActivityScheduled.task_queue` there. The existing
history reader to mirror is
`nif_collect_settlement.rs::scheduled_task_queue:461`.

`Event::ActivityScheduled` is **not** re-recorded on a hop. The artifact's order
lists only `ActivityFallbackRouted` then `ActivityStarted`, and
`ActivityStarted` carries no `task_queue` field — which is precisely why the hop
needs its own event. `resolve_node` (`nif_activity.rs:159`) is untouched: node
affinity is within-pool and a hop changes the pool, so the hop's dispatch
carries the same resolved node the config selects, or none.

### 6.3 Cursor span

`HistoryCursor::resolve_activity` (`crates/aion/src/durability/cursor.rs:297-367`)
walks an activity's span by matching `ActivityStarted`, a re-dispatch
`ActivityScheduled`, `ActivityFailed`, and the terminals, all keyed by
`activity_id`, and **skips** everything else in place (`_ => {}` at `:361`).

Two changes:

1. **Site 1's predicate** (§1.3) — the walk continues past a `PolicyRefused`
   failure exactly as it continues past a `Retryable` one.
2. **`ActivityFallbackRouted` is consumed into the span**, as a new match arm
   pushing to `matched` beside the `ActivityStarted` arm (`:308-313`). Artifact
   lines 295-297 require it: recovery and the cursor must treat the hop and the
   nonterminal refusal as part of the same activity span. Leaving it to the
   `_ => {}` skip would leave a straggler event matchable by a later command.

`replay_inspect.rs` walks recorded history directly with no natives; its
`event_kind` arm (`:698` region) is all it needs beyond site 7.

---

## 7. Test plan — the artifact's binding list

The artifact's L3 rows (lines 360-366), each mapped to a concrete gate. Existing
precedents named so the builder does not invent a harness.

**T1 — declared chain: one refusal, one hop, zero same-queue retries, empty
final failure reason on success.**
Precedent: `crates/aion/tests/activity_retry_e2e.rs` (350 lines) — real embedded
engine, in-process `ActivityDispatcher` stub recording every delivery's
`(task_queue, attempt)`, packaged example rebuilt from committed source. New
`crates/aion/tests/fallback_chain_e2e.rs` with a `RefusingDispatcher` that
returns `policy_refused:…` on queue A and succeeds on queue B. **The claim is
audited from HISTORY, not from the dispatcher's call log**, because that is what
the artifact means by auditable (lines 302-305): exactly one `ActivityFailed`
with `kind == PolicyRefused`; exactly one `ActivityFallbackRouted` with
`from_task_queue == A`, `to_task_queue == B`, `fallback_index == 0`; **zero**
second `ActivityStarted` on queue A; terminal `ActivityCompleted`; workflow
`Completed` with an empty failure reason.

**T2 — no-chain: configured retry policy preserved, no hop, terminal
policy-refused classification, NO assertion on attempt count.**
Same harness, an always-refusing dispatcher, an act with `retry N` and no
`fallback`. Assert: **zero** `ActivityFallbackRouted` events; every interim
`ActivityFailed` carries `kind == PolicyRefused`; the terminal `ActivityFailed`
carries `kind == Terminal` with a message starting `policy_refused:`; the
message does **not** start `exhausted:`. 🔴 The artifact forbids asserting a
particular count (line 363) — asserting one would mint the numeric bound the
design deliberately withholds. The test asserts `attempts >= 2` only, to prove
retry happened at all.

**T3 — replay after EACH crash boundary chooses the same recorded queue.**
All five boundaries in §6.1, one case each — not a single "crash somewhere"
test. Precedent: `crates/aion/tests/engine_reload_recovery.rs` (a crash-window
sweep over a real store) and `crates/aion/tests/recovery_restart.rs`. Each case
drives to the boundary, drops the engine, rebuilds on the same
`InMemoryStore`, and asserts the re-dispatch's queue equals the recorded one and
that no duplicate `ActivityFallbackRouted` appears. The "between 3 and 4" case
is the load-bearing one: it must show the queue coming from the event, provable
by making the authored list *and* the initial queue such that a fresh derivation
would pick differently if the event were ignored.

**T4 — settlement predicate, per site.** Seven focused unit tests, one per site
in §1.3, each feeding a history whose only failure is a nonterminal
`PolicyRefused` and asserting the site does not settle (sites 1-5) or rejects it
(sites 6-7). Plus the coupling test for `resolution_from_matched` (§1.4).
These are the ruling's teeth: a site silently reverted to `!is_retryable()`
fails exactly one of these.

**T5 — AWL surface.** Parse/print round-trip of `fallback a, b`; `aion awl fmt`
idempotence; duplicate-entry refusal with the second span; call-site `fallback`
refusal; empty-list parse refusal; the soft-keyword collision diagnostic; and
`fallback norn, retry 3 every 1s` parsing as **two keys**, not three queues.
Fixture corpus under `crates/aion-awl/tests/fixtures/rev2/`, following the
existing valid/invalid layout.

**T6 — both lowerings agree.** Precedent: `crates/aion/tests/awl_retry_counting_e2e.rs`,
which runs one document through the reference emitter and the direct MIR
lowering against a real engine and asserts identical delivery behaviour. The
fallback twin asserts both backends produce the same `config.fallback` array and
the same hop sequence.

**T7 — immutability premise.** A run started on module version v1 (authored
`fallback a, b`), a v2 of the same logical workflow deployed mid-run with a
different list, then a crash and recovery: the recovered run hops per v1.
Precedent and fixture: `crates/aion/tests/engine_reload_recovery.rs` +
`common/reload_fixture.rs`, which already builds two content-hash-distinct
versions of one workflow. This is the artifact's "invisible until the moment it
matters" defect (lines 330-336), made visible.

**Test discipline, from the artifact (lines 368-371) and `.fleet/BRIEF.md:75-78`:**
any live-seat test is runtime-gated on an env var, logs a `tracing::info!` skip
line, and returns `Ok(())`. **No `#[ignore]`, in any spelling.** No blocking
sleep sized to an expected duration: poll under a total deadline, with timeout
represented as a distinct outcome rather than a failed assertion.

---

## 8. Scope boundary — what L3 does NOT touch

Consumed from L2, never duplicated (artifact lines 340-347): adapter recognition
(`aion-integration-norn/src/session.rs:297`,
`aion-integration-acp/src/outcome.rs:176`), `HarnessError::PolicyRefused`
(`aion-integrations/src/error.rs:99`), `ActivityErrorKind::PolicyRefused`
(`aion-core/src/error.rs:14`), `harness_error_to_outcome`
(`aion-worker/src/runtime/agent.rs:379`), the bridge mapping
(`aion-server/src/worker/bridge.rs:478`), and Gleam `policy_refused:`
parse/encode (`activity_dispatch.gleam:138`, `:191`).

Report-only, out of scope: the `exhausted:`/`parked:`/`lost:` engine-side prefix
fallthrough (artifact lines 38-39), and erratum E-1/E-2/E-3 above.

---

## 9. Process walls, carried into the leg

- No lint suppressions in any spelling: no `#[allow]`, `#[expect]`, `#[deny]`,
  `#[ignore]` on tests, `_var` renames, or `#[cfg(any())]` dead-code hiding.
  `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check`
  both clean.
- No hand-edited generated types. `apps/aion-ops-console/src/types/generated/
  index.ts` is regenerated. `crates/aion-proto/src/worker.rs` and the `.proto`
  are hand-authored surfaces by design and are not touched by this leg.
- No new dependencies.
- Files already over 500 lines at base may gain only what the compiler demands,
  disclosed in the report. Known: `crates/aion/src/durability/recorder.rs`
  (1995), `crates/aion/src/durability/cursor.rs` (1715),
  `crates/aion/src/runtime/nif_activity_retry.rs` (905),
  `crates/aion/src/runtime/nif_collect_settlement.rs` (531). New routing logic
  goes in the new `nif_activity_fallback.rs`, not into these.
- Report at the workflow's injected wire path, with the `git ls-tree` receipt
  PASTED INTO the report body.
