# MCP tasks: the ticket must be a projection of the run, not a record beside it

**Decision brief — 2026-08-19. Author: Vesper Lynd. Routes to Waffles.**

Tom, on being told an awaited MCP task expires after an hour by default:

> "the ticket expiring after default is just … stupid. … we've got all the
> durability we could possibly need. … I better not see anything in there like a
> default, like an assumption for how long something should last, because we
> know that workflows can run multi-day if not forever."

He is right on the merits, and the fix is **not** to raise the default. The
default is a symptom. This brief names the cause and proposes removing it.

## The defect, and why the TTL is a symptom

Three facts, in causal order:

1. **A task-shaped `tools/call` holds an executor.** `tasks/runner.rs::drive`
   spawns a detached tokio task that races the tool future — and for
   `start_run{await_completion}` that tool future is `await_terminal`, a loop
   polling the engine's projected status until it is terminal.
2. **An abandoned task would hold that executor forever.** The runner says so
   itself: *"Racing it here is what makes the advertised TTL true even when
   nobody ever polls: without it, a task whose client walked away would keep its
   executor alive forever."*
3. **So a TTL is required, so a TTL needs a default** —
   `DEFAULT_MCP_TASK_TTL_MS = 3_600_000`.

The TTL is a **resource-leak mitigation wearing policy's clothes**. It is
described in the config as a lifetime choice, and it is really a bound on how
long the server is willing to hold a thread for a client that may have gone
away. That is why one hour looks arbitrary: it *is* arbitrary, because the
question it answers ("how long may I hold an executor?") is not the question it
appears to answer ("how long may work last?").

Directly against CLAUDE.md: **NO ARBITRARY LIMITS / NO ASSUMED DEFAULTS.**

There is a second defect underneath it. `TaskStore` is
`Arc<Mutex<HashMap<String, Entry>>>` — **in memory**. A restart drops every
task while the workflows they track carry on perfectly. In an engine whose whole
thesis is that work outlives its process, the handle to that work does not.

## The observation that dissolves both

For a task whose underlying work is an aion workflow — and today that is *every*
task, because `start_run` is the only task-shaped tool — **every field the MCP
task model needs is already durable, in the workflow's own history.**

| MCP task field | Already durable as |
|---|---|
| `status` | `WorkflowStatus`, itself a projection of history |
| `createdAt` | the `WorkflowStarted` event's recorded timestamp |
| `lastUpdatedAt` | the latest event's recorded timestamp |
| `result` | the run's terminal outcome, rendered as `start_run`'s result |
| `ttlMs` | **`null`** — there is nothing to expire |
| `pollIntervalMs` | a client hint from config; nothing depends on it |

The MCP layer is currently keeping a **second, divergent copy** of a fact aion
already holds — and the copy is the one that dies on restart. *One truth in two
places has already drifted.* The cure is subtraction.

## Proposal

**A run-backed task becomes a projection of its run.** This is load-bearing
invariant #4 (*status is a projection, never a stored mutable field*) applied one
layer up, where it was not being applied.

1. **The `taskId` encodes the run handle** — namespace, workflow id, run id —
   instead of being a random key into a `HashMap`.
2. **`tasks/get` reads the run and projects**, exactly as `describe_run` does.
   No stored state. No executor.
3. **No TTL for a run-backed task.** `ttlMs` is `null`, which is already the
   extension's spelling of "unlimited" and already representable
   (`Task.ttl_ms: Option<u64>`). **Nothing to expire means nothing to default.**
4. **`start_run` stops awaiting.** The start is still performed and confirmed
   before the task is handed back — so there is never a task for a run that
   failed to start — but the waiting is the client's poll, not a server thread.

### What this deletes

- `DEFAULT_MCP_TASK_TTL_MS`, `DEFAULT_MCP_AWAIT_POLL_INTERVAL_MS`
- `mcp.task_ttl_ms`, `mcp.await_poll_interval_ms`
- `tasks/runner.rs` in its entirety
- `TaskStore` and its expiry machinery
- `await_terminal`'s polling loop in `tools/start.rs`

Every deleted item is either an arbitrary number or the machinery that made an
arbitrary number necessary.

### What it buys

- **Survives restart for free.** No persistence code is written anywhere; the
  durability used is the one aion already has and already proves.
- **An unpolled task costs exactly nothing** — no thread, no memory, no entry.
- A months-long workflow is served by the same mechanism as a two-second one,
  with no number anywhere that has to be guessed for either.

## 🔴 The security change this forces, which is an improvement

Today `task_id` is a **bearer token**: `tasks/get` takes no caller identity, and
the id's unguessability *is* the authorization. `model.rs` says so — *"it doubles
as a bearer token for the stored state behind it, so it must be neither
enumerable nor guessable."*

