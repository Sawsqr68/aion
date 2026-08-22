# L1 design report — policy-refused fallback

## Delivered

- Authored `docs/design/policy-refused-fallback.md` (design artifact only).
- No Rust, Gleam, AWL grammar, generated wire type, or test implementation was
  changed in this leg.

## Observed decisions

- The artifact classifies provider policy refusal as stochastic and selects
  `HarnessError::is_deterministic() == false`
  (`docs/design/policy-refused-fallback.md:132-143`). This follows the two
  measured survivals in
  `docs/evidence/policy-refused-census-20260822.md:34-68`.
- It expressly supersedes the original “will refuse this content every time”
  and “fail fast with no in-place retry” propositions
  (`docs/design/policy-refused-fallback.md:24-34`).
- Norn recognition is placed at
  `crates/aion-integration-norn/src/session.rs::response_into_payload`; the only
  accepted structured shape is `/error/code == "cyber_policy"`, while current
  norn output is not claimed to populate it. Missing/unknown data uses the
  admitted textual match against the measured `JsonRpcError.message` phrase
  (`docs/design/policy-refused-fallback.md:63-93`).
- ACP recognition is typed at
  `crates/aion-integration-acp/src/outcome.rs::outcome_into_payload`, branching
  on `StopReason::Refusal` before `non_completion`
  (`docs/design/policy-refused-fallback.md:111-122`).
- The F-1 choice is `ActivityErrorKind::PolicyRefused`, which requires wire and
  regenerated TypeScript changes in L2
  (`docs/design/policy-refused-fallback.md:151-172`). The prefix is the
  end-to-end class carrier; retry-loop disposition separately determines
  finality (`docs/design/policy-refused-fallback.md:174-199`).
- The observed `ActivityError.details` path does not survive the
  `ActivityDispatcher::dispatch -> Result<String, String>` engine seam; the
  artifact records that limitation rather than depending on details for
  routing (`docs/design/policy-refused-fallback.md:183-199`,
  `crates/aion/src/activity/bridge.rs:75-90`,
  `crates/aion/src/runtime/nif_activity_retry_dispatch.rs:573-585`).
- Fallback is an ordered task-queue chain, not a model field. Duplicate authored
  queues are rejected; an entry equal to the current queue is skipped and its
  original list index remains the durable index
  (`docs/design/policy-refused-fallback.md:202-211`).
- The chain is consulted first. A live chain consumes no same-provider retry
  budget. With no chain, the act's configured retry policy remains; the design
  mints no bound (`docs/design/policy-refused-fallback.md:213-244`).
- `resolve_task_queue` remains the initial queue precedence. A fallback is a
  post-failure override recorded as `Event::ActivityFallbackRouted` through the
  single-writer `Recorder`, with recovery behavior specified for crashes before
  a hop, after a hop, and after the final refusal but before terminal delivery
  (`docs/design/policy-refused-fallback.md:246-313`).
- F-3 remains report-only (`docs/design/policy-refused-fallback.md:36-37`).

## Review findings resolved

An independent review found four defects in the first draft, all corrected
before commit:

1. A non-retryable policy-refused event would have satisfied the current
   `activity_settled()` predicate before its hop. The artifact now requires an
   explicit terminal predicate: `PolicyRefused` is an intermediate history fact
   and only `Terminal` settles (`docs/design/policy-refused-fallback.md:176-182`,
   `crates/aion/src/runtime/nif_activity_retry.rs:321-350`).
2. The draft incorrectly claimed structured details reached engine history.
   The artifact now records the observed string erasure and uses the dedicated
   kind/prefix as the control carrier
   (`docs/design/policy-refused-fallback.md:183-199`).
3. The draft left norn's structured data shape open. It now admits exactly
   `data.pointer("/error/code")` and states that current population of that shape
   is unproven (`docs/design/policy-refused-fallback.md:66-75`).
4. The draft did not define repeated/current queue entries. The artifact now
   rejects duplicates and skips the current queue deterministically
   (`docs/design/policy-refused-fallback.md:202-211`).

