# AWL workflow documents

One `.awl` file declares exactly one workflow. The always-current reference
is `aion awl guide` (`<word>` for one word with a checked example,
`--reference` for the whole embedded language reference); this file is the
orientation layer over it.

## Document anatomy

```awl
//! Doc comment describing the workflow.        (§1 Document structure)
workflow my_flow

  input repo: String                            (§3 Workflow header)
  input attempts: Int
  timeout 2h                                    // bounds the whole run
  outcome done: type Report, route success      // terminal payloads map to
  outcome refused: type Refused, route failure  // engine statuses here

type Report  { text: String, exit_code: Int }   (§2 Values and types)
type Refused { reason: String }

worker tools                                    (§4 Workers, actions, calls)
  action inspect(repo: String) -> Report
    node inspector, timeout 10m, retry 3 backoff 5s..1m

step main                                       (§6 Steps)
  inspect(repo: repo) -> report
  outcome ok:  when report.exit_code == 0, route done(text: report.text, exit_code: report.exit_code)
  outcome bad: otherwise, route refused(reason: report.text)
```

Everything in the header is contract: `input` fields form the start contract
(derive its JSON Schema with `aion awl schema FILE.awl`), `outcome`
declarations name every way the run can end and which engine status each
maps to (`route success` / `route failure`).

## The language map (13 sections, from `aion awl guide`)

| § | Covers | Key words |
|---|---|---|
| 1 | Document structure | `workflow` |
| 2 | Values and types | `type`, `schema`, `const`, `json`, `of`, `true`, `false` |
| 3 | Workflow header | `input`, `signal`, `success`, `failure` |
| 4 | Workers, actions, calls | `worker`, `action`, `timeout`, `retry` (`every` / `backoff`), `advisory`, `agent`, `node`, `run` |
| 5 | Children and subflows | `child`, `spawn`, `subflow` |
| 6 | Steps, dependencies, collections | `step`, `after`, `filter`, `map`, `sort`, `count`, `any`, `all`, `is` (`empty`/`present`/`absent`), `not`, `and`, `or` |
| 7 | Selection, outcomes, routes | `outcome`, `when`, `otherwise`, `route` |
| 8 | Parallel and sequential work | `fork`/`join`, `sequential`, `distribute`, `sequence`, `collect`, `in` |
| 9 | Bounded iteration | `loop`, `counting`, `until`, `max`, `visits` |
| 10 | Waits, timeouts, signals, sleep | `wait`, `sleep` |
| 11 | Failure handling | `on` (failure blocks) |
| 12 | Sending signals | `send`, `to` |
| 13 | Read-only queries | `query`, `answer` |

## Patterns worth knowing by heart

**Pipelines** thread a value through calls and field accesses:

```awl
step greet_and_shout
  name |> greet |> .greeting |> shout |> route shouted
```

**Selection** — the first true `when` wins; `otherwise` must come last and
completes the arm set. Guards can prove optionals present, and that proof
narrows `T?` to `T` inside the winning arm:

```awl
step decide
  outcome proven: when control.text == control_expect, route build
  outcome broken: otherwise,
    route refused(reason: "control did not answer as expected")
```

**Flow loops** — a step can route back to an earlier step; bound revisits
with `max N visits` and read the count in guards:

```awl
step check
  outcome more:     when visits < poll_cycles, route pace
  outcome finished: otherwise, route done(...)
```

**Value loops** (§9) rebind a threaded value until a condition, with a
mandatory `max` ceiling. **Parallelism** (§8): `fork item in items … join`
inside one step, or `distribute … collect` for a multi-step region that runs
one track per element — real parallel children. **Signals** (§10/§12): declare
inbound with `signal` in the header and take delivery with `wait` (a
`timeout` on the wait makes the binding optional — expiry is absence);
declare outbound with `send` and address a running workflow by id with
`send … to`. **Queries** (§13): `query` declares what an operator may ask a
running instance; `answer` computes it from bindings in scope.

## Action configuration (§4)

One config line per action: `node <name>` pins dispatch to a worker node,
`timeout <dur>` is a real armed per-attempt deadline, `retry N every D` or
`retry N backoff MIN..MAX` set retry policy. `advisory` marks an action whose
failure never faults its step. `agent` marks an agent seam: one `String`
prompt parameter in, and the declared result must be the canonical
agent-outcome record — exactly four fields, `text`, `final_message`,
`stop_reason`, `session_id`:

```awl
type Reply { text: String, final_message: String, stop_reason: String, session_id: String }

worker dev
  action delegate(repo: String, prompt: String) -> Reply
    agent, timeout 90m, retry 3 backoff 30s..5m
```

Retries on agent actions absorb provider outages, not build failures — a
failed build is a reply your guards branch on, never an error.

## Determinism rules

Workflow expressions re-execute on replay, so they must be pure: `map`,
`filter`, `sort` (keys must be `Int`/`Float`/`String` — the checker refuses
anything unordered), `count`, `any`/`all`. Anything that touches the world is
an action. Timers (`sleep`, `timeout`, `wait` deadlines) are durable server
timers, not process clocks — a workflow can sleep across server restarts.

## The loop

```sh
aion awl check FILE.awl     # until clean; errors are file:line:col
aion awl fmt FILE.awl       # canonical form, in place
aion run FILE.awl --input '{...}' [--timeout 5m]
```

`--timeout` bounds only your wait — the run continues server-side; cancel it
to stop it. `aion deploy` ships without starting. Deployed packages archive
their source (provenance only, excluded from the content hash), and the ops
console shows it read-only.
