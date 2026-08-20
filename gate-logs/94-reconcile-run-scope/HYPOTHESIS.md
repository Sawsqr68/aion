# aion#94, next member of the class: `Registry::reconcile` takes a run and does not use it

**Vesper, 2026-08-20. NOT PROVEN — this is a source-chain hypothesis with a
named falsifier.** It was found while measuring the structural cure for #94 (can
the handle be reconciled from the terminal already in hand, with nothing fallible
on the release path?) and it is a different, larger defect than the window that
question was about.

## The shape

```rust
pub fn reconcile(&self, id: &WorkflowId, run: &RunId, events: &[Event])
    -> Result<Option<WorkflowHandle>, EngineError>
{
    let projected = status_from_events(events);      // <-- NOT scoped to `run`
    let mut slots = self.slots()?;
    let Some(handle) = slots.handles.get_mut(&(id.clone(), run.clone())) else { … };
    handle.replace_projected_status(projected);
```

`run` selects the handle to WRITE. It does not scope the projection that is
written. `status_from_events` is last-lifecycle-event-wins over whatever slice it
is given, and every production caller gives it **the whole history for the
workflow id** — which, after a continue-as-new, contains more than one run.

So the signature reads as run-scoped and the behaviour is not. Every call site is
correct only under an invariant that is stated nowhere: *the handle being
reconciled belongs to the workflow's latest run.*

🔴 The fix I landed for #94 itself used `status_from_events(run_segment(&history,
handle.run_id()))` — I run-scoped by hand at that one site because the projection
genuinely needed it there. The same need was one call away and I did not look.
**A remedy adopted from an incident is fitted to that incident's mechanism.**

## Where the unstated invariant fails

Two continue-as-new paths, and they differ:

| path | predecessor handle | window |
|---|---|---|
| **API-driven** (`lifecycle/continue_as_new.rs`) | REMOVED at `:150` | narrow — removal happens AFTER `start_workflow_with_options` (`:111-143`) appends the successor's `WorkflowStarted`, so between those points the predecessor handle is registered while the history already ends in the successor's events. No `.await` between them, so another thread must observe it. |
| **nif-driven** (workflow code calls `continue_as_new`) | 🔴 **NOT REMOVED** | unbounded |

The nif path records the terminal (`nif_continue_as_new::record_continuation`),
the process exits, `completion.rs` reconciles (correct at that instant — the
successor has not started yet), and then `start_continuation_replacement` starts
the successor at `completion.rs:506`. Nothing in that path removes the
predecessor's handle. It stays registered, `ContinuedAsNew`, residency
`Suspended`, for the life of the engine. Grepped: the only `registry.remove`
callers are reopen, deadline, terminate ×3, start's error path, and the
API-driven continue-as-new. `completion.rs:1277` is inside a test fixture. There
is no eviction or sweep of terminal handles in `registry/`.

## The trigger is a READ-ONLY API

`Engine::list_workflows` (`engine/api_workflow_ops.rs:558-564`) iterates
`registry.list()` — which returns **every** handle — reads the full history for
each, and reconciles it:

```rust
for handle in self.registry.list()? {
    let history = self.store.read_history(handle.workflow_id()).await?;
    self.registry.reconcile(handle.workflow_id(), handle.run_id(), &history)?;
```

So a client merely LISTING workflows writes the successor run's status into the
predecessor run's handle. A terminal run's cached status becomes `Running`, and
every subsequent list rewrites it.

## Consequence, read against the #94 census

- **`admission.rs:180`** counts a version live from `!cached_status().is_terminal()`.
  The audit called this "over-demands, **transiently** — the window closes on
  reconcile". If this hypothesis holds, for a nif-continued run it does not close:
  a superseded version is demanded of every worker on that queue for the life of
  the engine. **The audit's own severity judgement for that site was written
  against the wrong duration.**
- **Wire sites** (`embedded.rs:171`, `workflows.rs:415`) report the predecessor
  run as `Running` — not for a window, but until the engine restarts.
- **`reload.rs`** is immune, because the #94 fix reads history and run-segments.

## The cure — subtraction, and it is the same shape as the #94 fix

Scope the projection inside `reconcile`, so no caller has to know:

```rust
let projected = status_from_events(run_segment(events, run));
```

**Why this is safe where the current code is already correct:** `run_segment`
slices from that run's `WorkflowStarted` to just before the next one, or to the
end. For the workflow's LATEST run the segment runs to the end of history, and
`status_from_events` scans backwards to the last lifecycle event — which lies
inside that segment. Identical answer. It differs only in the cases the current
code gets wrong.

It also makes the signature honest: a function taking a `RunId` should mean it.

## 🔴 NOT PROVEN, and what would falsify it

This is a source chain, not a measurement. The claim needs a **red-first** test:
drive a nif-driven continue-as-new, let the successor start, call
`list_workflows`, and assert the predecessor handle's `cached_status()` is
`ContinuedAsNew`. Expected to FAIL against today's code with `Running`, and to
pass with the one-line run-scoping.

It is falsified if any of these turns out otherwise:

1. The predecessor handle IS removed on the nif path by something I did not find
   (then the defect collapses to the narrow API-path window).
2. `registry.list()` excludes suspended handles (it does not, by reading — but
   reading is not running).
3. `run_segment` on the latest run does not give the identical answer for some
   history shape I have not considered — reopen, in particular, deserves a
   fixture: `WorkflowReopened` is a lifecycle event that does NOT open a new
   segment, so a reopened run's segment boundaries are worth asserting rather
   than assuming.

**Held until the box is quiet** — a compile is needed and the machine is under
another lane's build (load 24-41 with up to 20 `rustc`). Not posted to #94 until
measured; the correction it implies to the audit's severity is exactly the kind
of claim that must not travel ahead of its evidence.
