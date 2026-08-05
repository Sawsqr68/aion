# Seat resume — marker AJ, 5 August 2026

Supersedes AI. **Latest letter wins.** AI's record stands (it holds the closed
#144/#145 detail and the standing constraints); everything below is newer.

`origin/main` = `b6fae1ca`. **The working tree is DIRTY with unfinished #84
work** — see below. Nothing of #84 is committed.

## 🔴 IN FLIGHT — #84, uncommitted, UNBUILT and UNTESTED

Stopped mid-step at the context ceiling. The edits are written but **`cargo
clippy`/`cargo test` have NOT been run once on them**. Assume nothing compiles
until measured.

### What #84 actually is (bigger than its title)

The title says "declared-body resolution scans the raw retained set". Reading
the code, the raw-vs-reachable framing is a red herring: filtering to the
reachable set does **not** fix it, because a live run's own version is reachable
(`AdmissionReason::LiveWorkflow`) and so is the new route-active one — both stay
in the set and the ambiguity survives.

The real defect: **`DeclaredBodies::body_for(task_queue, action)` has no run
parameter, so it cannot answer correctly.** A body is a property of the run's
own package version, not of the queue. Consequence, and it is a durability
violation, not an ergonomic one: a run started under version A and still in
flight fails **terminally** at its next bodied activity the moment version B is
deployed with an edited command — a version that run will never execute. That
contradicts the whole point of content-hash namespacing.

Corroboration that this is a live constraint people already work around:
`BRIEF-139-LEG-A-REWORK.md` R2.4 orders two documents to carry a
**byte-identical body** forever, precisely because "different bodies for one
action name refuse every dispatch terminally".

### The fix as written

New `crates/aion-server/src/worker/declared_body_selection.rs` —
`select_declared_body(contracts, action, run_version: Option<&ContentHash>)`:

- **`Some(version)`** → that version's contract is the SOLE authority. It
  declares a body → run it. It declares none (or does not serve this queue) →
  `None`, delegate to a worker. Never borrow another version's body: that would
  run a command the run's own package never declared.
- **`None`** (run's version unnameable) → today's queue-wide reading, ambiguity
  refusal and all. Without the run's identity, choosing IS the guess the refusal
  exists to prevent. This is why the fallback is safe: it is byte-for-byte the
  old behaviour.

Six unit tests in that file, including the **control** — the same retained set
read with `None` must still refuse, so the two positive assertions cannot pass
vacuously.

Changed in `declared_body.rs`:
- new `pub struct DispatchingRun<'a> { workflow_id, run_id }`
- trait `DeclaredBodies::body_for` gains a `run: DispatchingRun<'_>` parameter
- `DeclaredBodySource::body_for` forwards it
- new `EngineDeclaredBodies::version_of(run)` → `registry().get(workflow_id,
  run_id)` → `handle.loaded_version().clone()`; `Ok(None)` warns, `Err` errors,
  both return `None` (never a silent switch)
- `EngineDeclaredBodies::body_for` = contracts + `version_of` →
  `select_declared_body`
- the dispatcher builds `DispatchingRun` from the request
- test double `FixedBodies` takes the new parameter

`worker/mod.rs`: module declared + `DispatchingRun` / `select_declared_body`
re-exported.

### NEXT STEPS, in order

1. `cargo clippy -p aion-server --all-targets -- -D warnings` — expect first-pass
   errors; nothing has been compiled.
   - Known risks: `aion_core::{WorkflowId, RunId}` path on `DispatchingRun`
     (aion-server may re-export them elsewhere); whether `Registry::get` is
     reachable as `engine.registry().get(..)` from aion-server; `ActionContract`
     field list in the test fixture (I filled `name/input_schema/output_schema/
     node/timeout/retry/advisory/agent/body` — verify none added/renamed);
     clippy `too_many_arguments`/`needless_pass_by_value` on the new signature.
2. `cargo fmt --all`.
3. `cargo test -p aion-server` — baseline to beat is **944 passed / 0 failed**
   at `b6fae1ca`. Expect 944 + 6 new = 950.
   - ⚠️ `cargo test` STOPS at the first failing target, so a red run's totals
     are truncated and NOT comparable. Re-run clean before quoting a number.
   - The libSQL flake `schedule_namespace_enforcement_survives_restart`
     ("bad parameter or other API misuse", known #85 shape) appeared once today
     and passed alone and on a clean re-run. It is unrelated to this change.
4. **Mutation-check the fix**, as with #145: make `version_of` return `None`
   unconditionally and confirm
   `an_in_flight_run_keeps_its_own_versions_body_after_a_redeploy` dies. A fix
   whose test survives its own removal proves nothing.
5. Docs: `docs/workers/DECLARED-COMMANDS.md` §"Changing a body makes a NEW
   version, and the old one stays" now describes the OLD behaviour for
   in-flight runs — it must say a run in flight keeps its own version's body and
   that the refusal is only reached when the run cannot be named. The
   `DECLARED-BODY-FINDINGS-2026-08-05.md` finding-2 note likewise.
6. Consider whether `BRIEF-139-LEG-A-REWORK.md` R2.4's byte-identical-body
   order can be relaxed once this lands. **Do not touch the brief without
   Waffles** — it is a ratified work order.

If any of this turns out to be wrong, **the reversible move is to drop the whole
change** (`git checkout -- crates/aion-server/src/worker/` plus deleting
`declared_body_selection.rs`) and re-open #84 with this analysis. Nothing else
depends on it.

## Board

- ✅ #144 (`483c933a` + `4bed6a81`), ✅ #145 (`fc1c10cc`) — detail in marker AI.
- 🔧 #84 in flight, uncommitted, unbuilt (this document).
- Held on Tom: the #139 live cut, #142, the 0.11.1 decision. Queued: #141.

## Unchanged standing constraints

🔴 Worker rebuilds frozen until the #139 cut — the console is not a worker, and
restarting an already-built worker is not a rebuild. **Editing aion-server
source is not a worker rebuild, but do not restart Tom's live processes.**
🔴 Tom's `~/.aion/config.toml` `[ops_console] FileSystem` line must stay until he
installs a binary built from `b20d2da1` or later. 🔴 beamr JIT blocker with
Artemis. ⛔ Never `git add -A` here — name paths. ⛔ Never bare `cargo clean`.
Backticked commit messages via `git commit -F -`. Do not edit `CLAUDE.md`.

Live pids, untouched all session: server **45425**, worker **48399**, both from
the repo root. Server log: scratchpad `server.log`.
