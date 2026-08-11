# AWL-AUTHORING.md — from objective to checked workflow

This is the practical authoring path. The authored artifact is one or more
`.awl` files. Read `AWL-REFERENCE.md` for the language contract,
`WORKERS.md` before declaring actions, and `COMMANDS.md` before deployment.
For one word of the language at the desk, `aion awl guide WORD` prints its
meaning, a complete checked example, and the reference section that explains
it.

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

Read `WORKERS.md` before promising an implementation. Five things can serve a
declared action, and choosing between them is an authoring decision:

1. **The document itself**, with a declarative `run "…"` body — the server
   executes the command and no worker serves that action:

   ```awl
   type RunOutcome { exit_code: Int, stdout: String, stderr: String }

   worker greeter
     action greet(name: String) -> RunOutcome
       run "echo hello $name"
   ```

   The result type must be a record over a subset of
   `{ exit_code: Int, stdout: String, stderr: String }`, only the action's own
   parameters interpolate (`$name` / `${name}`), the program must be written
   out, and there is no shell. **Read `docs/workers/DECLARED-COMMANDS.md` first: there
   is no sandbox.** Never propose one for work the operator has not agreed can
   run with the server's full privileges.

2. **A worker document** — a second `.awl` file whose opening declaration is
   `worker <queue>`, carrying a command body on every action, served by
   `aion worker awl WORKER_FILE.awl`. The commands run on the WORKER's host,
   not the server's, and the `run text` / `run json` capture selectors are
   available there (see `AWL-REFERENCE.md` §16 and
   `docs/workers/AWL-WORKER-DOCUMENTS.md`).

3. **An agent harness**, with the `agent` marker — the action's one `String`
   input is a prompt and its output is the canonical agent-outcome record, a
   record you declare with exactly `text: String` and `stop_reason: String`:

   ```awl
   type Reply { text: String, stop_reason: String }

   worker assistants
     action ask(prompt: String) -> Reply
       agent, timeout 10m
   ```

   `aion worker agent FILE.awl` serves it without anyone writing a crate —
   once the same `worker` block carries a `harness` section saying how to
   launch the agent (`kind`, `concurrency`, the reconnect trio, `env_pass`,
   and the per-kind settings). None of that is a flag; `aion awl check` names
   anything the section is missing. See `WORKERS.md` and
   `docs/workers/AGENT-WORKERS.md`. `agent` and a `run` body are mutually
   exclusive.

4. **`aion worker shell`**, for host commands wired by a strict manifest.

5. **A real worker process** — `aion awl scaffold FILE.awl --out ./workers`
   generates a compiling Rust crate per `worker` block, with every advertised
   schema taken from the compiled contract rather than re-derived.

An action with no body and no marker is a requirement on an out-of-band worker.
`run` commands are the only bodies the language admits — a worker document
gives every one of its actions a command body, never arbitrary code.

The one other declaration-level marker is `advisory`: an advisory action never
faults its calling step and is not a completion dependency, so it cannot bind a
result. Like `agent`, it classifies the declaration and cannot be set at a call
site.

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

For multiple statements within one step, `fork ... join` is accepted. The
collection form (`fork item in items`) is parallel; add `sequential` to
process collection items one at a time in input order. A bare `fork` — the
header with no collection — opens named branches for heterogeneous calls.

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

The opening step contains only its region operation; the closing step's
`collect` must come first, and further statements may follow it
(`crates/aion-awl/tests/fixtures/rev2/flow-shape/valid/sequence_region_loopback.awl`
counts and judges the collected list in the same step). Use
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

## 7a. Talk to the outside: `send` and `query`

`wait` receives. Two further header declarations cover the other directions,
and both are in the language today — never say otherwise.

**`send` signals another running workflow**, addressed by its durable id:

```awl
//! Announce an item to its coordinator.
workflow item_announcer
  input coordinator_id: String
  input item_id: String
  send progress: Progress
  outcome done: type Progress, route success

type Progress { item: String, state: String }

step announce
  send progress(item: item_id, state: "started") to coordinator_id
  route done(item: item_id, state: "started")
```

The target is a workflow id, which is a `String` — `workflow.id`, an input, or
any bound `String`. Payload forms mirror route payloads: named fields, a bare
value, or nothing at all when the declared type is `Nil`. A name declared as an
inbound `signal` is not sendable; declare the outbound direction too.

**`query` declares what an operator can ask a RUNNING instance**, and `answer`
registers the value:

```awl
//! Report review progress to whoever asks.
workflow review_reporter
  query review_status: Status
  outcome done: type Status, route success

type Status { phase: String }

step announce
  answer review_status(phase: "announcing")
  route done(phase: "announcing")
```

Every declared query must be answered somewhere in the document or the check
fails. Re-register an answer inside a loop to make a long-running run report
live progress. Operators read it with `aion query WORKFLOW_ID QUERY_NAME`, and
`aion awl schema FILE.awl --queries` emits every answer's schema. A completed
run no longer answers live queries.

See `AWL-REFERENCE.md` §12 and §13 for the full rules.

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
checks; something must also serve every declared queue.

`aion run --timeout 5m` bounds how long the COMMAND waits, written as an AWL
duration literal. There is no default: absent the flag, the wait is bounded
only by the document's own `timeout` declaration, and a document declaring
neither waits indefinitely. Reaching the bound stops the wait, never the run —
the workflow keeps executing on the server, so never report a `--timeout` exit
as a failed or cancelled workflow.

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
- Sending signals:
  `crates/aion-awl/tests/fixtures/rev2/step-bodies/valid/send_signal.awl`.
- Query and answer, every admitted form:
  `crates/aion-awl/tests/fixtures/rev2/step-bodies/valid/query_answer.awl`.
- Declared `run` bodies:
  `crates/aion-awl/tests/fixtures/rev2/declarations/valid/action_run_bodies.awl`.
- The `agent` marker:
  `crates/aion-awl/tests/fixtures/rev2/declarations/valid/agent_seam.awl`.
