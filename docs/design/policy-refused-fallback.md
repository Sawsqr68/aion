# Policy-refused classification and declared provider fallback

**Status:** L1 design decision, 2026-08-22

**Scope:** design only; L2 implements classification and L3 implements routing

**Evidence:** `docs/evidence/policy-refused-census-20260822.md`

## Decision summary

A provider policy refusal is a **stochastic routing fact**, not a deterministic
content fault. The system will:

1. recognise the refusal inside each adapter before its typed discriminator is
   flattened;
2. expose it as `HarnessError::PolicyRefused` and
   `ActivityErrorKind::PolicyRefused`;
3. render it on the worker-to-engine reason wire as `policy_refused:`;
4. consult an authored queue fallback chain before considering any
   same-provider retry;
5. record every refusal and queue hop through the workflow's `Recorder`, so
   replay obtains the same queue sequence from history; and
6. retain a bounded same-provider retry when no chain is declared, using the
   act's configured retry policy rather than a bound invented here.

This design supersedes two original propositions from `.fleet/BRIEF.md`:

- It supersedes the original claim that the provider will refuse the same
  content every time. Workflow `4ddee56c` completed the same act on attempt 2
  after a `cyber_policy` refusal on attempt 1, and aion#65 round 3 independently
  survived the same pattern. There is no mechanism on this tree that turns
  those observed stochastic outcomes into deterministic ones.
- It supersedes the original instruction to fail fast with no in-place retry
  when no chain exists. That behavior would have killed both measured
  survivals. The no-chain path therefore retains the act's configured bounded
  retry behavior.

The design does **not** address the existing unparsed engine-side prefixes
listed as finding F-3 in `.fleet/plan.md`; that remains report-only scope.

## Evidence and constraints

The census found four flights whose providers kept refusing across their whole
budgets and at least two independent cases in which the same content cleared on
a later execution. It supports both routing away from a refusing provider and
retaining a configured no-chain retry. It does not support any newly chosen
retry count.

The tracked frame at `gate-logs/dev-worker-0150.log:589` contains the upstream
structured path `error.code = "cyber_policy"`. The next line, which is what norn
hands to aion, contains only the prose `"provider rejected the request under its
cybersecurity policy"`; the code has disappeared. The JSON-RPC application code
seen by aion is the generic `-32603`, so it cannot identify a refusal.

A provider change cannot be an in-process model rotation. The measured norn
harness is ChatGPT-only, AWL has no model field, and task queue is the existing
pool/flavour selector (`PoolAddress` in
`crates/aion-server/src/worker/registry.rs`). A fallback is therefore a
queue/harness hop.

## R1-a — recognition belongs inside the adapters

### Norn

**Primary site:**
`crates/aion-integration-norn/src/session.rs::response_into_payload`.

This is the last attested aion-owned point holding
`JsonRpcError { code, message, data }`. Recognition happens before the existing
`HarnessError::harness(format!(...))` conversion. The one accepted structured
shape is the measured provider frame copied into JSON-RPC data:
`data.pointer("/error/code") == Some("cyber_policy")`. This exact path mirrors
`gate-logs/dev-worker-0150.log:589`; L2 must not recursively search arbitrary
JSON or accept look-alike keys. The tree does **not** attest that norn currently
populates this shape, so it is a narrowly specified capability rather than a
claim about today's norn output. The provider code value is evidence-attested,
not inferred from the generic JSON-RPC code.

`JsonRpcError.code == -32603` is not a discriminator. The real-run fixture
`crates/aion-integration-norn/src/advice.rs::tests::RECORDED_REFUSAL` shows a
non-policy session refusal under that generic code, while the measured policy
refusal was also surfaced as an application-level run failure. The outer code
therefore identifies neither class. `error.data` is the only possible structured
JSON-RPC channel.

When `error.data` is `None`, has an unknown shape, or does not carry the policy
code, norn cannot express this refusal structurally to aion today. The honest
fallback at this same adapter site is **textual** and deliberately narrow: match
the observed norn phrase
`provider rejected the request under its cybersecurity policy` in
`JsonRpcError.message`. The match is against that adapter-owned message field,
not against arbitrary transcript, prompt, output, or formatted
`HarnessError` text. The resulting refusal records that detection was textual
and preserves the original detail. Unmatched errors remain
`HarnessError::Harness`.

