# Fleet + lanes: LIVE STATE at 2026-07-30 ~08:00Z (resume point)

Supersedes the earlier revision of this file. `FLEET-SMOKE-STATE.md` remains the
record of the first smoke flight.

## LIVE PROCESSES — Tom's own server (he ordered the restart at 07:05Z)

- **Server** `./target/release/aion server`, started 07:08Z from the repo root,
  log `~/.aion/server.log`. HTTP **127.0.0.1:8080**, gRPC **50051**, liminal
  worker listener **50061**. Store = repo-local legacy `./aion-data`
  (haematite, 64 shards) — Tom's real state, preserved. Authoring root
  `~/.aion/authoring`. v0.10.0, deploy enabled.
- **Worker** `examples/staged-rounds/worker/target/release/staged-rounds-worker
  --profiles-dir ./profiles --identity tharsis-fleet --norn-account default
  --max-parallel 4`, log `~/.aion/worker.log`. All FIVE connections registered.
- **NEW `~/.aion/config.toml`** is what makes the worker channel exist.
  `store.data_dir` / `authoring.workspace_dir` DELIBERATELY ABSENT so the
  resolver picks the legacy working-dir locations where the real state lives.
- Canvas: `staged_rounds.awl` (302 lines) loaded into the studio, on disk at
  `~/.aion/authoring/staged_rounds.awl`. Console route **/authoring**.

## 🔴 THE THARSIS RUN IS EXECUTING

Workflow `9da28cf9-5eaa-45b8-9b3d-eccc194dceed`, run
`8fed4511-401c-49dc-b2e7-44d3624dffdf`, started **07:10:44Z**, status `Running`.
Target `/Users/tom/Developer/ablative/dev-ops/tharsis` (which DOES gitignore
`target/`). 5 gates, 4 lenses, max_rounds 8, 15 pointers, 5863-char brief.

The planner landed at **07:25:51** after ~15 min of provider overload, and the
FAN-OUT FIRED: three phase-1 items, three real git worktrees under
`tharsis/.staged-rounds/`, three branches, three dev agents in parallel:
`d1-project-foundations`, `b1-m1-core-model`, `s1-lite-readonly-export`.

Persistent Monitor **bp7y2n4zk** watches step transitions / terminal status /
either process dying. Last event: `steps=2 last=fold_phase`.

DO NOT rebuild or restart the worker mid-run. The live binary predates today's
worker fixes; harmless because tharsis gitignores `target/`.

### Provider-overload finding (Tom challenged this and was right to)

Bare `norn` (nothing of ours in the path) hit the IDENTICAL
`server_is_overloaded` error ~10 times, then punched through in ~50s and wrote
`/tmp/asparagus.md`, exit 0. So the overload is real and independently
confirmed. But the asymmetry is unexplained-but-plausible, NOT verified: a small
request cleared in 50s while the planner's (full brief + 15 docs + big output
schema + a repo to read) took 15 min. Big requests are shed first.

## COMMITTED THIS SESSION (local on main, NOTHING PUSHED)

- `8416af54` docs(fleet) — earlier state + #80/#81.
- `d2b32d84` **#79 item-stage fix**, VERIFIED AT MY SEAT: clippy exit 0 zero
  diagnostics, **176 passed / 0 failed / 0 ignored** (was 154).

## UNCOMMITTED, IN THE TREE RIGHT NOW — needs one combined gate then commit

1. **verify_integration rider** (`examples/staged-rounds/worker/src/handlers/
   {verify,gate}.rs`, `tests/shell_gate_activities.rs`). Agent reported clippy
   exit 0 + **178 passed / 0 failed / 0 ignored**, measured in a disposable
   detached worktree because the tree did not build. I verified the BYTES (no
   bypasses, `worktree_status` genuinely deleted with no dangling refs, sizes
   fine) but have NOT gated it in the real tree. HOLD until the tree builds.
2. **AWL signal sender** (`crates/aion-awl/`, whole lane). VERIFIED AT MY SEAT:
   clippy `-p aion-awl` exit 0 zero diagnostics, **467 passed / 0 failed /
   0 ignored** across 42 suites.
3. **MY OWN aion-server + console work** (this is the bit that only exists in
   the working tree — see below).

### What I did by hand for the Send variant (8 Rust sites + 3 console sites)

`Statement::Send` / `StatementKind::Send` broke eight exhaustive matches in
`aion-server`. I did them properly, not mechanically:

- `awl/edit_rename.rs` — NEW `rename_send`: renames BOTH the payload args/value
  AND the target expr. Renaming one and not the other leaves a document that
  parses and no longer compiles.
