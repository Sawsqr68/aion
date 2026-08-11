# AWL-REFERENCE.md — checked workflow authoring surface

This is the assistant's AWL reference: the workflow-language surface accepted
by the repository's `aion-awl` parser and checker. It is distilled from
`crates/aion-awl/src/`, checker-valid rev-2 fixtures, and deployed `.awl`
examples. If an installed `aion awl check` disagrees, its diagnostic and that
binary's source win on everything the check sees — but `check` runs parse and
typecheck only, and a further band of refusals arrives only when the document
is compiled (§15).

That precedence rule is live, not theoretical. This document describes the
checker on the repository's main branch. The most recent release cut (0.12.0)
predates the worker-document family (§16) and the embedded language guide
(`aion awl guide`), so an installed binary at 0.12.0 or earlier refuses those
surfaces while accepting the rest. Upgrade or build from the repository before
concluding a construct does not exist.

## 1. Document structure

One `.awl` file is one document of one FAMILY, and its opening declaration says
which: `workflow <name>` declares a flow (this section and every section up to
§15), `worker <queue>` declares the implementation of one task queue (§16).
Use two-space indentation. The canonical order of a workflow document is:

```awl
//! Required workflow narration.
workflow workflow_name
  timeout 2h
  input request: Request
  signal approval: Approval
  send progress: Progress
  query status: Status
  outcome done:   type Result, route success
  outcome failed: type Failure, route failure

const max_rounds = 3

type Request { name: String }
type Result  { message: String }
type Failure { reason: String }
type Approval { reviewer: String }
type Progress { stage: String }
type Status   { phase: String }

worker queue_name
  action perform(request: Request) -> Result
    node worker_node, timeout 10m, retry 3 every 30s

child independent_review(request: Request) -> Result

subflow inline_review(request: Request)
  outcome out: type Result
  step run
    perform(request: request) -> result
    route out(result)

step first
  perform(request: request) -> result
  result |> route done
```

The sample illustrates declaration placement; adapt the flow rather than
copying unused declarations. A workflow needs at least one terminal outcome and
must not fall off the final step.

`//!` lines immediately before `workflow` narrate the workflow. `///` lines
document the declaration or field that follows. `//` is an ordinary comment.
Identifiers are `snake_case`; nominal type names and enum variants are
`TitleCase`.

## 2. Values and types

Built-ins are `Bool`, `Int`, `Float`, `String`, `Nil`, `Dir`, lists `[T]`, and
optional fields `T?`. Optional means absent; JSON `null` is invalid. `[T?]`
cannot be written as a declared type
(`header-types/invalid/list_element_optional.awl`), while `[T]?` means the
whole field can be absent. The `[T?]` shape itself does exist in one place:
a tolerant `collect binding? -> results` binds exactly it (§8). A `Bool` is
written `true` or `false`; both are reserved words.

Record and enum declarations:

```awl
type Finding { title: String, blocking: Bool, detail: String? }
type Decision = Approve | Revise | Reject
```

Record construction and access:

```awl
Finding(title: "missing gate", blocking: true)
report.findings
```

Schema doors:

```awl
type Existing = schema("schemas/existing.schema.json")
type Inline = schema {
  "type": "object",
  "required": ["value"],
  "properties": { "value": { "type": "string" } }
}
```

Imports resolve relative to the `.awl` document. Missing files, invalid JSON,
and null-admitting types are check errors, and unsupported schema structure is
refused by name: `oneOf`, `anyOf`, `patternProperties`, and an external `$ref`
each carry their own diagnostic (`schema-doors/invalid/import_oneof.awl`,
`import_anyof.awl`, `import_patternprops.awl`, `import_external_ref.awl`,
`import_null_type.awl`). A `$ref` into the schema's own `$defs` is supported
(`schema-doors/valid/import_nested_defs.awl`).

Constants are compile-time values:

```awl
const attempts = 3
const instructions = """
Use the repository evidence. Return only the requested result.
"""
const report_schema = schema of Report
const literal_schema = json { "type": "string" }
```

Constants may use literals, lists, `json` blocks, `schema of Type`, and constant
concatenation. They cannot depend on runtime bindings. `schema of Type` is a
constant-position form only: in a step-body expression it checks clean but does
not lower (§15) — bind it to a `const` and use the constant.

