# LEG L2 — classification: the PolicyRefused class, adapter detection, Gleam arms

**Deliverable:** `HarnessError::PolicyRefused` exists, both adapters detect it,
the Gleam grammar presents it as itself. R3.1 and R3.4 green.
**Depends on:** L1's committed `docs/design/policy-refused-fallback.md`. **Do not
start before it is committed.**
**Blocks:** L3.
**Lands with:** L3. See "Landing" below.

Read first: `docs/design/policy-refused-fallback.md` (authoritative for every
decision), then `.fleet/plan.md` §1–§3, then `.fleet/BRIEF.md` §CORRECTION.

**Implement the artifact's decisions and only those.** Where this brief and the
artifact disagree, the artifact wins — it was written with the tree in hand. If
the artifact left something open, that is a defect in L1; report it and stop
rather than deciding it here.

---

## Scope

`crates/aion-integrations`, `crates/aion-integration-norn`,
`crates/aion-integration-acp`, `crates/aion-worker`, `gleam/aion_flow`, plus
whatever `crates/aion-core` / `crates/aion-server` change L1's R1-c option
requires.

**Out of scope:** the AWL `fallback` declaration, engine chain consultation,
replay determinism, `resolve_task_queue`. All L3.

---

## R2.1 — the class

Add `PolicyRefused` to `HarnessError` — `crates/aion-integrations/src/error.rs:14-115`,
`#[non_exhaustive]` at `:15`. `thiserror` derive is already on the enum (`:14`);
the new variant carries its own `#[error(...)]` and a documented field set per
the artifact.

The exhaustive match at `error.rs:222-232` will fail to compile until the arm is
added. **That is the mechanism working as designed** (`:190-195`) — take the arm
the artifact decided, and put the artifact's rationale in the doc comment beside
the existing per-variant notes at `:197-220`.

🔴 The arm is whatever L1 decided. **`.fleet/BRIEF.md:86-87` pre-decides it
"deterministic" and is a refuted, unswept site** — see `.fleet/plan.md` §0. Do
not take that arm because the brief body says so. Take it because the artifact
argued it.

## R2.2 — the sweep

Every site that matches on `HarnessError` must decide the new variant on purpose.

| Site | What it is |
|---|---|
| `crates/aion-integrations/src/error.rs:222-232` | the exhaustive `is_deterministic()` — compile error until handled |
| `crates/aion-worker/src/runtime/agent.rs:244-248` | `deliver_dyn_command` — catch-all after `CapabilityNotSupported` / `StaleTarget` |
| `crates/aion-worker/src/runtime/agent.rs:327-331` | `apply_command` — same shape |
| `crates/aion-integration-norn/src/advice.rs:139-155` | `advise()` — matches `Harness` and `Transport`, passes the rest through |
| `crates/aion-worker/src/runtime/agent.rs:335-364` | doc block enumerating the deterministic variants — goes stale the moment one is added. Update it; `:362-364` explicitly warns against exactly this |

`agent.rs:244-248` and `:327-331` are on the intervention path, where a wildcard
is defensible. Decide it anyway and say why in a comment — a silent swallow is
what this leg exists to remove.

## R2.3 — detection, norn

At the site the artifact names. Per `.fleet/plan.md` §2 the two candidates are:

- `crates/aion-integration-norn/src/session.rs:286-292`, `response_into_payload`
  — the `JsonRpcError` binding at `:287`. `error.data`
  (`crates/aion-integrations/src/jsonrpc/envelope.rs:117`) is parsed and retained
  by the transport and **dropped unread** at `:288-291`. This is the attested
  path: `crates/aion-integration-norn/src/advice.rs:273-276` transcribes a real
  refusal in exactly this format string, code `-32603`.
- `crates/aion-integration-norn/src/translate/outcome.rs:61-91` — `stop: &Value`
  at `:61`, `reason: &str` at `:67`, flattened by `format!` at `:83`.

🔴 **`cyber_policy` does not appear anywhere in this repository.** It is a fact
about the provider's wire, not a symbol here.
`crates/aion-integration-norn/src/protocol.rs` — by its own doc `:1`, the only
module that names norn's wire — declares no error schema and no refusal-code key.
Do not hardcode the spelling outside whatever constant the artifact defines, and
do not invent a fixture that pretends the tree knows it.