A focused re-review then found two final-exhaustion ambiguities. The artifact
now separates prefix-carried class from retry-loop-carried finality and requires
recovery to derive and deliver the same final terminal when history ends on the
last nonterminal refusal (`docs/design/policy-refused-fallback.md:193-199`,
`:284-306`).

## Moved seam coordinates observed

The brief's positional references had moved on this tree. I anchored the design
on symbols and observed these current coordinates:

- `HarnessError`: declaration at
  `crates/aion-integrations/src/error.rs:16`; enum/doc span `:14-115` rather than
  the brief's ending at `:113`. `HarnessError::Harness` is `:79-82`.
- `HarnessError::is_deterministic`: `crates/aion-integrations/src/error.rs:222-232`
  rather than the brief's `:223-233`.
- `harness_error_to_outcome`:
  `crates/aion-worker/src/runtime/agent.rs:366-375` rather than `:366-378`.
- Retry clone/mutation:
  `crates/aion/src/runtime/nif_activity_retry_dispatch.rs:261-262` rather than
  `:264-265`.
- Norn placeholder negative test:
  `crates/aion-integration-norn/src/harness.rs:637-646`, function declaration at
  `:638`, rather than `:639-645`. It proves `{attempt}` is literal; it does not
  state a policy intent about provider variation.
- Gleam `parse_error`:
  `gleam/aion_flow/src/aion/internal/activity_dispatch.gleam:134-162` rather than
  ending at `:160`.
- Gleam `encode_error`:
  `gleam/aion_flow/src/aion/internal/activity_dispatch.gleam:178-196`; it is a
  separate symbol from `parse_error` rather than the brief's bundled range.
- Gleam mock encoder arms:
  `gleam/aion_flow/src/aion/testing/mock.gleam:84-95`; the originally cited
  `:80-90` begins before the function and ends before all arms.
- Norn composition root:
  `crates/aion-cli/src/harness/compose.rs:52-59` rather than `:54-59`.
- ACP refusal surface:
  `crates/aion-integration-acp/src/outcome.rs:130-177`, with
  `outcome_into_payload` at `:130` and `non_completion` at `:169`, rather than
  the brief's `prompt_error` range `:180-204`.
- AWL emitter lowering:
  `crates/aion-awl/src/emitter/stmts.rs::retry_policy` at `:163-180`; the brief's
  range began at `:154` before the symbol.
- AWL MIR lowering:
  `crates/aion-awl/src/mir/lower/activity.rs::build_retry_policy` at `:284-320`
  rather than `:274-285`.
- `PoolAddress`:
  `crates/aion-server/src/worker/registry.rs:116-131`, declaration at `:128`,
  rather than the brief's `:117-123`.
- `resolve_task_queue`:
  `crates/aion/src/runtime/nif_activity.rs:108-113`, with its precedence doc at
  `:77-100`; the brief's `:77-113` brackets the area but not the symbol start.
- The exhaustive-match doctrine is at
  `crates/aion-integrations/src/error.rs:190-195`, not ending at `:194`.
- `parse_error`'s vocabulary has five existing prefixes at
  `activity_dispatch.gleam:135`, `:138`, `:141`, `:145`, and `:151`; it is not
  the three-prefix vocabulary stated in the original brief body.

Coordinates relied on that held on this tree include
`crates/aion-integration-norn/src/session.rs:288-291`,
`crates/aion/src/runtime/nif_activity_retry_dispatch.rs:183-203` and `:194`,
`crates/aion-core/src/generated_types.rs:222`, and
`crates/aion-worker/src/runtime/agent.rs:244-248`, `:327-331`, and `:335-364`.

## Validation observed

- `wc -l docs/design/policy-refused-fallback.md`: below the 500-line wall.
- A retry-number grep found only the measured attempt-1/attempt-2 exhibits; no
  authored retry bound was introduced.
- `git diff --check`: clean.
- `git diff --name-only`: only the design artifact and this report before
  commit; no downstream implementation file changed.
- No provider policy refusal occurred during this leg.
- No workspace compile/test gate was run because this leg changes Markdown
  design/report files only. The artifact received an independent full review
  and a focused follow-up review instead.