Durations are positive integer literals with `s`, `m`, `h`, or `d`, such as
`30s`, `5m`, `2h`, and `7d`. `workflow.id` is the durable execution identifier.

## 3. Workflow header

- `timeout DURATION` sets the workflow-run maximum. It is optional.
- `input name: Type` is repeatable and defines the start contract.
- `signal name: Type` declares a durable external signal this workflow
  **receives**.
- `send name: Type` declares a signal this workflow **sends** to another
  workflow (§12). Receiving and sending the same name are separate
  declarations; a document may carry both.
- `query name: Type` declares a read-only surface an operator can ask a
  RUNNING instance, and the type of the answer they get back (§13).
- `outcome name: type Type, route success|failure` declares a terminal payload
  and maps it to the engine's completed or failed status.

Terminal routes must supply the declared outcome type, either by construction
or with an already-bound value of that type. The declared type is not
restricted to records: builtins and lists are outcome types too, routed as
value payloads — `route done(note)` against `outcome done: type String`
(`flow-shape/valid/value_route_payload.awl`), or `type [String]`
(`flow-shape/valid/region_pure_decision.awl`). Subflow outcome declarations
take the same type forms.

## 4. Workers, actions, and calls

A worker block imports an external worker contract:

```awl
worker dev
  action build(repo: String, clean: Bool) -> Build
    node builder, timeout 30m, retry 2 backoff 10s..2m
```

The worker name (`dev`) is the task queue. Every worker has at least one action.
Action parameters and return types are checked. Calls use named arguments only:

```awl
build(repo: request.repo, clean: true) -> built
```

No binding is required when only the action effect matters.

### The config line

An action declaration may carry at most ONE indented config line, whose
comma-separated keys are exactly:

| Key | Form | Meaning |
|---|---|---|
| `advisory` | bare marker | The action's failure never faults its calling step, and it is not a completion dependency of its layer. Because it can promise no value, an advisory call cannot bind a result or serve as a pipe stage (§6). |
| `agent` | bare marker | The action is an agent seam (below). |
| `node` | `node NAME` | Pins dispatch to a worker node label. |
| `timeout` | `timeout DURATION` | Per-activity timeout. |
| `retry` | `retry N every DURATION` or `retry N backoff MIN..MAX` | Retry policy. |

`advisory` and `agent` classify the DECLARATION: an action either is one or is
not, so neither can be declared or overridden at a call site. A call site may
override `node` and `timeout` only, as an indented config line after the call:

```awl
build(repo: request.repo, clean: true) -> built
  node urgent, timeout 5m
```

### Agent seams

`agent` says exactly one thing: the action's single `String` parameter carries
a PROMPT and its result is the canonical agent-outcome record — a record you
declare yourself, under any name, with exactly two fields: `text: String` (the
agent's terminal reply) and `stop_reason: String` (the canonical snake_case
stop reason: `end_turn | tool_use | limit_reached | cancelled | error |
other`). That is the whole of what an adapter needs to hand the call to an
agent harness instead of running it as an ordinary activity, and it is the
shape every harness emits: each maps its native terminal outcome onto the
record at its own boundary, so the flow reads `reply.text` — and can route on
`reply.stop_reason` — without ever parsing harness-specific JSON.

```awl
//! Ask an agent one question and route its reply text.
workflow ask_the_agent
  input question: String
  outcome done: type String, route success

type Reply { text: String, stop_reason: String }

worker assistants
  action ask(prompt: String) -> Reply
    agent, timeout 10m

step ask
  ask(prompt: question) -> reply
  route done(reply.text)
```

The checker enforces the shape: exactly one parameter, that parameter `String`
(not `String?`), the return type a declared record of exactly `text: String`
and `stop_reason: String` (an empty `text` on a completed run is honest — an
agent that finished its turn emitting only tool calls said nothing), and never
together with a `run` body — the server runs a declared body itself and never
hands it to a harness. The marker classifies the declaration and must not
change the flow: `agent_seam.awl` and `agent_seam_unmarked_control.awl` must
lower to identical MIR.

`aion worker agent <FILE.awl>` serves these seams without you writing a crate,
launching the agent from the `harness` section the same `worker` block carries
(§16).

### Declared command bodies

An action may carry a `run` body, which means **the document is the
implementation**: the server executes the command itself and no worker serves
that action.

