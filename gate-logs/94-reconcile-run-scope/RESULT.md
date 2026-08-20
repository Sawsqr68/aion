# aion#94 — `Registry::reconcile` projected the wrong run. PROVEN, FIXED, GATED.

**Vesper, 2026-08-20.** Supersedes the hypothesis in `HYPOTHESIS.md` (kept: it
records what was believed before it was measured, and the falsifiers it named).

## The defect

`reconcile(id, run, events)` used `run` only to select the handle to WRITE. The
projection was `status_from_events(events)` over the caller's whole slice, and
every production caller passes the entire history for the workflow id — which,
after a continue-as-new, holds more than one run. `status_from_events` is
last-lifecycle-event-wins, so the predecessor run's handle was cached with the
SUCCESSOR's status.

A function taking a `RunId` and not scoping by it left every call site correct
only under an invariant stated nowhere: *the handle is the workflow's latest run.*

## RED FIRST — the pin, with its invocation

```
cargo nextest run -p aion-rs --lib -E 'test(reconcile_)' --no-fail-fast     RC=100

thread 'registry::table::tests::reconcile_scopes_the_projection_to_the_named_run'
panicked at crates/aion/src/registry/table.rs:989:9:
assertion `left == right` failed: the predecessor run is terminal; only the successor is running
  left: Some(Running)
 right: Some(ContinuedAsNew)
```

The fixture carries its own control — it asserts that the whole-history
projection really is `Running` before asserting the scoped one is not — so a
failure is the scoping defect and not a malformed history. And it has a twin,
`reconcile_still_sees_the_successor_as_running`, because without it the first
assertion is satisfied by anything that answers `ContinuedAsNew` unconditionally.

## The fix

```rust
let segment = run_segment(events, run);
if segment.is_empty() { return Err(EngineError::RunNotInHistory { … }); }
handle.replace_projected_status(status_from_events(segment));
```

For a workflow's LATEST run this changes no answer: its segment runs to the end
of history, which is where the backwards scan starts. It differs only where the
old code was wrong.

### The empty segment is an ERROR, not a default — and that is the point

`status_from_events(&[])` returns `Running`. So a run absent from the history it
is projected against would have been silently cached as RUNNING — **the
reconciliation whose whole purpose is to stop terminal runs being cached as live
would itself have cached one as live.** The banned fallback was one line away
from the fix for the bug it causes.

New typed variant `EngineError::RunNotInHistory { workflow_id, run_id }`. Not
reachable through a normal start (`WorkflowStarted` is recorded at
`start.rs:171`, well before the handle is published at `publish_started_handle`),
so it means a genuine invariant breach and is surfaced rather than absorbed.
Mapped at both `aion-server` wire sites to `backend` — the class this table
already gives every engine-internal refusal — because nothing about the caller's
request is wrong.

## 🔴 Two more fixtures that modelled a history which cannot exist

`registry/table.rs` and `lifecycle/transition.rs` each had a `started()` helper
stamping a hardcoded `run_id` of `Uuid::from_u128(1)`, regardless of the run
being reconciled. Every existing `reconcile` test therefore asserted against a
history belonging to no registered run — and all of them passed, **because the
projection ignored which run it was projecting.**

A test that cannot tell the two apart cannot detect the confusion between them.
Both helpers now take the run as a parameter; the run-blind one in `table.rs` was
deleted rather than kept alongside its replacement.

## Gates

| gate | result |
|---|---|
| `cargo nextest run -p aion-rs --lib --no-fail-fast` | **709/709 passed** |
| `cargo nextest run -p aion-rs --no-fail-fast` (all targets) | **1198/1198 passed** (1 slow) |
| `cargo clippy --workspace --all-targets -- -D warnings` | **clean** |
| `cargo fmt --all` | applied |

### The denominator, reconciled rather than assumed

An earlier log in this session recorded **1235** tests for `aion-rs`. That is a
different tree — worktree `scratchpad/wt-25-main`, branch
`fix/94-unload-pin-history-authority` at `624c16304`, 145 commits apart — so it
is not a comparable baseline. The check that does apply is the diff: it adds
exactly **2** `#[test]` functions and removes none, so 1196 → 1198 is accounted
for.

⚠️ That count needed `git diff --no-ext-diff`. This repository configures an
external diff driver, so `git diff | grep '^+'` matches nothing and reports a
clean-looking zero — a tool failing into a comfortable number. (Also cost a pass:
BSD `grep -E` does not support `\s`; use `[[:space:]]`.)

## 🔴 WHAT IS NOT PROVEN — reachability

The unit pin proves the MECHANISM: `reconcile` projected the wrong run. It does
**not** prove the production path end to end. That argument is still from source:

- `Engine::list_workflows` (`engine/api_workflow_ops.rs:558-564`) iterates
  `registry.list()` — every handle — and reconciles each against full history.
- The nif-driven continue-as-new path never removes the predecessor's handle.
  (The API-driven path does, at `continue_as_new.rs:150` — but only AFTER
  `start_workflow_with_options` has appended the successor's `WorkflowStarted`.)
- Grepped: `registry.remove` callers are reopen, deadline, terminate ×3, start's
  error path, and API-driven continue-as-new. `completion.rs:1277` is a test
  fixture. There is no eviction sweep of terminal handles.

**Owed:** an engine-level test that drives a nif-driven continue-as-new, calls
`list_workflows`, and asserts the predecessor handle's cached status is still
`ContinuedAsNew`. Until it exists, the severity claim below is reasoned, not
measured, and must be reported that way.

## The correction this implies to my own #94 audit

`AUDIT-cached-status-readers.md` recorded `admission.rs:180` as *"over-demands,
**transiently** — the window closes on reconcile"*. If the reachability argument
holds, for a nif-continued run it did not close: the predecessor's version stayed
counted live and demanded of every worker on that queue for the life of the
engine, rewritten by every `list_workflows` call.

**A remedy adopted from an incident is fitted to that incident's mechanism.** My
#94 fix in `reload.rs` run-segmented by hand at that one site because the
projection needed it there. The same need was one call away, in the function that
site's own comment points at, and I did not look.