- `awl/edit_studio.rs` `statement_uses_field` — send-aware over payload AND
  target. Answering `false` would let a rename report "nothing uses this field"
  while a send still names it.
- `awl/projection/build.rs` ×4 — `scan_statement_list` (no nested list),
  `collect_waits` (a send does NOT durably suspend), `collect_activities` (a
  send names a signal, never dispatched to a worker), `collect_child_calls` (a
  send starts no child). Each carries a comment saying WHY the no-op is correct.
- `awl/projection/edges.rs` — not a route edge; control continues, and there is
  no in-document node to draw to.
- `awl/projection/statement_build.rs` + `statement_types.rs` — NEW
  `ProjectionStatementKind::Send`, its own kind so a durable outward effect is
  visible on the canvas rather than collapsed into a neighbour.

**Then the console, which the agent did not flag and which would have broken
Tom's canvas:**
- `apps/aion-ops-console/src/features/authoring/lib/facade-semantic.ts` —
  `projectionStatementKinds` is a RUNTIME allow-list; a document containing a
  send would have thrown `Invalid authoring response` and killed the whole
  canvas. Added `'send'`.
- `.../lib/projection-types.ts` — added `'send'` to the union.
- `.../components/StatementTopology.tsx` — `Record<ProjectionStatementKind,
  string>` label map, caught by tsc exhaustiveness. Added `send: 'Send signal'`.

Verified: `cargo clippy -p aion-server --all-targets -- -D warnings` **exit 0**;
console `bun run typecheck` clean; `bun test` **580 pass / 0 fail** (92 files);
`bun run lint` 9 pre-existing warnings, 0 errors.

## STILL OUT / BLOCKING THE COMBINED GATE

- **Scaffold agent** (`crates/aion-package/src/codegen/awl_worker/`,
  `aion-cli`) — still running. Its `declaration_rs.rs:168 emit_declaration` is
  135/100 lines and currently FAILS workspace clippy. Not fallout from the
  other lanes.
- **#81 permissions agent** (`crates/aion-server/src/{config,filesystem}.rs`) —
  still running. Disjoint from my `awl/` edits.

**SEQUENCE WHEN THEY LAND:** run ONE combined gate over the whole workspace
(`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test`, console
typecheck+test), then commit the verify_integration rider, the signals lane, and
my Send-arm work together.

## Rulings made this session (recorded so they are not re-litigated)

- **#79**: one staging rule — always stage tracked mods; stage untracked only
  inside the item's `scope_in`; record every exclusion by name including on a
  skip; integration stages tracked-only. Empty `scope_in` refused at the fold
  boundary + planner prompt + schema `minItems: 1`.
- **verify_integration**: fix it the same way. Leaving two different answers to
  one question in one codebase is worse than either answer.
- **Send-arm no-ops**: each is a positive statement about what a send IS, and
  says so in a comment. A send does not suspend, is not an activity, is not a
  route edge, starts no child — and it IS its own projected kind.

## Signals: residue + Phase 2 (from the agent, worth keeping)

Surface built: header `send name: Type` declares outbound; statement
`send progress(item: x) to coordinator_id`. `send`/`to` are NOT reserved words
(the corpus already has `action send(to: String, …)`). Failed send faults the
step and reaches `on failure`. Gleam fixture compiles under real gleam 1.17.0;
MIR golden committed; `aion@signal:send/3` confirmed exported by the sdk-closure
beams.

Irreducible residue: a LIVE target whose document never waits that signal name
(silent — payload recorded, nothing consumes it), or waits it with an
incompatible payload (loud on the RECEIVER's side while the sender looks
healthy). Unresolvable target and terminal target are both LOUD. Also:
at-least-once delivery (`nif_signal.rs:367` routes then records), and an unused
outbound declaration is not flagged because the checker has no warning severity.

Phase 2 is NOT small: a spawn/child handle does not tell the compiler which
SIGNALS the child declares, and `ChildHandle` is opaque in
`gleam/aion_flow/src/aion/child.gleam` with NO id accessor — so it needs a
`gleam/aion_flow` change before AWL can lower a handle-addressed send at all.
Rule on the child-side signal surface FIRST.

Recommendation banked: add `outbound_signals` to `PackageContract` — today a
deployed contract does not state what a workflow SENDS.

## Open tasks: #80 (stale version makes queue unservable — needs a ruling),
#81 (chmod repeat complaint), #34 item 5 (declared terminal outcome reads as a
crash), #24, #30. Cally Ray briefed on #30.