```awl
type RunOutcome { exit_code: Int, stdout: String, stderr: String }
type Stdout     { stdout: String }
type Greeting   { text: String }

worker greeter
  /// The document IS the implementation.
  action greet(name: String) -> RunOutcome
    run "echo hello $name"
  /// No body: an out-of-band worker must serve this one.
  action polish(text: String) -> Greeting
  /// A body and a config line coexist; the config line prints first.
  action shout(text: String, mark: String) -> Stdout
    node shell, timeout 30s
    run "printf %s%s ${text} $mark"
```

Rules the checker enforces:

- **One body per action.** There is no multi-command form. To run several
  commands, name a shell explicitly — `run "sh -c 'a | b'"` — which is visible
  in review, and that is the point.
- **The argument is one quoted string.** `run "<command>"`. In a WORKFLOW
  document that is the only form: the capture selectors `run text "…"` and
  `run json "…"` are worker-document vocabulary (§16) and are refused here,
  because the server's dispatcher produces the outcome record and nothing
  else.
- **Interpolation is the action's own parameters only**, as `$name` or
  `${name}`, in snake_case. `$HOME` is refused: a shell variable is not an AWL
  parameter. A value is substituted as one WHOLE argv element and never
  re-enters a parser, so a value can never become code.
- **The program itself must be written out.** `run "$program arg"` is refused.
- **The result type is a record over a SUBSET of the fixed outcome object**
  `{ exit_code: Int, stdout: String, stderr: String }` — exactly those names,
  exactly those types. A non-record return, an unknown member, or a member with
  the wrong type is refused.
- Shell syntax in the command does NOT work: `run "a | b"` passes `|` and `b`
  as arguments to `a`; `run "x > f"` does not redirect.

**Read `docs/workers/DECLARED-COMMANDS.md` before writing one. THERE IS NO SANDBOX** —
no allowlist, no filesystem confinement, no network restriction, no privilege
drop, no resource limit. A declared command can do anything the server's OS
user can do. What protects you is review, not runtime enforcement: the command
is source you can read before deploying, and it is sealed into the package's
worker contract, so it cannot change afterwards without producing a different
package.

An action with NO body and no `agent` marker is a requirement on an out-of-band
worker. See `WORKERS.md` for the implementation layers.

## 5. Children and subflows

A child is a separate durable workflow:

```awl
child review_round(repo: String, round: Int) -> Report

step review
  review_round(repo: request.repo, round: 1) -> report
```

The call waits for the child's result. `spawn review_round(...)` starts a child
without waiting and cannot bind a result. Child deployment and schema matching
are runtime/deploy concerns.

A subflow is an inline reusable graph:

```awl
subflow review_once(repo: String)
  outcome out: type Report

  step run
    inspect(repo: repo) -> report
    route out(report)
```

A subflow has typed parameters, exactly one typed outcome, and its own steps.
An invocation runs as its own step: the call must be the ONLY statement of a
top-level step body — never beside other statements, never inside a `fork`,
`loop`, `on failure` block, or substep, and never a pipe stage — and binds the
outcome value with `->` (`flow-shape/valid/dev_flow.awl`). It compiles into
the containing workflow; it is not independently deployed or observed.
Subflows may nest but cannot recurse.

Nested `step` declarations are also accepted inside a step body. They share the
parent's inputs and form a scoped graph; routes may not escape that scope.
Substeps compile only as the TRAILING block of an ordinary step body — every
statement after the first substep must be another substep — and the parent
step must carry outcome clauses to route on after the group settles. A substep
cannot nest further substeps, cannot carry `after` dependencies or a
`max … visits` bound, and cannot sit inside a loop body, fork branch, or
`on failure` block. These substep shape rules are enforced when the document
is compiled, not by `aion awl check` (§15).

## 6. Steps, dependencies, and calls

Statements in an ordinary step run in written order. A step with no `after` and
no incoming route falls through from the preceding written step. `after`
creates explicit dependencies:

```awl
step build after prepare
step verify after build, policy
```

A step starts after all named dependencies complete. Independent steps that
become eligible together compile to one parallel dispatch only when EVERY
member of the layer is a pure dispatch: no outcome clauses, no `on failure`,
no `max … visits`, a body of exactly one call statement, and that call naming
a declared action — not a child, not an advisory action. A layer with any
other member runs in written order instead. The checker rejects unknown
targets, cycles through `after`, unreachable steps, and bindings that are not
available on every path.