**Secondary structured point:**
`crates/aion-integration-norn/src/translate/outcome.rs::outcome_into_payload`.

The stop envelope remains structured until this function formats `stop` into
prose. On this tree the protocol names only `stop.reason = "completed"`; there
is no declared refusal value, and the measured refusal arrived through the
JSON-RPC error path. Therefore L2 must not guess that an arbitrary non-completion
stop is a policy refusal. This site recognises `PolicyRefused` only if the norn
wire contract gains an explicit refusal reason; otherwise its current
non-completions remain ordinary harness failures.

`AdvisingHarness::advise` is not a detection site. It receives an already
flattened `HarnessError::Harness { detail: String }`, so putting recognition
there would make every path textual and would discard the available
`JsonRpcError.data` discriminator.

### ACP

**Site:** `crates/aion-integration-acp/src/outcome.rs::outcome_into_payload`, at
the branch over `prompt.stop_reason`, before
`outcome.rs::non_completion` flattens it.

ACP recognition is **typed**:
`agent_client_protocol::StopReason::Refusal` maps directly to
`HarnessError::PolicyRefused`. `MaxTokens` and `MaxTurnRequests` do not. The
existing `non_completion` helper currently folds all three into one prose
`HarnessError::Harness`; L2 must split `Refusal` before that helper. The
JSON-RPC `prompt_error` function is not ACP's policy-refusal seam.

## R1-b — class and stochastic classification

`HarnessError` gains a harness-neutral `PolicyRefused` variant. It carries the
human-readable detail and may carry adapter-supplied structured diagnostic
metadata, such as the provider code and whether recognition was structured or
textual. Provider-specific names remain inside adapter construction; callers
route on the variant, never on prose.

`HarnessError::is_deterministic()` returns **false** for `PolicyRefused`.
Exhibits A and B directly rule out `true`, and no contrary mechanism exists.
This answer describes the provider outcome's repeatability only; it does not
mean policy refusal should flow through the ordinary transient-retry branch.

The binary `is_deterministic()` API cannot express “route first, retry second.”
Therefore `crates/aion-worker/src/runtime/agent.rs::harness_error_to_outcome`
must special-case `HarnessError::PolicyRefused` before its existing
`true -> terminal` / `false -> retryable` mapping. Ordinary errors continue to
use that binary. The new arm produces a dedicated policy-refused activity
failure, while `is_deterministic() == false` remains the truthful answer to the
narrow determinism question.

L2 must also sweep the intervention catch-alls in
`agent.rs::deliver_dyn_command` and `agent.rs::apply_command`,
`crates/aion-integration-norn/src/advice.rs::advise`, and the classification
documentation above `harness_error_to_outcome`. A new variant must never be
silently collapsed by a wildcard.

## R1-c — reason vocabulary and the F-1 choice

This design chooses **F-1 option 1**: add
`ActivityErrorKind::PolicyRefused` in `crates/aion-core/src/error.rs`.

The other choices are rejected:

- Keeping `ActivityErrorKind` binary cannot produce a third worker-originated
  reason at `crates/aion-server/src/worker/bridge.rs`, which currently chooses
  only `retryable:` or `terminal:` from `error.is_retryable()`.
- Using only `ActivityError.details` would durably preserve metadata but would
  not reach the reason string parsed by the retry loop and Gleam grammar,
  because `bridge.rs` reads only `error.message`.
- Teaching the bridge to infer policy refusal from message/detail would move
  classification away from the adapter and recreate a second, drift-prone
  classifier.

`ActivityFailure` therefore gains a dedicated constructor/classification that
converts to `ActivityErrorKind::PolicyRefused`. The worker/server bridge maps
that kind explicitly to `policy_refused:`. This is a wire change: update the
registered type in `crates/aion-core/src/generated_types.rs`, regenerate all
TypeScript bindings, and never hand-edit generated output.

The reason string and details have separate duties:

- `ActivityErrorKind::PolicyRefused` is the authoritative control signal.
  `bridge.rs` renders it as `policy_refused:`, the engine routes on that prefix,
  and Gleam parses it as its own public error class. It is neither ordinary
  `Retryable` nor final `Terminal`: `ActivityError::is_retryable()` stays false,
  while settlement/cursor code gains an explicit terminal predicate under which
  only `Terminal` settles an activity. A recorded `PolicyRefused` is therefore an
  intermediate routing/retry fact until the engine records a final disposition.