🔴 **Do not put detection in `AdvisingHarness`.** `advise()` at
`advice.rs:139-141` destructures `HarnessError::Harness { detail: String }` — it
sits above the adapter and by then `data` is gone (`session.rs:288-291`) and
`stop` is a String (`outcome.rs:83`). Placing detection there forces substring
matching. `.fleet/BRIEF.md:71-75` proposes it as the template; `.fleet/plan.md`
§2.4 shows why it cannot be. The decorator stays what it is — decoration.

If the artifact concluded norn cannot express a refusal structurally and detection
must be textual, implement it textually **and say so at the site**, in a comment,
naming where the substring came from. An admitted prose match is acceptable; a
prose match dressed as structural is not.

## R2.4 — detection, ACP

`StopReason::Refusal` is a typed variant —
`crates/aion-integration-acp/src/translate.rs:291` maps it to `"refusal"`, and it
is exercised at `:657` and `:678`. The discriminator is `prompt.stop_reason` at
`crates/aion-integration-acp/src/outcome.rs:152-154`; `non_completion` at
`:169-177` currently folds `Refusal` in with `MaxTokens` and `MaxTurnRequests`
into one prose message.

Separate `Refusal` out per the artifact. **No substring matching is needed or
permitted here** — the value is typed.

(`.fleet/BRIEF.md:79-81` cites `outcome.rs:180-204` as the ACP surface. That is
`prompt_error`, the JSON-RPC path. The refusal surface is `:130-177`.)

## R2.5 — the reason vocabulary

Per L1's R1-c decision, exactly one of:

1. **`ActivityErrorKind` gains a variant** — `crates/aion-core/src/error.rs:9-14`.
   Wire + TS-codegen event. **REGENERATE. Never hand-edit generated types.** If
   a new type is introduced, add its `push_type::<T>` to the hand-maintained
   registry at `crates/aion-core/src/generated_types.rs:210-234` — that file's
   doc `:3-19` records the 2026-07-30 break from forgetting exactly this, and
   `assert_every_dependency_is_exported` (`:16-19`) is what catches it.
   `ActivityErrorKind` itself is already registered at `:222`.
2. **`crates/aion-server/src/worker/bridge.rs:475-490`** mints a third prefix from
   something other than `error.is_retryable()`.
3. **The structured-detail route** — `ActivityFailure::with_detail(Payload)`,
   `crates/aion-worker/src/activity.rs:52-57`; into `ActivityError.details` at
   `:104`; across the protocol at `crates/aion-worker/src/protocol/session.rs:832`;
   durably recorded in `Event::ActivityFailed { error: ActivityError, … }`,
   `crates/aion-core/src/event.rs:469-478`. Call site to change:
   `harness_error_to_outcome`, `crates/aion-worker/src/runtime/agent.rs:366-375`,
   which builds a bare `ActivityFailure` today and never attaches detail.
   Reconcile `details` with the reason string per the artifact —
   `bridge.rs:490` reads only `error.message`.

## R2.6 — the Gleam grammar

The engine's reason string must present as itself, never as "activity engine
failure".

- `parse_error/1` — `gleam/aion_flow/src/aion/internal/activity_dispatch.gleam:134-162`.
  Existing prefixes: `retryable:` `:135`, `terminal:` `:138`, `timeout:` `:141`,
  `cancelled:` `:145`, `non_determinism:` `:151`. Fallthrough to
  `error.ActivityEngineFailure(message: raw)` at `:156`. Add the arm.
  Note the pattern: each arm does `string.drop_start(raw, <len>)` with the
  prefix's exact byte length. Get it right.
- `encode_error/1` — `:178-196`. Add the matching arm so the round trip holds.
- `describe_activity_error/1` — `gleam/aion_flow/src/aion/awl/error.gleam:160-175`,
  `ActivityEngineFailure` arm at `:172-173`.
- `gleam/aion_flow/src/aion/testing/mock.gleam:84-89` — the encode arms.