Bind a call with `->`. Pipe a value with `|>`:

```awl
request |> normalize |> .repo |> inspect -> report
report |> route done
```

A pipe action takes the piped value as its one argument, so a stage must be
declared with exactly one parameter — a multi-parameter action or child cannot
be a stage, and neither can an advisory action
(`declarations/invalid/advisory_pipe_stage.awl`). A plain expression also
binds as a statement with the same arrow — `"run: " + result -> summary`
(`ergonomics/valid/flow_vocab_b1.awl`). Available deterministic
value operations include field access, literal indexing (`items[0]`), string
`+`, comparisons, boolean `not`/`and`/`or`, and collection operations
`filter`, `map`, `sort`, `count`, `any`, and `all`. Predicates include
`is empty`, `is present`, and `is absent`. There are no user functions or
general arithmetic; put computation in a worker action.

### Guards narrow optionals

A guard that proves an access path present narrows that path from `T?` to `T`
wherever the proof holds. Within one boolean chain, `and` proves each
conjunct's `is present` for the operands after it, so
`when nudge.end is present and nudge.end` typechecks, and proofs chain — `a is
present and a.b is present` narrows `a.b` because the first proof already made
`a` a record (`step-bodies/valid/guard_chain_carries_every_proof.awl`). `or`
proves each disjunct's `is absent` on its false edge:
`when detail is absent or detail.approved` reads `.approved` only where
`detail` is present (`schema-doors/valid/short_circuit_optional.awl`).

An outcome guard's proofs hold through its whole arm, including the route
payload (`step-bodies/valid/guard_narrows_an_optional_field.awl` and
`guard_narrows_a_deep_optional_field.awl`). Narrowing keys on the exact access
path, not just on whole bindings. Proofs never escape their arm and never flow
backwards to statements before the guard; an optional used without a proof is
a check error (`loop-outcomes/invalid/optional_used_unguarded.awl`,
`step-bodies/invalid/narrowing_does_not_escape_its_arm.awl`,
`step-bodies/invalid/proof_does_not_flow_backwards.awl`).

## 7. Selection, outcomes, and routes

There is no `select` keyword. A checked selection is an ordered set of step
outcomes:

```awl
step choose
  outcome blocked: when findings |> any(.blocking), route remediation
  outcome clear: otherwise,
    route done(summary: "all clear")
```

The first true `when` wins. `otherwise` is the complement and must be last.
Conditional outcomes must be exhaustive. Exhaustive normally means an
`otherwise` arm, with one proved exception: when every arm's guard compares
the SAME subject against a bare enum variant and every variant of the enum is
covered, the set is total and needs no `otherwise`
(`loop-outcomes/valid/enum_when_totality.awl`); an uncovered variant is a
check error naming that variant
(`loop-outcomes/invalid/enum_variant_uncovered.awl`). Enum variants are
referenced bare in expressions as `TitleCase` names — `result.category ==
Urgent`. A route names either another step or
a workflow/subflow outcome. A body-less step containing only outcomes is a pure
decision. A route can also be an ordinary final statement:

```awl
route next_step
```

Backward routes are flow loops and require a real bound with `max N visits` on
a step in the cycle. The `visits` binding is available in guards.

## 8. Parallel and sequential work

### Intra-step `fork` / `join`

Collection form:

```awl
fork item in items
  inspect(repo: item.repo)
join -> reports
```

The collection form runs one branch per item in parallel and joins results in
input order. Its body compiles only as exactly one unbound call, which may
name an action or a child — a per-item child fan-out is a checked, lowered
shape (`dag-fork/valid/child_collection_fork.awl`). Add `sequential` to run
one item at a time in input order:

```awl
fork item in items sequential
  publish(repo: item.repo)
join -> receipts
```

A bare `fork` — the header with no collection — opens named branches, which
run heterogeneous calls concurrently and use `join` without a collection
binding:

```awl
fork
  fetch_profile(id: request.id) -> profile
  fetch_history(id: request.id) -> history
join
```

A named branch compiles only as an action call (a result binding is fine);
child calls in named branches do not lower. Both fork body shapes are
compile-time rules, not check-time (§15).

### Multi-step `distribute` / `sequence` regions

Use a region when every item needs a graph of steps:

```awl
step open
  distribute item in items

step work
  inspect(repo: item.repo) -> report

step close
  collect report -> reports
```

