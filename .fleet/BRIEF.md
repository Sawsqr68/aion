# BRIEF — PolicyRefused L3: authored fallback and routing (leg 3 of 3, staged)

You are standing on L2's ratified result (fleet/policyref-l2-result =
68568af4d). L1 designed, L2 classified. L3 ROUTES. The authoritative design is
the artifact IN THIS TREE: `docs/design/policy-refused-fallback.md` — read it
whole before anything; sections R1-d (declared fallback semantics), R1-e
(history-derived replay), "Why the authored list is immutable for a run", and
"Implementation boundaries and gates" are your specification. Where this brief
and the artifact disagree, the artifact wins and you SAY SO in your report.

## Scope (artifact: "L3 owns only authored fallback and routing")

AWL `fallback` syntax/lowering, queue-chain configuration, route recording
(`Event::ActivityFallbackRouted` + recorder method), retry-loop consultation,
recovery/replay cursor handling, and E2E coverage. L3 CONSUMES L2's committed
class (`HarnessError::PolicyRefused`, `ActivityErrorKind::PolicyRefused`, the
`policy_refused:` reason vocabulary) — never duplicates recognition.

## Binding counsel findings — carried verbatim, non-negotiable

1. 🔴 THE SETTLEMENT TERMINAL PREDICATE (L2 counsel OPEN-2, ruled):
   "L2 must not implement it; L3 cannot omit it."
   Five sites read `!error.is_retryable()` as "settled" (audit-time
   coordinates — RE-DERIVE each by symbol on this tree):
   `crates/aion/src/durability/cursor.rs:343`,
   `crates/aion/src/runtime/nif_collect_settlement.rs:339` and `:430`,
   `crates/aion/src/runtime/fan_out.rs:339`,
   `crates/aion/src/runtime/nif_activity_retry.rs:349`.
   A `policy_refused:` reason is NOT retryable — but with a live fallback
   chain it is NOT SETTLED either. Every one of those five sites must decide
   the three-valued question on purpose; leaving any on the two-valued read
   silently settles a mid-chain refusal.
2. 🔴 CONFIRMED BY L2's JUDGE (hand-off note): `is_retryable_reason`
   (`nif_activity_retry.rs:96-98` audit-time) still tests only the
   `retryable:` prefix, so on the tree you stand on a `policy_refused:`
   reason settles without retry or hop. Artifact R1-d step 5 ("no-chain →
   existing configured retry policy, refusal stays nonterminal while budget
   remains, terminal on exhaustion with `policy_refused:` reason — never
   relabeled") plus hop-first-retry-second is YOURS to make true.

## Hard walls, from the artifact

- **Single writer**: `ActivityFallbackRouted` is appended ONLY through the
  workflow's existing Recorder held by `RetryRecorderSeam`. NEVER
  `EventStore::append` directly. The recorder guard re-reads the run segment
  under the recorder lock (settled ordinal / closed epoch / already-recorded
  hop stands the task down).
- **Immutability premise**: the authored fallback list is read from THE RUN'S
  OWN BOUND MODULE VERSION, never the currently deployed package (artifact
  §"Why the authored list is immutable" — current-deploy reading is a
  long-lived-workflow replay break invisible in every test).
- **Hop ordering is durable** (artifact R1-e order 1-5); replay after a crash
  at EACH boundary chooses the same recorded queue — tested.
- **No numeric retry bound minted anywhere** — the design deliberately has
  none; the no-chain path uses the act's existing resolved `RetrySpec`.
- **`resolve_task_queue` gains NO fourth precedence level** — fallback is a
  post-failure override of the recorded initial choice.
- Terminal `policy_refused:` reason lists queues by RECORDED sequence.
- Duplicate fallback entries rejected at AWL check; an entry equal to the
  current queue is skipped at runtime and the original authored index is what
  history records.
- Known artifact erratum (L1 counsel OPEN-1, ruled "report, do not stop"):
  the norn secondary-site name `outcome_into_payload` does not exist; the
  referent is `run_result_to_outcome`. Not an L3 work item — recorded so you
  do not re-derive the confusion.

## Tests (artifact list, binding)

- A declared chain records refusal then hop and succeeds WITHOUT a same-queue
  retry; the E2E claim auditable from history: one refusal, one hop, zero
  same-queue retries, empty final failure reason on success.
- No-chain behavior preserves the configured retry policy, attempts no hop,
  terminates with policy-refused classification WITHOUT asserting a
  particular attempt count.
- Replay after each crash boundary chooses the same recorded queue.
- Live-seat tests runtime-gated (env var, logged skip, return Ok) — never
  `#[ignore]`. No blocking sleep sized to a duration; poll + total deadline +
  timeout-as-distinct-outcome.

## Process walls

No lint suppressions in any spelling. No hand-edited generated types
(regenerate; `aion-proto/src/worker.rs` and the `.proto` are hand-authored
surfaces by design). No new dependencies. Files already over 500 lines at
base may gain only what the compiler demands, disclosed in the report (L2's
precedent). Report at the workflow's injected wire path with the
`git ls-tree` receipt PASTED INTO the report body.
