# AWL-REFERENCE.md — checked workflow authoring surface

This is the assistant's AWL reference: the workflow-language surface accepted
by the repository's `aion-awl` parser and checker. It is distilled from
`crates/aion-awl/src/`, checker-valid rev-2 fixtures, and deployed `.awl`
examples. If an installed `aion awl check` disagrees, its diagnostic and that
binary's source win.

## 1. Document structure

One `.awl` file declares one workflow. Use two-space indentation. The canonical
order is:

```awl
//! Required workflow narration.
workflow workflow_name
  timeout 2h
  input request: Request
  signal approval: Approval
  outcome done:   type Result, route success
  outcome failed: type Failure, route failure

const max_rounds = 3

type Request { name: String }
type Result  { message: String }
type Failure { reason: String }
type Approval { reviewer: String }

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
- `signal name: Type` declares a durable external signal.
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

No binding is required when only the action effect matters. Per-action config
supports `node`, `timeout`, and either `retry N every DURATION` or
`retry N backoff MIN..MAX`. A call site may override `node` and `timeout`, but
not retry policy. The override is an indented config line after the call:

```awl
build(repo: request.repo, clean: true) -> built
  node urgent, timeout 5m
```

AWL workflow files declare requirements; they do not contain worker action
bodies. See `WORKERS.md` for current implementation layers.

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

## 12. Source proofs

Every shape above is represented in checker-valid repository inputs:

- Header/types/workers/pipes: `flagship/valid/awl_hello.awl`.
- Worker config and fork modes: `dag-fork/valid/release_pipeline_combined.awl`.
- Regions/subflows/visits: `flow-shape/valid/dev_flow.awl`.
- Wait/sleep: `step-bodies/valid/wait_and_sleep.awl` and
  `step-bodies/valid/wait_timeout_optional.awl`.
- Children/spawn: `declarations/valid/child_spawn_combo.awl`.
- Loops/outcomes/failure/substeps: `loop-outcomes/valid/*.awl`.

Those fixture paths are under `crates/aion-awl/tests/fixtures/rev2/`. Production
examples under `examples/*/awl/` show the same constructs in deployed workflows.
Run `aion awl check` on the actual document after every change.