`distribute` executes item tracks in parallel. Replace it with `sequence` to
execute tracks one at a time in collection order. The opener's region operation
must be its only statement. `collect` must be the FIRST statement of the close
step and closes the nearest region — further statements may follow it
(`flow-shape/valid/sequence_region_loopback.awl` counts and judges the
collected list in the same step). Results preserve input order; an empty
input collects to `[]`.

Strict `collect report -> reports` fails the run when an item fails. Tolerant
`collect report? -> reports` binds `reports` as `[Report?]` — one optional
slot per input item, in item order, with failure detail kept in history. That
is the one way the `[T?]` shape exists (§2); each slot is an optional and must
be proved present before its fields are read, exactly like any other `T?`
(§6). Regions may nest, but each inner region must
close before its outer region; routes cannot enter or escape an open region.

## 9. Bounded iteration

Value-threading loop:

```awl
loop round = initial counting cycles
  improve(prior: round) -> round
  until round.complete
  max max_rounds
```

The body runs at least once and must rebind the one threaded value. `max` is
mandatory. `counting` is optional and binds the number of completed iterations.
A step containing a loop needs explicit outcomes that distinguish completion
from exhaustion. `route` is not allowed inside the loop body.

Flow loops use backward routes plus a step-level visit bound:

```awl
step review
  inspect(repo: request.repo) -> report
  outcome again: when report.blocking, route revise
  outcome done: otherwise, route finished(report)
  max 3 visits
```

The visit bound is an expression over inputs and consts, not only a literal —
`max max_polish_rounds visits`
(`loop-outcomes/valid/backward_route_bounded_cycle.awl`).

Use the shape that matches the work; never emulate an unbounded loop.

## 10. Waits, timeouts, signals, and sleep

Declare a signal in the header, then wait for it:

```awl
step hold
  wait approval -> answer
```

The binding is mandatory: `wait approval` without `-> answer` is a parse error
(`step-bodies/invalid/wait_missing_binding.awl`).

A timed wait binds an optional value because expiry is absence:

```awl
step hold
  wait approval timeout 2h -> answer
  outcome approved: when answer is present, route continue_work
  outcome expired: otherwise, route failed(reason: "approval timed out")
```

`sleep 30s` is a durable timer statement. Action timeout belongs on an action or
call; workflow timeout belongs in the header; wait timeout belongs on `wait`.
They are distinct.

## 11. Failure handling

An action failure fails its step after declared retries. Optional compensation:

```awl
on failure
  remove_assets(ids: assets)
  route failed(reason: "publish failed after compensation")
```

`on failure` must end in a route; silently swallowing failure is invalid.
External-command exit status may instead be modeled as ordinary typed action
data when the workflow needs to decide what it means.

## 12. Sending signals to other workflows

`wait` receives a signal. `send` is the other direction: this workflow signals
another running workflow, addressed by its durable id.

Declare what you send in the workflow header, then send it from a step body:

```awl
//! Announce an item to its coordinator as it is gated.
workflow gated_item
  input coordinator_id: String
  input item_id: String
  send heartbeat: Nil
  send progress: Progress
  send verdict: String
  outcome done: type Progress, route success

type Progress { item: String, state: String }

step announce
  send heartbeat to coordinator_id
  send progress(item: item_id, state: "started") to coordinator_id
  send verdict("gated: " + item_id) to coordinator_id
  send progress(item: item_id, state: "gated") to workflow.id
  route done(item: item_id, state: "gated")
```

The payload takes the same three forms a route payload does:

| Form | When |
|---|---|
| `send name(field: v, …) to target` | the declared type is a record |
| `send name(value) to target` | any value assignable to the declared type |
| `send name to target` | **only** when the declared type is `Nil` |

The bare form is admitted for `Nil` alone, and it deliberately does not pick up
a same-named binding.

Rules:

- The name must be declared with `send name: Type` in **this** document's
  header. A name declared as an inbound `signal` is refused with a diagnostic
  telling you to add the `send` declaration; a document may declare both
  directions of the same name.
- The target is a **workflow id, which is a `String`** — `workflow.id`, an
  input, or any bound `String`. It is not required to be a declared child.
- The payload belongs to the signal, not to the target: writing it after `to`
  is caught by name.

