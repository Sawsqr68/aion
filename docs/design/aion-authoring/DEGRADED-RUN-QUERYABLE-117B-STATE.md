# #117 (b) — a run that will never be resident must be able to SAY so

**Status: engine half LANDED. The server/CLI surfacing is a separate
increment needing its own block declaration.**

## Where the lane actually stands (measured 2026-07-31 ~14:26Z)

#117 lists three pieces. Two are already resolved; only (b) is open.

**(a) the misattributing message — ALREADY DONE, `6b9252ad`.** The task text
cites `loader/persistence.rs:116` saying *"store corruption or manual
tampering"*. That string no longer exists anywhere in `crates/`. The current
message states the DISJUNCTION (superseded package-identity domain **or**
corrupted/tampered bytes), asserts no cause, and gives the remedy (`redeploy`).
The doc comment above it (lines 90-99) explains why it must not assert a cause.
Checked before building anything — the fix was already landed by a prior lane.

**(c) the cancellation semantics — still needs a ruling.** Unchanged. Minting a
Recorder directly against the store for a single cancellation append is written
up in the #117 doc and deliberately NOT built.

**(b) queryable degraded state — OPEN. This lane.**

## The site, found by following the call graph rather than the task text

`durability::recovery::recover()` / `RecoveryReport` are a red herring for this
purpose: they are exported from `durability/mod.rs` and **called nowhere**. The
engine's real startup path is
`engine/startup.rs::repopulate_active_workflows`, and the skip is:

```rust
// crates/aion/src/engine/startup.rs:224-232
Err(error) => {
    tracing::error!(
        workflow_id = %workflow_id,
        workflow_type = %workflow_type,
        error = %error,
        "active workflow failed startup recovery; skipping it and continuing"
    );
    continue;
}
```

The error is formatted into a log line and dropped. The typed error is not
retained, so nothing can be asked about it afterwards. That is the whole of (b):
**the fact exists exactly once, in a stream nobody can query.**

## Why this is the same defect class as #119

The boot log ERROR is an announcement. Nothing compares it, nothing can be asked
about it later, and the operator who needs it most is the one who wasn't
watching stdout at boot. Observability that nothing can interrogate is not a
control. #119 closed that for build posture; this closes it for run residency.

## The shape (measured constraint, not a preference)

The read plane and the control plane already disagree, and this is the
reconciliation point:

```
describe  -> rc=0, full history, Running     (the read plane FOUND it)
resume    -> rc=1 "is Running, not Paused"   (also found it, read its status)
cancel    -> WorkflowNotFound                (control plane: does not exist)
pause     -> not found
```

`describe` is where the operator already goes and it already answers. So the
degraded fact belongs ON the describe answer, not behind a new endpoint the
operator has no reason to call. A run that is `Running` per its projected status
but can never obtain a writer must say that in the one place someone is already
looking.

## Build order

1. Engine: retain the skip — `workflow_id`, `workflow_type`, and the typed
   reason — in a structure owned by the engine, populated at `startup.rs:224`.
2. Engine: accessor to ask whether a given run is degraded and why.
3. Surface it on `describe`.
4. Tests: a run whose pinned version cannot load is (i) skipped, (ii) retained,
   (iii) reported by describe. Plus the inverted control — a healthy run must
   NOT be reported degraded.

## Standing constraints for this lane

- Invariant #3 holds: this lane adds NO writer. It only retains and reports a
  fact the engine already computed. The cancellation append stays unbuilt
  pending the ruling.
- Status is a projection (invariant #4). Degraded residency is NOT a status and
  must not become one — residency is orthogonal, exactly as `Resident` /
  `Suspended` already are.

## What landed

`crates/aion/src/registry/unrecoverable.rs` — `UnrecoverableRuns`, a per-process
set owned by `Registry` (residency is what the registry is about; this is its
negative half). Third independent mutex, never held with `handles` or `index`,
so it sits outside the existing ordering rather than extending it.

Wired at all three points in `engine/startup.rs`:
- the recovery-failure arm **records** the typed reason;
- the Ok arm **clears** it;
- the already-resident guard **clears** it too, which is what covers the SS-5
  adoption re-run — that path reaches workflows by a route this loop never took.

## The seam test is the one that matters

The set's own unit tests drive it directly and would stay green if
`repopulate_active_workflows` never called it — which is precisely how #58
shipped an inert feature behind eight passing tests. So
`a_run_whose_pinned_version_cannot_load_is_retained_as_unrecoverable` runs the
REAL startup sweep and asks the registry afterwards.

It is a **baseline comparison, not an enumeration**: one run whose pinned version
is catalogued and one whose is not, in the same sweep, so the assertion is about
the difference. A `record` that fired for every run, or for none, fails one half
or the other.

**The baseline half caught a real defect on first run** — my "healthy" run was
pinned to `"a" * 64` while the catalogue held `ContentHash::from_bytes([3; 32])`,
so it was degraded too and there was no baseline. The assertion earned its place
before the feature did.

**Inverted control, watched:** with `record(...)` removed from the skip site the
test fails with its own reason string — *"the run whose pinned version cannot
load must be retained as unrecoverable"*. Wiring restored, green re-verified.

## Gates

- `cargo clippy -p aion-rs --all-targets -- -D warnings` — clean
- `cargo test -p aion-rs --all-targets --no-fail-fast` — 1021 passed / 0 failed

## Still open on #117

- **(c) the cancellation semantics** — a non-resident run still cannot be
  cancelled. Minting a Recorder directly against the store for the single
  cancellation append is written up and needs a ruling. This lane adds no writer.
- **The operator-facing surface** — `describe` is CLI-side, built from a server
  summary, so reporting the degraded fact there crosses into `aion-server` and
  is its own increment.
