# The assistant, ported to AWL — an honest differences ledger

`examples/assistant/awl/assistant.awl` is the full port of
`examples/assistant/src/assistant.gleam`. It is not a spike. This document
records every place the AWL version does not exactly reproduce the Gleam
semantics, with the reason, and what the port makes better.

Everything below was established against the checker, the compiler, the
emitted MIR goldens, and the engine source — not inferred from the language
reference. Where a claim is about runtime behaviour, the code path is cited.

## Why the port exists

The Gleam-authored `assistant` package **cannot be started at all** on a
server that routes activities through task queues. A project manifest's
`DeclaredActivity` carries exactly one field (`activity_type`), so
`PackageContract::from_manifest` commits every activity as UNSCOPED, and
`crates/aion/src/lifecycle/start_admission.rs:37-47` refuses the start with
`NO_QUEUE_DECLARATION`. See `GLEAM-PATH-CANNOT-START-FINDING.md`. The AWL
compiler emits a scoped contract (`crates/aion-awl/src/compile.rs:381` sets
`unscoped_activities: Vec::new()`), so AWL is the only surface that can author
this workflow.

The Gleam workflow module is untouched and stays in place until the AWL
version is live-proven.

## Verification state

| Gate | Result |
| --- | --- |
| `aion awl check examples/assistant/awl/assistant.awl` | ok (2 steps) |
| `aion awl fmt` | applied, and idempotent on a second run |
| `aion awl emit` | exit 0 — the document compiles end to end, nested wait-loop included |
| `cargo test -p aion-awl` | exit 0, 473 passed / 0 failed |
| agent-dev worker `cargo clippy --all-targets -- -D warnings` | exit 0, zero diagnostics |
| agent-dev worker `cargo test` | exit 0, 79 passed / 0 failed |
| agent-dev worker `cargo build --release` | exit 0 |
| `cargo clippy -p aion-awl -p aion-core -p aion-package --all-targets -- -D warnings` | exit 0 |

`cargo clippy --workspace --all-targets` exits 101, on four errors all inside
`crates/aion-server/` from another agent's concurrent, in-flight refactor of
`validate_worker_contracts` (plus a `queues_tests.rs` not yet created). None of
this port's commits touch any file under `crates/` — verified with
`git diff --name-only`.