What the checker CANNOT prove is the far side's agreement: the target is a
runtime id, so an unresolvable target and a target with no matching receiver
are runtime facts. A live target that never awaits the signal is the one quiet
case — the signal lands in its history and nothing consumes it.

`send` and `to` are positional words, not reserved: `action send(to: String,
…)` and `step send` keep working.

## 13. The read-only query surface

`query` declares what an operator can ask a RUNNING instance, and the type of
the answer. `answer` registers the value, computed from the bindings in scope
at that point.

```awl
//! Review an item, reporting live progress to whoever asks.
workflow gated_review
  input item_id: String
  input max_passes: Int
  query review_status: Status
  query passes_left: Int
  query heartbeat: Nil
  query notes: [String]
  query last_note: String?
  outcome done: type Status, route success

type Status { phase: String, note: String? }

step announce
  answer review_status(phase: "announcing", note: "no reviewer has looked yet")
  answer passes_left(max_passes)
  answer heartbeat
  answer notes([item_id])
  answer last_note(item_id)
  route done(phase: "announced")
```

`answer` takes the same payload forms as `send`: named record fields, a bare
value, or nothing at all when the declared type is `Nil`.

An answer may be re-registered anywhere a statement is legal — inside a `loop`
body, inside `on failure`, in a substep — and re-registering inside a loop is
exactly what makes a long-running run report live progress:

```awl
step gate after announce
  loop verdict = Decision(approved: false)
    answer review_status(phase: "reviewing")
    evaluate(item: item_id) -> verdict
    until verdict.approved
    max max_passes
  answer review_status(phase: "settled", note: item_id)
  outcome settled: when verdict.approved, route done(phase: "settled")
  outcome exhausted: otherwise, route gave_up(item: item_id)
  on failure
    answer review_status(phase: "failed", note: "the gate faulted")
    route gave_up(item: item_id)
```

Rules:

- Every `answer` must name a query the header declares.
- **Every declared query must be answered somewhere in the document**, or the
  check fails. The traversal is syntactic, not flow-reachability, and descends
  into subflows, fork bodies, loop bodies, substeps, and `on failure` blocks.
- The answer's value is checked against the declared type.

Two stated limits: an `answer` in an unreachable step, and an `answer` inside a
per-item region that lowers to a synthesized child, both satisfy the
"answered" rule while registering nothing an operator querying the parent can
see.

Operators read the surface with `aion query <workflow-id> <query-name>`, and
`aion awl schema FILE.awl --queries` emits the schema of every answer. A
completed run no longer answers live queries — inspect a terminal run with
`aion describe`.

`query` and `answer` are positional words, not reserved: `action
parse_output(…, query: String)` and `wait decision -> answer` keep working.

## 14. Source proofs

Every shape above is represented in checker-valid repository inputs:

- Header/types/workers/pipes: `flagship/valid/awl_hello.awl`.
- Worker config and fork modes: `dag-fork/valid/release_pipeline_combined.awl`.
- Regions/subflows/visits: `flow-shape/valid/dev_flow.awl`.
- Wait/sleep: `step-bodies/valid/wait_and_sleep.awl` and
  `step-bodies/valid/wait_timeout_optional.awl`.
- Children/spawn: `declarations/valid/child_spawn_combo.awl`.
- Loops/outcomes/failure/substeps: `loop-outcomes/valid/*.awl`.
- Sending signals: `step-bodies/valid/send_signal.awl`.
- Query/answer, every admitted form: `step-bodies/valid/query_answer.awl`.
- Declared `run` bodies: `declarations/valid/action_run_bodies.awl`.
- The `agent` marker: `declarations/valid/agent_seam.awl`, with
  `declarations/valid/agent_seam_unmarked_control.awl` as its control.
- The `advisory` marker: `declarations/valid/advisory_side_channel.awl`.
- Value route payloads: `flow-shape/valid/value_route_payload.awl`.
- Subflow-local names: `flow-shape/valid/subflow_local_names.awl`.
- Distribute/collect, strict and tolerant:
  `flow-shape/valid/distribute_child_collect.awl` and
  `flow-shape/valid/sequence_activity_tolerant.awl`.

The refusals matter as much as the shapes; the sibling `invalid/` directory
carries one fixture per rule (for example
`declarations/invalid/run_parameterized_program.awl`,
`declarations/invalid/agent_action_declares_a_body.awl`,
`step-bodies/invalid/query_declared_never_answered.awl`).

