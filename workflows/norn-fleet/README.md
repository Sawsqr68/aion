# norn-fleet

Fan a list of open issues out over norn-backed judges, in parallel, on the
engine's own durability — behind a control that refuses the whole run when the
judging pipeline cannot be shown to work. One input, one report back.

This is the fleet-judgement pattern: the workflow owns the fan-out, the
retries, and the collection; a norn worker owns the judgement. Mechanical
checks stay in ordinary workflows (see `workflows/issue-recheck/`) — this one
is for the work that needs a reader.

## The pieces

| Piece | Who provides it |
|---|---|
| `norn_fleet.awl` | this folder — the workflow document |
| The judge | `aion worker agent` with the norn harness (installed binary, no crate, no rebuild) |
| The issue list and the control | **the dispatcher** — whoever starts the run builds the input |

## The dispatcher contract

The workflow takes one input:

```json
{
  "issues": [{"id": "...", "title": "...", "prompt": "..."}, ...],
  "control_prompt": "...",
  "control_expect": "..."
}
```

Every `prompt` must be **complete and self-contained**. The judge sees nothing
but that one string — no issue tracker, no conversation, no shared context.
The claim being judged, the file paths to start from, the standard to judge
against, and the reply format all have to be inside the prompt. Building that
string is deliberately the dispatcher's job: it keeps the judge action on the
plain agent seam (one String prompt in, the canonical agent-outcome record
out — `{text, final_message, stop_reason, session_id}`, the whole output, last completed answer,
why the run stopped, and the handle of the session that produced it), which is the
one shape a norn worker started from the installed binary can serve honestly.

### The control

`control_prompt` is a question the dispatcher already knows the answer to;
`control_expect` is that answer, exactly. The control judgement runs **before
any issue is judged**, and if the reply is not byte-identical to
`control_expect` the run routes to the `unmeasured` refusal outcome and
reports no verdicts at all. A judge that cannot read the tree — wrong
directory, broken file tools, dead session — returns confident well-formatted
verdicts that are indistinguishable from real ones; only a known answer can
tell the difference. A good control asks the judge to quote a literal that
exists at a known path, which proves file access and exact
instruction-following in one question, e.g.:

> Open the file CLAUDE.md at the root of your working directory. Its first
> line is a heading. Reply with exactly that heading's text after the `# `,
> and nothing else.

with `control_expect: "CLAUDE.md"`. The control's reply travels in the
success payload too (`FleetReport.control_reply`), so the answer carries its
own evidence.

### What a report proves — and does not

A verdict is one model's unverified opinion. The control proves the pipeline
worked; it proves nothing about any judgement's correctness. **Every citation
in a verdict must be read by hand before anything acts on it.**

### Positional pairing

`verdicts` pairs with `issue_ids` by position — `verdicts[i]` answers the
issue named by `issue_ids[i]`, in input order. This is guaranteed by the
lowering, not observed from runs: `distribute` lowers to `workflow.map`,
which is documented and gated as exactly-once per item in input order
(`crates/aion-awl/src/emitter/forks.rs`,
`gleam/aion_flow/src/aion/workflow/concurrency.gleam`).

### Strict collection, on purpose

