# #139 live cut — sequence for moving the running assistant onto the bodied provision

**Date:** 2026-08-05. **Seat:** Vesper Lynd. **Status:** RATIFIED by Waffles
2026-08-05 (with the step-4 measurement gate below); server-swap step
awaits Tom's go, which Waffles holds and will raise at the next natural
touchpoint. Until his word returns: no build, no swap, no deploy. Companion: `BRIEF-139-LEG-A-REWORK.md`, `BUNDLE-139-NOTES.md`.

## The facts the sequence is built on (all code-verified, file:line held)

1. **Deploy IS the route flip.** `aion deploy <file.awl>` compiles, loads,
   and unconditionally publishes routes — the new version is route-active
   the moment the deploy returns (`engine/reload.rs:48-96` →
   `catalog_load.rs:336-365`). `aion route` exists only for
   rollback/roll-forward between already-loaded hashes.
2. **The live server binary cannot run the cut.** Installed 0.11.0 predates
   `fe702945`, the ONLY commit carrying the `{workspace_root}` machinery.
   Deploying the bodied document to it would dispatch the literal
   placeholder: the script would `mkdir "{workspace_root}"` relative to the
   server cwd and record that garbage path into durable history. **The
   server must be swapped to a main-built binary BEFORE any bodied deploy.**
3. **Admission binds a worker to every REACHABLE version** (route-active ∪
   registered non-terminal runs ∪ starts in flight), never partially: one
   missing action refuses the whole registration
   (`engine/admission.rs:164-215`, `contracts.rs:145-152`). The old
   contract `6f43c809` declares `assistant_provision` worker-served, so a
   worker rebuilt from current main (which deleted that handler and its
   advertisement) is REFUSED entirely while any `6f43c809` run is
   non-terminal-registered — pinned by
   `admission_tests.rs:158-206`. Deploying the new version removes only the
   route-active reason, immediately.
4. **Admission is pre-flight only.** The currently-connected worker is
   never re-validated (`admission.rs:36-48`); it advertises
   `assistant_provision` and is compatible with BOTH contracts (the new
   version simply doesn't demand it). It survives the whole cut untouched.
5. **Post-deploy, provision never reaches a worker.** Declared-body lookup
   scans the retained contract set by `(task_queue, action)`, unpinned from
   the run's version (`declared_body.rs:110-141`) and intercepts before
   task-queue routing. Even an old-version run that resumes with an
   unfinished provision re-dispatches into the server-run body. Recorded
   completions replay from history and never dispatch
   (`durability/resolver.rs:217-221`).
6. **Ambiguity guard:** two retained versions declaring DIFFERENT bodies
   for the same action name refuse every dispatch terminally
   (`declared_body.rs:277-282`). Our two documents' run lines are
   byte-identical (sha256 `e377d3e8…`), and `6f43c809` declares no body at
   all — no conflict.

## The sequence

**Step 0 — freeze (in force).** No worker rebuild/restart anywhere. Holds
delivered to Seth Crackers and Frodo Baggins 2026-08-05; Waffles is
treating worker rebuilds as frozen until this cut lands.

**Step 1 — build, no live impact.** Build the `aion` binary from main
(≥ `fe702945`) on Tom's box. Touches nothing running.

**Step 2 — pre-flight.** From the repo root: `aion versions` (record
`6f43c809` route_active), `aion list` (enumerate non-terminal assistant
runs and note which are pinned to `6f43c809` — these are what later gates
the worker rebuild), confirm store is `<repo>/aion-data`.

**Step 3 — server swap (NEEDS TOM'S GO).** Stop the running server, start
the new binary **from the repo root** (the store is cwd-relative — the
wrong-store trap). Verify: `aion versions` non-empty with `6f43c809` still
route_active, banner carries the new `workspace_root` field, console
reachable, worker re-registers (the old binary re-registering is admitted:
it advertises a superset). Named risk: a restart resumes resident runs
through replay; the standing beamr JIT blocker (compiled frames re-entering
the interpreter with empty services) intersects any long-running resumed
workflow. The Aug 4 reboot already exercised this path on these same runs
without observed loss, but the risk is Tom's to accept alongside the swap.

**Step 4 — the cut itself, GATED ON A MEASUREMENT (Waffles' condition,
ratified 2026-08-05).** The deploy does not fire because the swap was
performed; it fires because the RUNNING server proves it carries the
expansion machinery. Two live facts, both from the server itself, both
required: (a) `/build` reports a commit at or after `fe702945` (the #123
machinery — the running binary says what code it is), and (b) the startup
banner carries the `workspace_root` field with a resolved path (the field
does not exist in any earlier binary, and a value rather than
`unresolvable:` proves the root actually resolved). Only then: `aion
deploy examples/assistant/awl/assistant_spike.awl`, then `assistant.awl`.
Each deploy routes on return. Old runs keep replaying `6f43c809`; new
starts resolve to the bodied version.

**Step 5 — proof.** Start a spike run: provision executes SERVER-SIDE
(first live exercise of the expansion), stdout in history must be EXACTLY
`<home>/clones/<run_id>/repo`, worker serves the agent round, run
completes. Then a real assistant start the same way. Unrun = unproven.

**Step 6 — freeze lift, gated.** The worker rebuild/restart waits until
`aion list` shows NO non-terminal run pinned to `6f43c809` (fact 3 —
a provision-less worker is refused while one exists). The current binary is
compatible with both sides, so there is no urgency: the freeze lifts for
the worker only when the last old-version session ends. The restart then
goes through `scripts/setup.sh`, which threads the server's banner
`workspace_root` into `AION_WORKSPACE_ROOT` (F4) — the final leg A claim
proven live. Announce the lift on the same threads the holds went out on.

**Rollback:** `aion route assistant 6f43c809` (and spike likewise) flips
the route back — both versions stay loaded; `unload` refuses route-active
and live-pinned versions by design.