Those fixture paths are under `crates/aion-awl/tests/fixtures/rev2/`. Production
examples under `examples/*/awl/` show the same constructs in deployed workflows.
Run `aion awl check` on the actual document after every change.

## 15. What `aion awl check` does not check

`aion awl check` runs the parser and the typechecker, nothing further. The
compile behind `aion awl emit`, `aion deploy`, and `aion run` lowers the
checked document, and lowering refuses a further band of shapes that check
clean, each with a diagnostic naming the shape ("does not yet lower …",
anchored to a line). A clean check is necessary, not sufficient; the compile
has the final word on these.

Inside an `on failure` block, lowering refuses a substep, a per-item region, a
bounded loop, a fork, a timeout wait (an untimed `wait` lowers), and a child
spawn — each named in its diagnostic, for example "a bounded loop inside an
`on failure` block". A guarded step whose body leaves "a fallible result the
`on failure` edge cannot guard in place (a conditional prelude or an
outcome-guard result)" refuses with exactly that phrasing.

Fork body shapes (§8):

- A collection fork body must be exactly one unbound call — "a collection
  fork body beyond one unbound call". That one call may name an action or a
  child (`dag-fork/valid/child_collection_fork.awl`).
- A named-fork branch must be an action call: a child call refuses as "child
  calls inside named fork branches", and any other statement as "a named fork
  branch beyond an action call".

Substeps (§5):

- `after` on a substep parses and checks but refuses at compile — "`after`
  dependencies on substeps".
- `max … visits` on a substep refuses — "a `max … visits` bound on a substep".
- A substep inside a loop body, fork branch, or `on failure` block refuses —
  "a substep outside a step body's trailing block". Substeps lower only as the
  trailing block of an ordinary step body whose parent carries outcome
  clauses.