- `ActivityError.details` is **not** the end-to-end carrier for this flight.
  Although `ActivityFailure::with_detail` crosses the worker protocol, the
  server bridge returns only a prefixed `String` through
  `ActivityDispatcher::dispatch`, and the engine retry recorder reconstructs
  `ActivityError { details: None }`. Adapter metadata is therefore lost before
  engine history on this path. Routing correctness and the L2/L3 contract use
  only the dedicated kind/prefix. Preserving provider code and detection mode in
  durable engine history would require a structured dispatcher result instead
  of `Result<String, String>`; that restructuring is outside this flight.

This resolves finding F-2 honestly: the available details channel and the
reason string are not reconciled by pretending they meet. The prefix is the
sole end-to-end **class** carrier; it does not encode finality. The retry loop's
own disposition carries finality, and the await seam records that disposition as
`ActivityErrorKind::Terminal` while preserving the `policy_refused:`-prefixed
message verbatim. The current detail loss is recorded as a limitation rather
than silently relied upon.

## R1-d — declared fallback semantics

AWL gains an ordered `fallback` list beside `retry` on `ActionConfig`. Entries
are task queues, not model names. The emitter and MIR lower the same authored
list into activity configuration. Duplicate entries are rejected during AWL
checking. At runtime, an entry equal to the current queue is skipped rather than
executed; the next distinct queue is the hop, and its original authored index is
what history records. This makes `fallback norn, claude` deterministic even when
the initial resolved queue is already `norn`, and it prevents a declared hop
from becoming a same-provider retry. This design adds no model field.

For a policy-refused execution:

1. Record the refusal as a nonterminal `ActivityFailed` with
   `ActivityErrorKind::PolicyRefused` and the execution attempt.
2. If an unconsumed distinct fallback queue exists, select it and record the hop
   before dispatching there.
3. Dispatch the same activity ordinal and input on that recorded queue.
4. If each distinct declared queue has refused, record/deliver the final
   disposition as `ActivityErrorKind::Terminal` whose reason begins
   `policy_refused:` and names the refusing queue sequence.
5. If no fallback chain was declared, apply the act's existing configured retry
   policy on the same queue. Each refusal with budget left remains a nonterminal
   `PolicyRefused` event. On exhaustion, record/deliver a final `Terminal` event
   whose reason begins `policy_refused:` with the attempts accounted; do not
   relabel it `exhausted:` or an engine fault.

Thus **hop first, retry second**. A live chain consumes only its own ordered
entries. A policy refusal while the chain is live is not eligible in
`failure_stand_down`'s ordinary retryable branch and consumes no same-provider
retry budget.

Execution identity and retry accounting must be separated. The existing
one-based `ActivityStarted.attempt` remains a monotonically increasing identity
for every actual execution, including a fallback execution. The authored retry
budget is no longer inferred solely from that global execution number: L3 must
track/derive same-provider retries separately, because a queue hop is an
execution but is not a same-provider retry. This preserves the `(workflow,
activity, attempt)` dedupe identity without charging a hop to `RetrySpec`.

There is deliberately no numeric retry bound in this design. The no-chain path
uses the value resolved by `retry_policy_from_config` from the act's existing
`RetrySpec`; the census supplies no basis for another default.

## R1-e — history-derived replay

A fallback hop is **not** a fourth input-precedence level in
`resolve_task_queue`. That function still chooses the initial queue exactly
once:

1. per-activity override;
2. workflow-declared default;
3. start-time queue from recorded history; then
4. the existing named last resort.

The resulting initial queue remains recorded in `Event::ActivityScheduled`.
Fallback is a post-failure override of that recorded initial choice.

L3 adds a dedicated history event, named here
`Event::ActivityFallbackRouted`, with at least:

- `activity_id`;
- the refusal execution attempt;
- `from_task_queue`;
- `to_task_queue`; and
- the zero-based position consumed from the authored fallback list.

The hop is appended only through the workflow's existing `Recorder` held by
`RetryRecorderSeam`; no component calls `EventStore::append` directly. The
recorder guard re-reads the current run segment under the recorder lock before
adding the hop, so a settled ordinal, closed engine-task epoch, or already
recorded hop stands the detached completion task down.

