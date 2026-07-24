# AWL-AUTHORING.md — from objective to checked workflow

This is the practical authoring path. The authored artifact is one or more
`.awl` files. Read `AWL-REFERENCE.md` for the language contract,
`WORKERS.md` before declaring actions, and `COMMANDS.md` before deployment.

## 1. Start with the contract

Name the workflow, its typed inputs, and every terminal outcome before writing
steps. Use `//!` narration before the workflow header and `///` documentation on
types, fields, workers, actions, and steps. That prose is part of the authored
model and reaches schemas and the console.

This complete minimal workflow is the checked `awl_hello` shape:

```awl
//! Greet a name, then shout it.
workflow awl_hello
  input name: String
  outcome shouted: type Shouted, route success

type Greeting { greeting: String }
type Shouted  { text: String }

worker awl_hello
  action greet(name: String) -> Greeting
  action shout(text: String) -> Shouted

step greet_and_shout
  name |> greet |> .greeting |> shout |> route shouted
```

Write it as `awl_hello.awl`, then check it immediately:

```sh
aion awl check awl_hello.awl
```

A clean check prints `ok: awl_hello.awl (1 step)`. A diagnostic is a source
error to fix, not a warning to route around.

## 2. Declare data once

Use AWL record types, enums, or schema imports. Prefer small nominal records
that make action boundaries and outcomes explicit:

```awl
type Request { repo: String, dry_run: Bool }
type State = Queued | Running | Complete
type Report { summary: String, evidence: [String], note: String? }
```

`?` means a field may be absent; it does not admit JSON `null`. Lists use
`[Type]`. Use `schema("relative/file.schema.json")` only when an existing JSON
Schema is the contract; imports resolve relative to the `.awl` file.

## 3. Declare workers as requirements

A workflow does not implement an action. It declares the task queue and typed
actions a real worker must serve:

```awl
worker review
  action inspect(repo: String) -> Report
    node reviewer, timeout 10m, retry 3 backoff 5s..1m
```

The worker name (`review`) is the task queue. Calls use required named
arguments:

```awl
inspect(repo: request.repo) -> report
```

Read `WORKERS.md` before promising an implementation. AWL worker bodies do not
exist today; command runners are host-native, while substantial workers use the
existing Rust or Gleam worker surfaces.

## 4. Draw the flow with steps

Ordinary statements inside a step run in written order. With no route or
`after`, the next written step is fall-through. Use `after` for DAG edges; steps
that become eligible together can run concurrently.

Use outcomes for selection and routing:

```awl
step decide
  outcome accept: when report.evidence is empty, route done(summary: report.summary)
  outcome revise: otherwise, route revision
```

There is no separate `select` statement. A selection is the ordered,
exhaustive set of `outcome ... when ...` / `otherwise` clauses after a step
body. A terminal route names a workflow outcome; a nonterminal route names a
step.

Use children when the work must be an independently deployed and observable
workflow:

```awl
child review_round(repo: String) -> Report

step review
  review_round(repo: request.repo) -> report
```

Use a `subflow` when the reusable graph should compile inline as part of this
workflow. A subflow has typed parameters, one typed outcome, and its own steps.
See the checked `flow-shape/valid/dev_flow.awl` fixture for the complete shape.

## 5. Choose parallelism explicitly

For multiple statements within one step, `fork ... join` is accepted. Bare
`fork` is parallel; add `sequential` to process collection items one at a time
in input order.

For a multi-step per-item track, use a region. `distribute` runs item tracks in
parallel; `sequence` runs them one at a time. Both must be closed by `collect`:

```awl
step wave
  distribute item in items

step inspect_item
  inspect(repo: item.repo) -> report

step gather
  collect report -> reports
```

The opening and closing steps contain only their region operation. Use
`collect report? -> reports` only when a failed item should produce an absent
slot instead of failing the whole run. Read `AWL-REFERENCE.md` for ordering and
nesting rules.

## 6. Keep repetition bounded

Use the value-threading `loop ... until ... max ...` form for local iteration,
or a backward step route with `max N visits` for a flow cycle. Never invent an
unbounded loop. Loop exhaustion and route decisions must be represented by
explicit outcomes.

## 7. Add durable waiting only through AWL

Declare a signal in the workflow header, then use `wait` in a step. A timeout
makes the result optional and therefore requires a presence/absence decision.
`sleep 30s` is a durable timer. Do not read host time or perform ambient I/O in
workflow flow; world-touching work belongs in worker actions.

## 8. Check continuously, then hand off honestly

After each structural change:

```sh
aion awl check FILE.awl
```

Optionally canonicalize with `aion awl fmt FILE.awl`, inspect the diff, and
check again. When the document is clean, follow `COMMANDS.md`: deploy the
`.awl` file directly and start it with `aion run FILE.awl --input 'JSON'`, or
use the ops console author → check → canvas → deploy → run path. Observe the
real run in the console. Do not report success merely because the source
checks; workers must also be connected on the declared queues.

## Verified exemplars

- Minimal call/pipe/outcome: `examples/awl-hello/awl_hello.awl`.
- Calls, retries, forks, child workflows, and loops:
  `examples/dev-brief/awl/dev_brief.awl`.
- Signals, children, route cycles, and operational narration:
  `examples/remediation-packet/awl/remediation_packet.awl`.
- Subflows and distribute/collect:
  `crates/aion-awl/tests/fixtures/rev2/flow-shape/valid/dev_flow.awl`.
- Wait and sleep:
  `crates/aion-awl/tests/fixtures/rev2/step-bodies/valid/wait_and_sleep.awl`.
