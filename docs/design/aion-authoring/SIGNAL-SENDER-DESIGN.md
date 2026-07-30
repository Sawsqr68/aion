# AWL signals: the sender

Written 2026-07-30 for task #24, after Tom named signals "a pretty important
part of it" for the fleet/agent work. Everything below is code-true as of main at
this date; each claim names where it was checked.

## The correction that changes the cost

Signals are NOT missing from Aion. They are missing from ONE surface.

| layer | receive | send |
|---|---|---|
| engine (`crates/aion`) | `Command::AwaitSignal` | **`Command::SendSignal` — exists** |
| Gleam SDK (`gleam/aion_flow`) | `signal.receive` | **`signal.send` — exists** |
| CLI / outside | `aion signal <id> <name> --payload` | n/a |
| **AWL** | `signal`/`wait` — exists | **ABSENT** |

`Command::SendSignal { key: CorrelationKey, delivery: SignalDelivery }` is a
first-class durable command (`crates/aion/src/durability/command.rs:26`). It
carries a correlation key, so it is replay-correct by construction: on
re-execution the recorded occurrence resolves and the signal is NOT re-sent.
Live execution is owned by the NIF signal bridge
(`crates/aion/src/runtime/nif_signal.rs:400`), which is why the generic
durability executor deliberately refuses it with "send-signal live execution is
owned by the NIF signal bridge" (`durability/executor.rs:234`) — that is a
routing statement, not an unimplemented stub.

The Gleam surface is `signal.send(workflow_id: String, reference:
SignalRef(payload), payload) -> Result(Nil, EngineError)`
(`gleam/aion_flow/src/aion/signal.gleam:61`): addressed by workflow id,
fire-and-forget, errors returned as typed data.

**So this is a language-surface task over a finished, already-durable mechanism —
not new engine work.** That is a materially smaller job than "build signals".

## What AWL has today

Verified in `crates/aion-awl/src/lexer/tokens.rs`: keywords `signal` (62) and
`wait` (96) exist. There is NO `send` and NO `emit` keyword — the token set has
neither, so nothing downstream could parse one.

Receiving works and is complete:

```awl
workflow approval_gate
  signal decision: Decision

step await_operator
  wait decision timeout 4h -> answer
  outcome approved: when answer is some and answer.approved, route ship
  outcome timed_out: when answer is none, route escalate
```

With a timeout the binding is `T?`, so absence is branchable rather than fatal.
An operator approval gate is therefore FULLY expressible today, delivered from
outside by `aion signal <workflow_id> decision --payload '{"approved":true}'`.

## The real gap is addressing, not the verb

Adding a `send` statement is the small half. The blocking question is *what an
AWL author can name as a target*, and today the answer is almost nothing:

- `workflow.id` is the ONLY builtin in the namespace — the checker says so in its
  own diagnostic: "unknown workflow builtin `workflow.{name}` — only
  `workflow.id` is available" (`crates/aion-awl/src/checker/exprs.rs:187`). So a
  workflow can name ITSELF and nothing else.
- `spawn name(args…)` exists (`ast/steps.rs:194`), but it yields NO handle. A
  binding after it PARSES and is then a deliberate check error — the AST field is
  documented as "A parsed-but-illegal binding, kept for checker diagnostics".
  So a parent cannot name the child it just spawned.
- There is no peer registry, no `children` collection, no id-returning call.

One useful pattern already falls out of `workflow.id` alone, and it is the one
the fleet case actually needs first: **a parent passes `workflow.id` down as an
input field, and each child sends back to it.** Child-to-parent needs no new
addressing at all. Parent-to-child does.

## Recommended shape

**Phase 1 — `send` to an id-valued expression. Ship this first.**

```awl
send progress to coordinator_id (item: item.id, state: "gated")
```

- `send <signal_name> to <expr> (field: value, …)`
- `<expr>` must type as `String` (a workflow id). `workflow.id` qualifies, and so
  does any input or bound field carrying one — which makes the child→parent
  pattern immediately expressible.
- The payload arguments are checked against a DECLARED signal type, exactly as an
  action call's arguments are checked against its declaration. That requires the
  sender to declare the shape it is sending; see the checker rules below.

**Phase 2 — a spawn binding, so a parent can address its children.** Turn the
currently-illegal `spawn child(...) -> handle` into a legal binding whose type is
a child handle carrying the child's workflow type. Then
`send cancel to handle (reason: "…")` is STATICALLY CHECKABLE: the compiler knows
the target's workflow type, so it can verify the target declares that signal with
a compatible payload type. This is the form that makes signals safe rather than
merely available, and it is why phase 1 should not be over-invested in.

Phase 2 is a strictly larger change (it interacts with #26, cancel not cascading
to implicit children, and with the `distribute` child model from #25), so it
should not gate phase 1.

## Determinism

Nothing new is required, and nothing may be invented. A send is a side effect, so
it must be a recorded command whose recorded outcome is returned on replay — which
is precisely what `Command::SendSignal`'s correlation key already provides. The
AWL lowering must therefore emit the existing command and must NOT reimplement
delivery. Any design that sends outside a recorded command re-sends on every
replay and is wrong.

## Failure semantics — decide, do not default

`signal.send` returns `Result(Nil, EngineError)`. AWL must say what happens to
that error, and there are only two honest answers:

1. **A failed send faults the step**, like a failing action call, and is
   catchable by the step's existing `on failure` clause. Consistent with the rest
   of the language; recommended.
2. A failed send binds a branchable result. Cheap to describe, but it invents a
   second error-handling idiom for one statement, which is worse than it sounds.

Take (1). It reuses machinery authors already know and keeps one error idiom.

## The hazard that needs a checker rule

The engine routes a signal BY NAME. A send whose name no receiver ever waits for
is not an error anywhere — the payload simply lands in history and nothing
consumes it. That is a silent no-op, which is exactly the class of failure this
codebase refuses elsewhere. Mitigations, in order of strength:

- **Phase 2 (typed handle): refuse at compile time.** The target's workflow type
  is known, so a signal it does not declare is a check error naming both sides.
  This is the real fix.
- **Phase 1 (id expression): require the sender to DECLARE what it sends**, e.g.
  a header entry that names the signal and its payload type. Then the payload is
  type-checked even though the target is dynamic, and the document states its
  outbound surface instead of leaving it implicit at call sites. The target's
  agreement remains unverifiable — that limit must be written on the feature's
  face, not discovered.
- Runtime observability: a delivered-but-never-awaited signal is worth surfacing
  on the run rather than leaving invisible. Worth its own task; not a substitute
  for the checker rule.

## Open questions for the ruling

1. Phase 1 only, or phase 1 + phase 2 together? (Phase 2 is where the type safety
   is, but it touches the child model and #26.)
2. Does a phase-1 `send` require a declared outbound signal in the header? The
   recommendation is yes — an undeclared send is an untyped hole in a typed
   language.
3. Is `send` fire-and-forget (matching `signal.send`'s `Nil`), or should AWL ever
   await acknowledgement? Recommendation: fire-and-forget, because acknowledgement
   is a different feature (a request/response pair) and conflating them would put
   an unbounded wait behind an innocuous-looking verb.
