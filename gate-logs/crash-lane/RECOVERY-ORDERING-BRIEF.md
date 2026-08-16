# Recovery-ordering fix — lane brief (#266 Defect A)

Branch `recovery-ordering` off origin/main de3b3c306. Waffles' fix-shape
approval 17:53Z, six terms. Root cause record:
`gate-logs/crash-lane/ORPHANED-RUN-SHAPING.md` (§ROOT CAUSE FOUND).

## The defect (one sentence)

Recovery replay runs inside `EngineBuilder::build()` and re-dispatches
in-flight activities before `install_engine_backed_seams` fills the
`DeclaredBodySource` OnceLock (state.rs:276 vs :287), so every adopted
declared-body dispatch silently (declared_body.rs:126 `map_or(None…)`)
falls to the worker path and parks forever on queue `local`
(`NO_LIVE_POLLERS`, deadline `never`).

## The fix

**Engine (`crates/aion`, builder.rs):** new builder mode
`defer_startup_recovery()`. When set, `build()` skips its four recovery
steps — `recover_active_workflows_on_startup`,
`recover_timers_on_startup`, `catchup_schedule_coordinator`,
`recover_schedules_on_startup` — and stows what they need that the
Engine does not already hold (the recovery seam override,
`bootstrap_schedule_coordinator`, the signal handoff/nif access as
required) in a one-shot slot on the Engine. New
`Engine::run_startup_recovery()` executes those four steps in the same
order; second call and non-deferred call are TYPED errors (loud, per
house law). Not deferred = `build()` behaves byte-identically to today
(every existing caller unchanged; the flag is explicit, no assumed
default).

**Server (`crates/aion-server`, state.rs):** `build_engine` adds
`.defer_startup_recovery()`; `build_with_connected_store` calls
`engine.run_startup_recovery().await?` immediately after
`install_engine_backed_seams(&seams, &engine, …)`.

## Term 2 — the new window, named

After the reorder the gap is: engine constructed + seams installed →
recovery complete. Who can observe the engine in that gap: **nobody
external and nothing in-process.** The HTTP/gRPC/liminal listeners, the
outbox dispatcher task, worker liveness probes, and the embedded
assistant/update-check installs are all started in `run.rs` AFTER
`ServerState::build*` returns (field-verified this incident: gen-4 park
17:43:59.561 < banner 17:44:00.020 < liminal listener 17:44:00.290 —
listeners have always come after state build). Recovery completes
inside `build_with_connected_store`, so the externally observable
ordering is UNCHANGED from today: doors open only after recovery. The
only actors alive in the window are the engine's own internals (NIF
bridges — installed before recovery inside `build()` already, unchanged)
and the seams we just installed — which is the point. No mirror-image
defect: nothing can dispatch fresh work before the listeners exist.

## Term 1 — the red test (committed red-first)

`crates/aion-server/tests/recovery_declared_body_e2e.rs`, corpse-seeded
two-generation shape (no double-writer, no drop-hang):

1. Gen 1: `ServerState::build_with_store(LibSqlStore over temp file)`,
   deploy (HTTP `/deploy`, persisted) an AWL package whose workflow runs
   one declared-command action on queue `local` (command: fast, e.g.
   writes a marker file). NO run started. Drop gen 1 clean.
2. Corpse-seed: over a fresh store handle to the same file, append the
   interrupted history exactly as death #3 left it — WorkflowStarted
   (pinned to the deployed content hash) + ActivityScheduled +
   ActivityStarted attempt 1, no completion. (Direct append is fixture
   construction on a dead store — no engine alive, single-writer law
   untouched.)
3. Gen 2: `build_with_store` over a fresh handle to the same file — the
   REAL defective boot path. Assert within a deadline: the declared body
   EXECUTED (history gains ActivityCompleted / run completes; equivalent
   marker-file check as belt).
   - Current ordering: RED — parks (`NO_LIVE_POLLERS`), never executes.
   - Fixed ordering: GREEN — adoption re-dispatch finds the installed
     body source and executes at the server.

## Terms 3-6 (riders and proofs)

- **T3**: `declared_body.rs:126` empty-source arm logs loud (ERROR-class:
  a dispatch consulted the body source before install — with the fix
  this line firing means an ordering regression). The false-premise doc
  comment on `install_declared_bodies` (state.rs:1491-97) REWRITTEN to
  the true contract (recovery is deferred until after install; the old
  "nothing is deployed before the engine exists" clause struck).
- **T4**: `queue_service::wait` park with no availability deadline emits
  an event on the cluster/event surface (operator-visible), not only a
  WARN. Scoped to what the wait path can already reach; #264 consumes.
- **T5**: #259 (parked re-drive) stays open — this lane removes the
  recovery trigger only.
- **T6**: field proof at deploy — the fixed binary's recovery adopts and
  completes pulse-6 (wid 1f032477) under watch. That settles the
  specimen; nothing else does.

## Battery

Same five-leg shape as the death-note lane (fmt WRITE / scoped test /
test-aion-server / clippy-workspace / clippy -p aion-server) PLUS
`cargo test -p aion-rs` (engine crate is touched — battery scope follows
the closure, not the diff). Box is mine (Themis released post-SLEEP-001).
Battery holds the worktree's mutating lane — no commits mid-run.
