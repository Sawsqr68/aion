# `workflows/` — our own work, run on Aion

This directory is not examples. Everything here is a workflow we actually run to
get real work done, on the same engine we ship.

The distinction matters because the two kinds of document rot differently. An
example is judged on whether it teaches; nobody notices when it stops being
true. A workflow that does real work is judged on whether it gives the right
answer, and it gets caught the first time it does not — which is the only kind
of documentation that stays honest.

`examples/` is for teaching. This is for working.

## Why put the work here at all

Two reasons, and the second is the one that pays.

**It is the demonstration.** Aion's claim is that a durable workflow beats a
script. Running our own work on it is the only version of that claim we cannot
fool ourselves about: a real answer we depend on, produced by the engine,
recorded in history, replayable.

**Machinery remembers what people forget.** Every workflow in here encodes a
discipline that a person under time pressure drops. `issue-recheck` is the clean
case: it will not report a finding until a control search has proved it can find
anything at all, because a search pointed at the wrong directory returns zero
hits and zero hits is also what a genuinely absent symbol returns. Knowing that
does not help. Knowing it *and being unable to skip it* helps.

That is the bar for anything added here. A workflow that only automates the
convenient half of a job is worse than the manual version, because it wears the
authority of a machine.

## What is here

| Workflow | What it answers |
|---|---|
| [`issue-recheck/`](issue-recheck/) | Is this reported-missing thing still missing? — with a control that refuses to answer on a broken search. |
| [`norn-fleet/`](norn-fleet/) | What does a reader make of each of these issues? — norn-backed judges fanned out in parallel, behind a control that refuses to judge on a pipeline that cannot prove it reads the tree. |
| [`norn-dev/`](norn-dev/) | Did this delegated build actually pass its gates? — one brief to a norn-backed builder, admitted by the server running the gate script, never by the builder's own account. |

## Running one

Each directory carries its own README with the inputs it takes. The shape is
always the same:

```sh
aion awl check <dir>/<file>.awl                  # local, no server
aion run <dir>/<file>.awl --input '<json>'       # compile, deploy, start, await
```

`aion run` validates the input against the document's derived start contract
before it opens a connection, so a wrong input fails at your desk.

## Rules for adding one

1. **State what it measures, and what that does not prove.** In the `//!` header,
   not only in the README. The document travels; the README does not.
2. **Carry a control.** If the workflow can produce a comfortable answer by
   failing, it must prove the method works before it reports anything.
3. **Ship the evidence with the answer.** The control's own result belongs in the
   outcome payload. An assertion that the method worked is not a demonstration
   that it did.
4. **Prove every arm before you commit it.** Including the refusal arm — an
   unreachable refusal is decoration. The strong form is a *positive control on
   the control*: drive the refusal by feeding a deliberately wrong expectation
   while the method returns the **true** answer, so the refusal demonstrably
   fires because the expectation failed and not because the method broke — and
   between that run and one green run, both arms of the comparison have been
   driven live. (First used landing `norn-fleet`; the refusal-run evidence is
   in `gate-logs/77-norn-fleet/`.)
