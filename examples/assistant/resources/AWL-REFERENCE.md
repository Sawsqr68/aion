# AWL-REFERENCE.md — checked workflow authoring surface

This is the assistant's AWL reference: the workflow-language surface accepted
by the repository's `aion-awl` parser and checker. It is distilled from
`crates/aion-awl/src/`, checker-valid rev-2 fixtures, and deployed `.awl`
examples. If an installed `aion awl check` disagrees, its diagnostic and that
binary's source win.

That precedence rule is live, not theoretical. The published `aion` crate
(0.11.0) carries the full surface described here; if an older installed binary
rejects a construct, upgrade before concluding the construct does not exist.

## 1. Document structure

One `.awl` file declares one workflow. Use two-space indentation. The canonical
order is:

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
optional fields `T?`. Optional means absent; JSON `null` is invalid. `[T?]` is
invalid, while `[T]?` means the whole field can be absent.

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

Imports resolve relative to the `.awl` document. Unsupported schema structure,
missing files, invalid JSON, and null-admitting types are check errors.

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
concatenation. They cannot depend on runtime bindings.

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
or with an already-bound value of that type.

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
| `advisory` | bare marker | The action's failure never faults its calling step, and it is not a completion dependency of its layer. Because it can promise no value, an advisory call cannot bind a result. |
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
a PROMPT and its `String` result carries the REPLY — the whole of what an
adapter needs to hand the call to an agent harness instead of running it as an
ordinary activity.

```awl
worker assistants
  action ask(prompt: String) -> String
    agent, timeout 10m
```

The checker enforces the shape: exactly one parameter, that parameter `String`
(not `String?`), the return type `String` (parse it downstream if the flow
needs structure), and never together with a `run` body — the server runs a
declared body itself and never hands it to a harness. The marker classifies the
declaration and must not change the flow: `agent_seam.awl` and
`agent_seam_unmarked_control.awl` must lower to identical MIR.

`aion worker agent <FILE.awl>` serves these seams without you writing a crate.

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
- **The argument is one quoted string.** `run "<command>"`.
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
Call it like an action and bind the result. It compiles into the containing
workflow; it is not independently deployed or observed. Subflows may nest but
cannot recurse.

Nested `step` declarations are also accepted inside a step body. They share the
parent's inputs and form a scoped graph; routes may not escape that scope.

## 6. Steps, dependencies, and calls

Statements in an ordinary step run in written order. A step with no `after` and
no incoming route falls through from the preceding written step. `after`
creates explicit dependencies:

```awl
step build after prepare
step verify after build, policy
```

A step starts after all named dependencies complete. Independent steps that
become eligible together run concurrently. The checker rejects unknown targets,
cycles through `after`, unreachable steps, and bindings that are not available
on every path.

Bind a call with `->`. Pipe a value with `|>`:

```awl
request |> normalize |> .repo |> inspect -> report
report |> route done
```

A pipe action takes the piped value as its one argument. Available deterministic
value operations include field access, literal indexing (`items[0]`), string
`+`, comparisons, boolean `not`/`and`/`or`, and collection operations
`filter`, `map`, `sort`, `count`, `any`, and `all`. Predicates include
`is empty`, `is present`, and `is absent`. There are no user functions or
general arithmetic; put computation in a worker action.

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
Conditional outcomes must be exhaustive. A route names either another step or
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

Bare `fork` runs branches in parallel and joins results in input order. Add
`sequential` to run one item at a time in input order:

```awl
fork item in items sequential
  publish(repo: item.repo)
join -> receipts
```

Named branches run heterogeneous calls concurrently and use `join` without a
collection binding:

```awl
fork
  fetch_profile(id: request.id) -> profile
  fetch_history(id: request.id) -> history
join
```

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
must be its only statement. `collect` must be the first statement of the close
step and closes the nearest region. Results preserve input order; an empty
input collects to `[]`.

Strict `collect report -> reports` fails the run when an item fails. Tolerant
`collect report? -> reports` preserves one optional slot per input item and
keeps failure detail in history. Regions may nest, but each inner region must
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

Use the shape that matches the work; never emulate an unbounded loop.

## 10. Waits, timeouts, signals, and sleep

Declare a signal in the header, then wait for it:

```awl
step hold
  wait approval -> answer
```

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
workflow gated_item
  input coordinator_id: String
  send heartbeat: Nil
  send progress: Progress
  send verdict: String

type Progress { item: String, state: String }

step announce
  send heartbeat to coordinator_id
  send progress(item: item_id, state: "started") to coordinator_id
  send verdict("gated: " + item_id) to coordinator_id
  send progress(item: item_id, state: "gated") to workflow.id
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
workflow gated_review
  input item_id: String
  input max_passes: Int
  query review_status: Status
  query passes_left: Int
  query heartbeat: Nil
  query notes: [String]
  query last_note: String?

type Status { phase: String, note: String? }

step announce
  answer review_status(phase: "announcing", note: "no reviewer has looked yet")
  answer passes_left(max_passes)
  answer heartbeat
  answer notes([item_id])
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