A derived id is constructible by anyone who knows the workflow id. So this
change **requires** that the task methods authorize properly: `tasks/get`,
`tasks/cancel` and `tasks/update` must take `CallerIdentity` and pass the same
namespace guard as `describe_run`.

That is strictly better than what is there now — capability-by-secret-string is
weaker than real authorization, and aion already has real authorization. But it
is load-bearing: **shipping derived ids without moving the authorization is a
leak.** It is not optional and not a follow-up.

## The one open question — for Waffles

**What should `tasks/cancel` mean on a run-backed task?**

Today it fires a cancellation token, the executor drops the tool future, and the
task settles `cancelled` — while the workflow carries on. That is already
documented as deliberate: *"a durable run is not something an MCP client's
disinterest is allowed to kill."*

With no executor there is nothing left to stop waiting on, so the three candidate
meanings are:

- **(a) Acknowledge; change nothing; keep reporting the run's truth.**
  Spec-conformant — the extension explicitly allows a cancelled-requested task to
  remain `working` and to reach a terminal status other than `cancelled`. Costs
  nothing and fabricates nothing. **Recommended.** The price is that
  `tasks/cancel` no longer forces a `cancelled` terminal state; it never honestly
  could, because the work it names is still running.
- **(b) Cancel the underlying workflow.** Coherent — under this design the task
  *is* the run — but it turns a verb that reads as "stop waiting" into a
  destructive one, and duplicates the explicit `cancel` tool, which carries the
  destructive annotation for a reason.
- **(c) Refuse, pointing at the `cancel` tool.** Most honest, and likely
  non-conformant: the extension obliges the server to acknowledge.

My recommendation is **(a)**, with the response message naming the `cancel` tool
as the way to stop the run.

### RULED (Waffles, 2026-08-19T07:31Z): option (a).

With one design note attached, which he required settled **before** building:
find where the extension expects a cancel-requested state to be VISIBLE on
subsequent `tasks/get` reads. If the acknowledgement alone satisfies the
contract, stay stateless; if the requested-state must persist across reads, its
honest home is an event in the run's own history — never a resurrected side-map,
"which would be the `TaskStore` crawling back in through the cancel door".

**Settled: stay stateless.** Read from the pinned contract itself
(`modelcontextprotocol/ext-tasks` @ `2c1425d9…`, `specification/draft/tasks.md`)
rather than from this repo's paraphrase of it — the code's own comment asserts
the same thing, but a relayed fact wears the relayer's name.

> "Cancellation processing is *eventually consistent* — the task's observable
> status **MAY** remain `working` (or some other non-terminal status) after the
> ack, and **MAY** ultimately reach a terminal status other than `cancelled` if
> the work finished before cancellation could take effect."

> "Cancellation is **cooperative**: The request signals intent, and the server
> decides whether and when to honor it."

And the status set is closed at five — `working`, `input_required`, `completed`,
`failed`, `cancelled`. **There is no `cancel_requested` or `cancelling` status**,
so the extension has no place to put a visible requested-state even if a server
wanted one. `TaskStatus` in `model.rs` already enumerates exactly those five, so
nothing is missing there either.

The contract therefore requires only the acknowledgement, and explicitly leaves
"whether and when to honor it" to the server. This server's answer is *never*,
for a reason already written into the runner it replaces: a durable run is not
something an MCP client's disinterest is allowed to kill. **No state is stored,
and no event is written to the run's history** — the second branch of the design
note does not apply.

The acknowledgement's message must say plainly that the run continues and name
the `cancel` tool, so that a caller is never left believing it stopped something.

## Scope, stated so it is not quietly widened

`start_run` is the only task-shaped tool, so **every** task today is run-backed
and this design covers all of them. If a future tool needs a task with no
workflow behind it, that is when a durable task record gets designed, and its
honest home is aion's own store rather than a second store in the protocol
layer. **Not built now** — there is no such tool, and building for one would be
the speculative generality this codebase forbids.

`pollIntervalMs` stays configurable. It is a hint to the client and bounds
nothing; no server behaviour depends on its value.

## Status

**BUILT 2026-08-19** — see `gate-logs/mcp-task-durability-BUILT.md` for the
record, the gates, and the two witnessed proofs (restart survival, and an
unauthorized read refused on a VALID handle, watched go red under a mutation).
Not deployed: a rebuilt binary rides the release line with a battery.

Before the build, this document was design only. It was verified then against
the running binary
(`gate-logs/mcp-awaited-start-verified.md`): 13 tools live, `start_run` carries
`await_completion`, and an awaited start returns a task that `tasks/get` reports
`completed` with the run's terminal status.