**Not yet done: this has not been deployed or run.** It is checked, compiled,
built, tested and committed. Live proof is a separate step, and see
[Open risk: worker contract admission](#open-risk-worker-contract-admission).

---

## 1. The design decision, and a correction to its stated reason

The port moves round-one prompt composition into the worker:
`assistant_provision(repo_path, run_id, objective) -> Workspace{path,
opening_prompt}`. That is implemented as directed, and it is the right call.

**But the reason given for it — "AWL cannot compose strings" — is factually
wrong, and the record should not carry it.** AWL has string concatenation on
runtime bindings, and it is used in production:

```awl
# examples/dev-brief/awl/dev_brief.awl
provision_workspace(..., workspace_path: config.repo_root
  + "/.yggdrasil-worktrees/dev-brief/" + workflow.id)
```

Triple-quoted raw-string constants exist too, and `const` supports constant
concatenation. A determined author could hold the ~4KB contract in constants,
concatenate the two runtime bindings in, and select the repository-attached
branch with two steps and an `outcome`. It would be genuinely bad — the
contract text would live in a document that cannot test it, and the branch
would be a control-flow fork carrying 4KB of prose — but "cannot" is not true.

The decision stands on its real merit, which is stronger than the stated one:
**the worker already owns everything the prompt describes.**
`examples/agent-dev/worker/src/handlers/skill_resources.rs` `include_str!`s
the skill documents and materializes them into `.assistant/resources/` inside
the very workspace `assistant_provision` creates. The handler that puts the
files there is the only place that can describe them from knowledge rather
than assumption. Composed anywhere else, the prompt can only restate a fact it
cannot check.

## 2. AWL has no query surface — `assistant_status` is GONE

**This is the largest single loss in the port, and it is not recoverable in
AWL today.**

The Gleam workflow answers an `assistant_status` query with `{phase, round}`,
re-registering the handler at every stage transition so it survives replay.
AWL has no vocabulary for it:

- `crates/aion-awl/src/lexer/tokens.rs:60-173` — the `Keyword` enum has 55
  variants; there is no `query`, and `from_word` (`:242-300`) has no `"query"`
  arm.
- `crates/aion-awl/src/parser/document.rs:128-221` — the workflow header
  accepts exactly `timeout`, `input`, `signal`, `send`, `outcome`. Anything
  else is a hard parse error.
- `crates/aion-awl/src/ast/document.rs:9-46` — `Document` has no query field.
- `crates/aion-awl/src/compile.rs:23-53` and the whole `src/emitter/` tree
  contain zero query references, so the emitted program registers no handler.
- `crates/aion-package/src/*.rs` — the committed contract surface has no query
  member.

The runtime protocol *does* support queries
(`crates/aion-proto/src/workflow.rs:80,87-99`), so the asymmetry is purely in
the authoring language: **an AWL workflow answers every query with
`unknown_query`.**

This matters more than it looks. The 28-day-parked run in
`DURABLE-AGENT-SESSION-EPOCH-FINDING.md` was confirmed healthy by exactly this
query — a live `assistant_status` returning
`{"phase":"awaiting_operator","round":2}` is what proved the workflow was
parked rather than wedged. The AWL port gives up that observation. The
substitute is the event history (which still shows the last
`ActivityCompleted` and the park on `assistant_continue`), but that is
inference from events, not an answer from the running workflow.

**Recommendation: AWL needs a query declaration.** It is the one construct the
Gleam surface has that AWL cannot express and that an operator actually
reaches for.

## 3. An undecodable signal payload FAILS the run — it is NOT tolerated

> **AMENDED 2026-07-30 — FIXED. Read the rest of this section as the finding
> that produced the fix, not as current behaviour.** The declared signal type
> is now enforced AT THE BOUNDARY, in `Engine::signal`, before anything is
> recorded and before the arrival can be consumed: a payload that does not
> satisfy the declared type is refused to the CALLER (`POST /workflows/signal`
> → HTTP 400, naming the field), the target run's history is byte-identical
> afterwards, and it stays parked on the same wait. An undeclared signal name
> is refused the same way. The declared workflow input schema is enforced at
> the start boundary alongside it. Proven live on both a pre-fix and a
> post-fix server; see
> `BRIEF-DECLARED-CONTRACT-AT-THE-BOUNDARY.md` and the commit that closes
> tasks #95 + #97-A. The mitigation described at the end of this section (make
> the payload hard to fail) is no longer the only defence.

The Gleam module is explicit that this must never happen:

> A payload that does not decode at all is ALSO tolerated as a no-op — an
> operator typo must never kill a live session.

**AWL cannot reproduce this.** Traced end to end:

- Delivery is type-erased and unvalidated. `ConcreteSignalRouter::route`
  (`crates/aion/src/signal/router.rs:35-60`) records the payload opaquely;
  there is no declared-type table on the path. The HTTP handler
  (`crates/aion-server/src/api/handlers/workflows.rs:166-204`) and the CLI
  (`crates/aion-cli/src/main.rs:686-709`) validate only that the payload is
  well-formed JSON.
- The decode happens later, in the workflow process, in the Gleam SDK
  (`gleam/aion_flow/src/aion/signal.gleam:41-58`), against the codec the AWL
  compiler bound into the `SignalRef`
  (`crates/aion-awl/src/emitter/wrappers.rs:180-195`).
- **The arrival is marked consumed BEFORE the decode is attempted**
  (`crates/aion/src/runtime/nif_signal.rs:338-341`). It is gone either way.
- A decode failure becomes `Error(AwlSignalFailed(...))`
  (`gleam/aion_flow/src/aion/awl/error.gleam:166-189`) and `TryBind`
  short-circuits the step. **The run fails.** The loop's `until` never runs.

So in AWL the tolerance is not "keep waiting" — it is "die". The only lever is
a step-level `on failure` handler, and I verified it checks clean on a step
containing a loop. **I deliberately did not use it**, because
`emit_with_failure` (`crates/aion-awl/src/emitter/failure.rs:102-107`) matches
`Error(_)` indiscriminately: it would catch a genuine `assistant` agent-round
failure just as readily and convert it into a clean completion. Trading a loud
failure for a silent one is worse than the problem.

**Mitigation actually taken:** make the payload hard to fail. See §4 — both
signal fields are required and scalar, so any JSON object with a string
`message` and a boolean `end` decodes, and extra fields are ignored (Gleam
field decoders are not exhaustive). What still fails the run: non-object JSON,
a missing field, or a field of the wrong JSON type.