Literal indexing (`items[0]`) refuses in four positions: a parallel fork
branch ("indexing inside a parallel fork branch" — a `sequential` collection
fork accepts it), a parallel per-item track ("indexing inside a parallel
per-item track" — `distribute` only; `sequence` tracks accept it), an outcome
guard ("indexing inside an outcome guard"), and a visit bound ("indexing
inside a `max … visits` bound").

Three more:

- `sort` requires a comparable key — "`sort` over a non-comparable key (needs
  Int, Float, String)". The checker accepts any accessor; comparability is
  enforced at compile.
- `schema of Type` outside constant position refuses (§2) — bind it to a
  `const` and use the constant.
- A route targeting a step the plan folded into the middle of a fall-through
  chain refuses — "route to a mid-chain step".

## 16. Worker documents

A `.awl` file whose opening declaration is `worker <queue>` is a **worker
document**: the implementation of one task queue. It declares that queue's
actions, each with the command that runs it, plus the types those actions use,
and nothing else.

```awl
//! Required narration, exactly as a workflow document needs one.
worker greeter
  action greet(name: String) -> Greeting
    run json "greeter-tool --json --name $name"
  action shout(text: String) -> String
    run text "tr a-z A-Z"
  action archive(path: String) -> Archived
    run "tar -cf archive.tar $path"

type Greeting { message: String }

type Archived { exit_code: Int, stderr: String }
```

`aion worker awl <FILE.awl>` serves it. The command runs on the WORKER's host,
not on the server's — that is the difference from a workflow document's
declared body (§4).

### What the family refuses, and why

| In a worker document | Refused because |
|---|---|
| `step`, `child`, `subflow` | A worker document runs no flow. |
| `const` | It has no expressions for a compile-time value to feed. |
| A second `worker` block | One document serves one queue: the one its opening declaration names. |
| An action with no body | A bodyless action is a REQUIREMENT, which a workflow document declares. |
| `timeout`, `retry`, `node`, `advisory`, `agent` | Each configures how the CALLER dispatches the action, so each belongs to the action's declaration in the workflow document. |

Every refusal names the remedy. A clean check reports
`ok: <file> (worker `<queue>`, N actions)` — there are no steps to count.

`aion awl emit`, `aion deploy`, `aion run` and `aion awl schema` (without
`--type`) all refuse a worker document by family: it declares no workflow, so
there is no module to emit, nothing to start, and no start contract to derive.

### Capture selectors

The word between `run` and the command decides what a SUCCESSFUL command
returns. It decides nothing else — a non-zero exit is retryable with the
command's stderr, an unspawnable program and a cancellation are terminal, in
every form.

| Form | Result | Declared return type |
|---|---|---|
| `run "…"` | The outcome record `{ exit_code: Int, stdout: String, stderr: String }` | A record over any subset of those members |
| `run text "…"` | Trimmed stdout as one string | `String` |
| `run json "…"` | Trimmed stdout parsed as JSON | Any declared type except `Nil`; output that does not parse is a TERMINAL failure |

Interpolation, the written-out program rule, and the absence of a shell are
exactly as §4 states them: one implementation, two families.

### The two files, and what holds them honest

The workflow document declares `action greet(name: String) -> Greeting` as a
requirement; the worker document declares the same signature with a body. What
keeps them equal is not diligence but ADMISSION: the worker's advertised
surface is derived through the same seam the deploy path uses, and the server
refuses a worker whose surface differs from the queue's deployed contract,
naming the action and the field. `aion worker awl <FILE.awl> --check` prints
that surface, and the command each action runs, without connecting.

**Read `docs/workers/AWL-WORKER-DOCUMENTS.md` before writing one. THERE IS NO
SANDBOX** — the same boundaries `docs/workers/DECLARED-COMMANDS.md` states,
on the worker's host instead of the server's.

### The `harness` section

A `worker` block may also carry a `harness` section: the launch settings for an
AGENT worker serving that queue. It sits before the block's `action`
declarations, one setting per line. Every word in it — `harness` itself and each
field name — is an ordinary identifier rather than a reserved word, so nothing
else in the language loses those names.

```awl
worker assistants
  harness
    kind acp
    concurrency 4
    reconnect_initial_backoff 5s
    reconnect_max_backoff 2m
    reconnect_max_attempts 100
    env_pass "PATH", "HOME"
    command "/usr/local/bin/claude-code-acp"
    cwd "/srv/agents/workspace"
    permission "deny"
    exit_grace 10s
  action ask(prompt: String) -> Reply
    agent
```

| Setting | Kind | Value |
|---|---|---|
| `kind` | both, required | the bare word `acp` or `norn` |
| `concurrency` | both, required | a whole number greater than zero |
| `reconnect_initial_backoff` | both, required | a positive duration |
| `reconnect_max_backoff` | both, required | a positive duration, not shorter than the initial |
| `reconnect_max_attempts` | both, required | a whole number greater than zero |
| `env_pass` | both, required | quoted variable NAMES, comma-separated |
| `args` | both, optional | quoted strings, comma-separated, order preserved |
| `command` | `acp` only, required | a quoted ABSOLUTE path |
| `cwd` | `acp` only, required | a quoted ABSOLUTE path |
| `permission` | `acp` only, required | `"deny"` or `"allow-once"` |
| `exit_grace` | `acp` only, required | a duration; `0s` is legitimate and means close stdin, then kill at once |
| `binary` | `norn` only, optional | a quoted path; omitted takes the adapter's own `norn`-on-`PATH` default |

The required set is the DECLARED KIND's. An `acp` section carrying no `binary`
is complete, a `norn` section carrying no `command`, `cwd`, `permission` or
`exit_grace` is complete, and a setting belonging to the kind the section does
not declare is refused BY NAME rather than ignored — a setting that silently
does nothing is how an operator comes to believe a policy is in force when it is
not. Nothing is defaulted: `aion awl check` names every required setting the
section omits.

`env_pass` is the agent subprocess's WHOLE environment. The document names the
variables, the machine running the worker supplies their values, and a variable
the list does not name is absent in the agent whatever the worker's own
environment carries. Entries are names only; one containing `=` is refused,
because a value written into a document is a secret in a file that gets
committed, diffed and deployed.

A list does not wrap — a comma at the end of a line is a parse error — and
`aion awl fmt` prints the fields in the fixed order above whatever order they
were written in. The section is launch configuration and not flow meaning: a
document carrying one lowers to the same MIR as the same document without it.

**Where it goes.** A `harness` section says how to launch an agent for the
actions on a queue that nothing else runs, so a queue whose actions ALL carry
declared bodies is refused, naming them: there is nothing there for an agent to
serve. A worker document requires a body on every action, so in practice the
section lives in the `worker` block of a WORKFLOW document — the one place a
bodyless action and a launch can sit together. `aion worker agent <FILE.awl>`
reads it, and has no flag for any of the settings in it.