Whatever new `error.ActivityError` constructor the arm needs goes in
`gleam/aion_flow/src/aion/awl/error.gleam` beside its siblings.

🔴 **Do NOT widen scope to the fallthrough mislabelling.** `:156` also swallows
`exhausted:` (`crates/aion/src/runtime/nif_activity_retry_dispatch.rs:198`),
`parked:` (`nif_activity_retry.rs:37`, `:49`), `lost:` (`:76`),
`transport-exhausted:` (`nif_activity_retry_dispatch.rs:277`) and
`superseded:server-death` (`nif_activity_retry.rs:382`). **Report all five as a
named finding; fix none of them.** Note also that the brief's second example,
`infrastructure:`, is NOT a reason prefix on this tree — it is a message body
(`crates/aion-server/src/worker/outbox_redrive.rs:390`,
`outbox_dead_letter.rs:82`) that reaches the operator as
`retryable:infrastructure: …` and DOES parse. Correct that in the finding.

---

## R3 — green for this leg

### R3.1 — unit, both directions (BRIEF.md:140-142)

A synthetic provider policy refusal (structured per the artifact's detection
seam) classifies `PolicyRefused`. An ordinary harness error still classifies
`Harness` and stays retryable. Both directions, both adapters.

For ACP the negative direction is sharp and free: `StopReason::MaxTokens` and
`StopReason::MaxTurnRequests` must NOT classify `PolicyRefused` — they share
`non_completion` (`outcome.rs:169-177`) with `Refusal` today.

Existing test homes: `crates/aion-integrations/src/error.rs:235+`,
`crates/aion-integration-norn/src/session/tests/outcomes.rs`,
`crates/aion-integration-norn/src/translate/tests/outcomes.rs`,
`crates/aion-integration-acp/src/outcome.rs:250+`.

### R3.4 — the Gleam surface (BRIEF.md:152-154)

A `policy_refused:` failure presents with that prefix, not "activity engine
failure". Assert on the projected `failure_reason` —
`crates/aion-core/src/filter.rs:176` (`failure_projection`), `:186`
(`project_failure_reason`), `WorkflowSummary.failure_reason` at `:92`. Brief's
`:176-206` **holds**.

### R3.5 — gates

`cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check`
both clean. Gleam gates per venue config. Full workspace green.

---

## Walls

`.fleet/plan.md` §5 in full. Live for this leg:

- No lint suppression in any spelling. No `#[ignore]` in any spelling — a test
  needing a runtime gates it AT RUNTIME (env var, `tracing::info!` skip line,
  `return Ok(())`).
- No file over 500 lines of code. **`crates/aion-integration-norn/src/harness.rs`
  is 1064 and `crates/aion/src/runtime/nif_activity_retry.rs` is 905 — do not add
  to either.** `session.rs` is 329, `advice.rs` is 583, `agent.rs` is 379,
  `crates/aion-integrations/src/error.rs` is 305. New logic goes in new named
  modules; `mod.rs` holds only `pub mod` and re-exports.
- `thiserror` for library errors. Never `.unwrap()` / `.expect()` in library
  code. Lock poison mapped to a typed error.
- Wire/TS changes REGENERATED, never hand-edited.
- No blocking sleep sized to an expected duration; poll with a total deadline and
  timeout as a distinct outcome.
- 🔴 **Do not mint a retry bound anywhere in this leg** — no `1`, no `2`, no
  "sensible default".
- 🔴 **Never write a test asserting the no-chain path fails after exactly ONE
  attempt.** That is L3's territory and the rule binds there too, but do not
  smuggle it in here as a unit test either.
- Report any provider policy refusal your own work trips as a first-class finding
  with what tripped and where. It is evidence for this brief. Do not rewrite the
  work to dodge the trigger.

## Landing

`.fleet/BRIEF.md:127` requires both halves to land together. L2 is a separate leg
but not a separate landing: **L3 branches from L2's head and the merge to the
flight base is L3's head, carrying both.** L2 does not merge alone.

Exception: if L1 recorded measured red under R1-f, L3 does not exist and L2 lands
alone. That is the designed outcome, not a shortfall.

`git ls-tree` receipt before declaring done.