**This is a real gap in AWL** and worth naming as a finding: the language has
no "reject this payload and re-park" primitive, and no way to catch one
failure class without catching all of them.

## 4. The signal payload's fields are REQUIRED, not optional

Gleam: `Continuation(message: Option(String), end: Option(Bool))`, so
`{"end": true}` and `{"message": "..."}` are each complete payloads.

AWL: `type Continuation { message: String, end: Bool }` — both required.

This is forced, not preferred. **AWL cannot read the value of an optional; it
can only test presence.** Verified against the checker:

| Expression | Checker |
| --- | --- |
| `nudge.end` where `end: Bool?` | `` `until` needs a Bool condition, found Bool? `` |
| `nudge.end is present and nudge.end` | `` `and` needs Bool operands, found Bool? `` |
| `nudge.message == ""` where `message: String?` | `` `==` needs matching operands, found String? and String `` |
| `route x(last: nudge.message)` where `message: String?` | `expects String, found String?` |

Nor is there narrowing. `when nudge.message is present` does **not** retype
`nudge.message` to `String` in that branch (tested directly). The
`guard_optional_wait.awl` fixture only appears to narrow because field access
*through* an optional record (`reply.message` where `reply: Reply?`) unwraps
one level; a bare optional scalar stays optional forever.

Consequences had the fields stayed optional:

- `end: Bool?` — only `is present` is readable, so `{"end": false}` would end
  the session. Actively wrong.
- `message: String?` — a blank message could not be detected, AND the message
  could not be passed to `assistant(prompt: ...)` at all, because the action
  takes `String`. That alone makes an optional message unusable.

**Operator contract change:** send `{"message": "...", "end": false}` to
continue and `{"message": "", "end": true}` to finish. `{"end": true}` alone
now fails the round (§3). Any caller UI or runbook must be updated.

## 5. Blank detection is `== ""`, with no `trim`

Gleam does `string.trim(message)` before deciding a message is blank. AWL has
no `trim` and no string functions at all — the deterministic value operations
are field access, indexing, `+`, comparisons, booleans, and the collection
ops. So the port tests `nudge.message != ""`.

Effect: a message of only whitespace (`"   "`) is NOT a blank nudge in AWL. It
spends a round and dispatches a whitespace prompt to the agent. Minor, but
real.

It could be fixed by normalising in a worker action, at the cost of an extra
recorded activity per nudge. Not worth it; recorded here so the choice is
visible.

## 6. Every loop must be bounded — the nudge wait has a ceiling

Gleam's `await_operator` recurses without limit: blank nudges are absorbed
forever. AWL's checker makes that illegal — `max` is mandatory on a `loop`
(`crates/aion-awl/src/checker/blocks.rs:290-296`), and a `max N visits` bound
must intersect every backward-route cycle
(`crates/aion-awl/src/checker/cycles.rs:1-9`).