The order around a hop is durable:

1. record the policy-refused `ActivityFailed` for the current queue;
2. derive the next list position from the recorded initial queue plus prior
   `ActivityFallbackRouted` events and the immutable authored list;
3. record `ActivityFallbackRouted` before the next dispatch;
4. record the next `ActivityStarted`; and
5. put the delivery on the recorded `to_task_queue`.

A crash between (1) and (3) re-derives the same next queue and records it. A
crash after (3) reuses the event's `to_task_queue`; it does not consult live
worker availability or choose again. If recovery finds a trailing nonterminal
`PolicyRefused` and derives that no distinct hop or configured same-provider
retry remains, it must produce the same `RetryLoopTerminal::Failed` disposition
with a `policy_refused:` message; the normal await seam then records the final
`ActivityErrorKind::Terminal` event. Thus a crash after the last refusal but
before final delivery cannot leave the ordinal dangling or invent another
execution.

Recovery and the replay cursor must treat `ActivityFallbackRouted` and
nonterminal `PolicyRefused` as part of the same activity span, just as they
already walk retryable `ActivityFailed` and later `ActivityStarted` events.
Queue selection and final exhaustion are therefore functions of history plus
the immutable activity configuration, never wall clock, fleet membership, or
provider availability.

The terminal `policy_refused:` reason lists queues by the recorded sequence,
not by recomputing what might have happened. This makes the E2E claim auditable:
one refusal plus one hop, zero same-queue retries, and an empty final failure
reason when the fallback succeeds.

This design is replay-deterministic within the existing architecture; measured
red is not required. It does require L3 to add the explicit event, recorder
method, cursor/recovery handling, and separate retry-budget accounting described
above. Omitting any one of those pieces would make the route implicit or charge
fallback executions against the wrong budget.

### Why the authored list is immutable for a run

*(Added post-gate by the flight owner: the derivation above relies on this
premise without stating it, which is the owner's finding on review, not a defect
in the gated design.)*

The derivation above depends on the authored fallback list being **immutable for
the lifetime of a run**. It is, and the reason is worth stating because the
alternative reading is the natural one.

Each `.aion` package version is a distinct, immutable module named by its content
hash (`logical_name$hash`). A running workflow is bound to the module version it
started with, so the fallback list it derives hops from cannot change underneath
it — a redeploy mints a *new* module rather than mutating the existing one.
`ContinuedAsNew` begins a new run and re-derives against whatever version that
new run binds to, which is correct rather than a gap.

⇒ Implementations must read the authored list from **the run's own bound module
version**, never from the currently deployed package. The current-deploy reading
produces identical behaviour in every test — the two only diverge for a workflow
that was mid-hop when a new version landed, and then replay derives a different
queue sequence than history recorded. That is a long-lived-workflow replay break
that is invisible until the moment it matters, which is the class of defect this
engine exists to eliminate.

## Implementation boundaries and gates

L2 owns only classification and presentation: adapter recognition,
`HarnessError::PolicyRefused`, `ActivityErrorKind::PolicyRefused`, worker/server
mapping, regenerated bindings, and Gleam `policy_refused:` parsing/encoding.

L3 owns only authored fallback and routing: AWL syntax/lowering, queue-chain
configuration, route recording, retry-loop consultation, recovery/replay, and
E2E coverage. L3 must consume L2's committed class rather than duplicating
recognition.

Tests must establish:

- structured norn data classifies as policy refusal while an ordinary generic
  JSON-RPC failure remains `Harness`;
- norn's admitted textual fallback is limited to the measured message field and
  phrase;
- ACP `StopReason::Refusal` classifies as policy refusal while max-token and
  max-turn stops do not;
- `is_deterministic()` is false, while worker conversion still emits the
  dedicated activity kind;
- worker bridge and Gleam preserve `policy_refused:`;
- a declared chain records refusal then hop and succeeds without a same-queue
  retry;
- no-chain behavior preserves the configured retry policy, attempts no hop, and
  terminates with policy-refused classification without asserting a particular
  count; and
- replay after each crash boundary described above chooses the same recorded
  queue.

Any live-seat test is runtime-gated, logs its skip, and returns successfully
when the required environment is absent. No test is ignored and no blocking
sleep is sized to an expected run duration; asynchronous outcomes are polled
under a total deadline with timeout represented distinctly.