The collect is strict because positional pairing dies the moment an element
can be absent — a report with holes cannot say which verdict answers which
issue. (The checker independently refuses element-optional lists in declared
types; its stated reason is mechanical — schema derivation cannot express
element optionality — the pairing argument is this document's own.) If one
judgement still fails after the declared retries, the whole run fails. See
"When a fleet fails" below for what that actually looks like.

### Retries and timeouts

The judge action declares `retry 5 backoff 30s..5m` — this is the real
defence against transient provider overload. The tharsis postmortem
(`docs/design/aion-authoring/THARSIS-RUN-1-POSTMORTEM.md`) recorded overload
bursts that punched through in under a minute; five retries on that backoff
outlast a multi-minute outage. An absent retry policy would mean exactly one
attempt per judgement — the engine bakes in no default.

The **workflow-level `timeout 2h`** is the enforced bound on the whole run.
The action-level `timeout 15m` is an authored contract carried in the package
hash and shown on the deployed surface, but the engine does not consume it
today (see `docs/workers/activities-and-workers.md`, "carried, not
consumed") — do not rely on it to kill a wedged judgement; the workflow
timeout is what actually fires.

## When a fleet fails

Honesty about the failure face, as of today:

- A failed run surfaces through `aion run` as
  `workflow run failed: workflow <n> returned error`, where `<n>` is an
  internal process id, not a workflow id (open defect #151 — the fleet's own
  first flight judged it CONFIRMED). Use the server console or `aion list`
  to find the run.
- One exhausted judgement **cancels the siblings still in flight** — the
  fan-out is cancelled as a unit, so completed-but-uncollected judgements
  are discarded with it.
- A control refusal is a *routed outcome*, not a crash: the run completes on
  the `unmeasured` route carrying what was expected and what came back.

## Running a fleet

First, the document has to say how the judge is launched. `norn_fleet.awl` as
tracked carries **no `harness` section**, and `aion worker agent` refuses a
document without one — the launch is not a flag and never falls back to a
default. Add it to the `worker fleet` block, above the `judge` action:

```awl
worker fleet
  harness
    kind norn
    concurrency 4
    reconnect_initial_backoff 1s
    reconnect_max_backoff 30s
    reconnect_max_attempts 100
    env_pass "PATH", "HOME"
    args "--workspace-root", "/srv/fleet/{workflow_id}", "-C", "/srv/fleet/{workflow_id}", "--account", "default"
    cwd "/srv/fleet"
```

`{workflow_id}` and `{activity_type}` are expanded per dispatch by the Norn
adapter; everything else is literal. `env_pass` is the judge's whole
environment — a variable not named there is absent in the spawned norn, whatever
the shell that started the worker exported. `aion awl check` names any setting
left out.

```sh
# 1. Validate locally (no server)
aion awl check workflows/norn-fleet/norn_fleet.awl

# 2. Start the judge worker (from the repository root — see the warning below)
aion worker agent workflows/norn-fleet/norn_fleet.awl \
    --liminal-address 127.0.0.1:50061 \
    --identity fleet-judge-worker

# 3. Confirm the queue is servable
curl -s -X POST http://127.0.0.1:8080/awl/workers/availability \
    -H 'Content-Type: application/json' \
    -d '{"namespace":"default","task_queue":"fleet"}'

# 4. Compile, deploy, start, await — one motion
aion run workflows/norn-fleet/norn_fleet.awl --input "$(cat issues.json)"
```

### ⚠️ The judges are not confined

There is **no sandbox and no file boundary** on this path. The worker's
working directory is only what the judges' relative paths resolve against —
nothing confines them to it. Confinement comes from norn's own
`--workspace-root`, which is an entry in the `harness` section's `args` and
therefore something you have to write: a section without it launches judges
with the operator's full filesystem access, restrained only by their prompts.
Write it, start the worker from the repository root so relative paths resolve
sensibly, write prompts that do not ask for writes, and treat every fleet as
running unattended agents under your own account — because it is.

### Concurrency is the fan-out width

The document's `concurrency` is required by the worker builder but is **inert
for agent dispatches on this transport** — every judgement dispatched to the
worker starts its own norn process immediately. A fleet of N issues is N
simultaneous norn sessions (plus the control, which runs alone before them).
Size the input list to what the box and the provider can take; there is no
throttle between the list and the processes.

## The trade this shape records

The judge returns the canonical agent-outcome record — `text: String`,
`stop_reason: String`, and `session_id: String` (tasks #181 and #205; the
record superseded #98's bare-`String` rule and is enforced by the checker,
which refuses any other agent return shape). The record carries the reply and why the run stopped; it does NOT
carry a structured verdict type. A structured schema (verdict enum, evidence
list, proposal field) would be better data — but serving it requires a
custom harness binary that parses and validates model output into the
declared type, and the record's `text` is deliberately a plain String. So
structure still lives in the *prompt* (ask for a fixed reply format) and
the dispatcher parses the record's `text` downstream. When a custom harness
lands, the upgrade path is unchanged in kind: widen the action's return
type and let the checker force the document and worker to move together.

One further operational note: a `harness` section whose `args` list is empty or
absent runs judges on bare norn defaults — default account, fresh session per
judgement, no session pinning, no workspace confinement. Everything the judge
gets, it gets because the document said so. And the standing hold on *long*
fleet runs is the beamr
JIT `NativeServices` defect (#82) — a compiled frame re-entering the
interpreter loses its services mid-run, which is what killed the 55-minute
tharsis fleet. Keep fleets short until the fixed beamr lands.