So the port takes a `max_nudges` **input** (not a constant — the value is the
operator's, per house rules) and gives blank-nudge absorption an explicit
ceiling. Exhausting it is a new, third disposition, `NudgeCapExhausted`,
routed to the `stalled` outcome as a clean success.

**This is a genuine semantic difference**, and arguably an improvement: an
unbounded no-op loop is unbounded recorded history. But it is a behaviour the
Gleam version did not have, and a session CAN now end in a way the original
could not.

## 7. `max_rounds` and `max_nudges` are required inputs with no defaults

Gleam has `pub const max_rounds = 50`. AWL has `const`, but the house rule is
that a bound the language demands comes from an input, and AWL has **no input
defaults** — so the caller must supply both. `max_rounds = 50` remains the
documented product decision; it is now stated by the caller rather than baked
in. Any start form or runbook must send both.

## 8. The round cap consumes one extra operator signal

Gleam's `session_loop` checks `round >= max_rounds` **before** awaiting the
operator, so at the cap it reports `RoundCapExhausted` immediately and never
consumes a signal.

An AWL `loop` evaluates `until` only after the whole body, and there is no way
to skip a statement conditionally inside a body (`route` is illegal in a loop
body). So on the final permitted round the port runs the agent, then waits,
consumes one `assistant_continue`, and only then reports `RoundCapExhausted`.

The operator's final message is consumed without being dispatched. Not wrong,
but it is a wasted message — and note that if they send `{"end": true}`
instead, they get `OperatorEnded`, which is arguably the better answer.

## 9. The blank-`objective` rejection is documented, not enforced

Gleam layers validation onto the input codec, so a blank `objective` is
rejected at the boundary with a `{"aion_error":"input_decode",...}` envelope.

AWL has no input-validation construct. An `input` declaration is name and type
only (`crates/aion-awl/src/ast/document.rs:65-78`), and the checker's only
input checks are snake_case naming and duplicate names
(`crates/aion-awl/src/checker/decls.rs:72-92`). There is no `require`,
`validate`, guard, or constraint.

The one lever is the JSON Schema type door: `type NonBlank = schema { "type":
"string", "minLength": 1 }` passes through byte-for-byte into the derived
start contract (`crates/aion-awl/src/schema/derive.rs:360-366`, `:160-191`).

**I did not use it, because I could not verify it is enforced.** `input_schema`
is carried as data and is validated for *worker contract compatibility*
(`crates/aion-server/src/api/worker_grpc.rs:606-622`), but I found no code path
applying it to a workflow START request. Declaring a constraint the server
does not check would be worse than declaring nothing: it would read as
enforcement to the next person. The rule is stated in the document's input
comment instead.

**Finding for someone to close:** either AWL's `input_schema` is enforced at
start and this should use the schema door, or it is not and that is a hole in
the start boundary for every AWL workflow.

## 10. No workflow `timeout` — deliberate, and a correction of risk

The Gleam version has no workflow timeout, and neither does the port. This is
worth stating loudly because a `timeout` looks like good hygiene and would be
catastrophic here: agent rounds carry no timeout by design (a wedged round is
cancelled or intervened), and a real session parked on `assistant_continue`
for **28 days** and was still healthy
(`DURABLE-AGENT-SESSION-EPOCH-FINDING.md`). A workflow timeout would kill
exactly the sessions this workflow exists to keep alive.

Note the spike (`assistant_spike.awl`) declares `timeout 1h`. That is correct
for a one-round spike and would be wrong here.

## 11. Errors become outcomes

Gleam has a typed `AssistantError(stage, message)` for a stage that could not
execute — a failed dispatch or a broken signal channel — plus ~60 lines of
error-message mapping. AWL has none of that: an action failure fails its step
after declared retries and the run fails with the engine's own error. The
three terminal outcomes are all clean completions.

This is a simplification, and mostly an improvement: the Gleam error mapping
was pure restatement of engine errors into strings. What is lost is the
`stage:` label telling an operator *which* stage failed — recoverable from the
event history, which names the activity.

---

## What the port makes better

- **It can actually start.** The Gleam version cannot, and has been dead since
  6 July.
- **The prompt is composed where the facts are.** The handler that creates the
  workspace and writes `.assistant/resources/` is the one that describes them.
- **Both prompt branches are tested.** The Gleam suite asserted the
  no-repository branch's distinguishing text and never asserted that the
  repository-attached branch withholds it. The Rust port tests both as mutual
  negative controls — an assistant told its empty scratch directory IS the aion
  repository would answer repository questions from nothing and call it
  verified.
- **The bounds are explicit and visible in the document.** Both budgets are
  inputs a reader can see, not a constant buried in a module.
- **`NudgeCapExhausted` names a state the original could reach but not
  report** (an operator nudging forever), instead of silently looping.
- **Half the line count and no error-mapping boilerplate**, with the flow
  legible in one screen.

---

## Worker contract admission — DIAGNOSED AND FIXED

The observed refusal read as a contradiction: `worker advertised <missing>` for
`assistant`, in a log record whose `activity_types` list contained `assistant`.
It is not a contradiction. **The two lines report two different proto fields,
and the gate is right.**

A registration carries names and typed descriptors independently:

- names → `activity_types`, `crates/aion-worker/src/runtime/liminal.rs:1134`
  (registry) and `:423-427` (agent types)
- typed descriptors → `activities`, `:1136-1144` (registry) and `:428-444`
  (agent)

Nothing synthesizes a descriptor from a name. Admission reads ONLY the
descriptor list (`crates/aion-server/src/worker/liminal_transport.rs:1495-1529`
builds `advertised` exclusively from `registration.activities`); the name list
is used only for registry membership (`:1574`). The refusal WARN then logs
`registration.activity_types` (`:1640-1652`) — the field the gate never saw.

**The measured fault: `agent-dev-worker` advertised 8 names and ZERO
descriptors.** `register_activity` inserts a handler only
(`crates/aion-worker/src/activity.rs:136-159`, whose doc comment says so
outright at `:196-199`), and `AgentHarnessConfig::new` initialises
`activity_descriptors: Vec::new()` (`liminal.rs:256-266`) which `main.rs` never
filled.

FIXED in `examples/agent-dev/worker/src/descriptors.rs`: the surface is derived
from the embedded `assistant.awl` and split between the agent harness and the
registry, each half advertised through the path that executes it. Hand-written
schemas were rejected deliberately — they are claims about a contract they
cannot see, and drift is refused with the same message as absence.

One hazard worth recording for anyone reusing this API:
`with_activity_descriptors` EXTENDS the agent activity-type set with every
descriptor name (`liminal.rs:276-292`). Handing it a mixed surface routes plain
registry actions to the agent harness — here it would have sent
`assistant_provision`, which clones a git workspace, to norn as a prompt.

**Still open, and not this worker's to fix:** the refusal diagnostic should
report the field it actually compared, or say "0 typed descriptors advertised".
As it stands it tells an operator the worker advertised something it did not,
and that reading costs hours.

## 12. An AWL agent activity delivers a JSON-WRAPPED prompt — open defect

Traced end to end, at the bytes. This affects every AWL-authored agent action
in the repository, not only this workflow.

AWL synthesizes an input RECORD for every declared action, unconditionally,
with no arity-1 unwrap (`crates/aion-awl/src/emitter/context.rs:155-159`), and
its codec serializes an object (`crates/aion-awl/src/emitter/codecs.rs:279-291`).
So `action assistant(prompt: String) -> String` puts `{"prompt":"..."}` on the
wire — confirmed in the emitted Gleam, `assistant_input2_to_json` builds
`json.object([#("prompt", ...)])`.

The worker's agent path passes the payload bytes through unchanged
(`crates/aion-worker/src/runtime/liminal.rs:962-970`), and the norn adapter
unwraps ONLY a bare JSON string (`crates/aion-integration-norn/src/harness.rs:242-255`):

```rust
match serde_json::from_str::<Value>(&text) {
    Ok(Value::String(inner)) => Ok(inner),
    _ => Ok(text),
}
```

An object falls to `_ => Ok(text)`. **The agent therefore receives the literal
text `{"prompt":"You are the aion AWL authoring assistant: ..."}` as its
prompt.** It does not fail, and it is not unwrapped — the pass-through is
deliberate and pinned by a test (`harness.rs:519-527`), on the stated ground
that "inventing a projection here would lose information".

The output direction is CORRECT: the harness returns a bare JSON string, which
matches the AWL contract's `{"type":"string"}`. Only the input side is wrapped.

This is why the spike appeared to work: an agent handed a JSON blob still
answers. It was degraded, not correct, and nothing said so.

It does NOT block admission — the descriptor derived from the document matches
the contract either way — so the live restoration can proceed. But every round's
prompt is JSON-wrapped until this is fixed.

The fix is not in this worker's gift. Two options, and the adapter authors have
already ruled out the second:

1. AWL emits a bare-string input codec for a single-`String`-param action bound
   to an agent activity type — but AWL does not currently know which actions are
   agent-routed, so this needs a language-level answer.
2. A prompt projection at the harness seam — explicitly rejected in the adapter
   as information-losing.

**Recommendation: (1), and it needs a decision before AWL agent authoring is
called finished.**

## 13. Shared-queue agreement — RESOLVED, and why the spike survives

Measured with the real gate: the spike declared
`assistant_provision(repo_path, run_id)` and the port added `objective`. Input
compatibility is contravariant, so a worker requiring `objective` narrowed the
spike's contract and was refused by it. Admission holds a worker against EVERY
reachable contract on a queue at once, so the refusal was total: **neither**
document could run. That is what took queue `default` down.

RESOLVED by bringing the spike into agreement rather than retiring it — and the
reason is finding 14, not sentiment. A deployed workflow type cannot be removed
from a running server, so "retiring" the spike would still have left an
`assistant_spike` type routed and served forever, with its source deleted. A
deployed workflow with no source is worse than a maintained minimal example.

The fix also corrected a real defect in the spike: it sent the bare `objective`
as its prompt, opening the session with no working contract at all. It now
sends `session.opening_prompt`.

The invariant is pinned in
`examples/agent-dev/worker/src/descriptors_tests.rs` —
`every_document_on_this_queue_agrees_with_the_advertised_surface` runs
`contract_diffs` for every document on the queue. This is a permanent coupling:
any future signature change to one document must be mirrored in the other, or
the queue goes down again.

## 14. Deploy is a ONE-WAY DOOR — no workflow type can ever be removed

Confirmed at the bytes, and a genuine platform gap.

The entire deploy surface is four verbs — `LoadPackage`, `ListVersions`,
`RouteVersion`, `UnloadVersion`
(`crates/aion-proto-generated/proto/deploy.proto:11-16`, mirrored exactly in
HTTP at `crates/aion-server/src/api/http/router.rs:151-156` and in the CLI at
`crates/aion-cli/src/main.rs:270-301`). There is no `undeploy`, `delete`,
`remove`, `retire`, or `archive` on any transport.

The dead end is structural, from three facts:

1. Every load inserts a route for every member type
   (`crates/aion/src/loader/catalog_load.rs:207-212`), so a type can never exist
   without a route.
2. `route_version` can only MOVE the route to an already-loaded version
   (`crates/aion/src/loader/catalog.rs:531-553`). There is no `routed.remove`,
   `clear_route`, or `unroute` anywhere in `crates/`.
3. Unload refuses the route-active version
   (`crates/aion/src/loader/catalog.rs:598-605`).

Therefore **exactly one version of every loaded type is always route-active, and
zero versions is unreachable.** The error text says "route another version
first", which is only actionable when another version exists — at N=1 it is
advice that cannot be taken. `docs/errors.md:69` repeats the same impossible
instruction.

Not even an embedder can work around it: `Engine::workflow_catalog()` is `pub`,
but every mutating catalog method is `pub(crate)`; the only `pub` mutator is
`load_package`.

The only exit today is offline surgery — `DELETE FROM packages WHERE
workflow_type = ...` plus a restart. That does work, because reload tolerates
and skips a route pointer naming an unloaded version
(`crates/aion/src/loader/persistence.rs:176-186`), leaving the stale
`package_routes` row inert. It is unsupported surgery, not a procedure, and
there is no documented removal process anywhere.

### What the missing operation needs to look like

A `RemoveWorkflowType` verb, in one deploy mutation:

- Delete `routed[workflow_type]` — the primitive that does not exist.
- Swap out every version of the type, bypassing the route check only because the
  route is deleted in the same commit; unregister modules whose hashes no longer
  back any surviving group.
- Durably: `delete_package` per version, plus a NEW
  `PackageStore::delete_package_route` — absent today
  (`crates/aion-store/src/package.rs:50-105`), and `DELETE_PACKAGE_SQL` touches
  only the `packages` table.

It must refuse on the same pin set unload already uses, union'd over ALL versions
rather than one: in-flight start pins, live non-terminal handles,
`list_active()` ∪ `list_paused()` runs whose `WorkflowStarted` names the type,
and recorded-but-unstarted children. Keeping BOTH scans matters — `list_active()`
projects `Running` only, so dropping the paused scan would strand paused runs.

It must not break: multi-entry archive groups (removing one member must not
orphan a sibling sharing the group's beams), long-lived runs pinned to old
versions, or status-is-a-projection — catalog and package state only, no history
deletion, no synthesized terminal events, and the durable delete must precede
module unregistration so a failure cannot leave a version gone from the process
but resurrected next boot.


## Adjacent defects found, out of scope for this port

### A terminal failure that calls itself retryable

The 28-day run's recorded terminal failure is, verbatim from durable history:

```json
{"kind":"Terminal","message":"retryable:harness reported failure: ..."}
```

The retry-exhaustion path flips `kind` to `Terminal` but leaves the
`retryable:` prefix in the message text. **An operator reading a terminal
failure is told it was retryable.** That is a false statement in the recorded
bytes of a durable event, which is the one place in this system that is
supposed to be beyond doubt. Engine/worker-side; not fixed here.

### The agent layer is not as durable as the engine

Recorded in full in `DURABLE-AGENT-SESSION-EPOCH-FINDING.md`. In short: Aion
parked a workflow for 28 days and resumed it flawlessly, but the norn session
underneath could not survive a provider epoch change, and the worker never
asked for permission to resume across one. Fixed worker-side in this branch
with an opt-in `--norn-allow-degraded-session` and a refusal that teaches, but
the general point stands: **any durable agent that parks longer than a provider
epoch hits this**, and nothing in the engine's own durability protects against
it.
