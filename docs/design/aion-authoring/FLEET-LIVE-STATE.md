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

## 🔴🔴 THE THARSIS RUN DIED AT 08:06:17Z — AND THE FAULT IS OURS (task #82)

Read this section first. It supersedes the "IS EXECUTING" section below.

**Every piece of real work succeeded.** Planner, 3x `provision_item`,
3x `dev_item` (real commits on three item branches), 3x `gate_item` — all
`ActivityCompleted`, gate results 54-61 KB each. Then, 14 ms after the last
gate landed, the workflow failed:

```
WorkflowFailed seq 37 @ 2026-07-30T08:06:17.859413Z
  message: "workflow 3 returned error"
  details: {"tag":"AwlActivityFailed",
            "message":"activity engine failure: engine NIF state is not installed for this runtime"}
```

Evidence chain, all verified at my seat:
- `"workflow {pid} returned error"` — `crates/aion/src/runtime/outcome.rs:116`.
  pid 3 = the top-level workflow's beamr process.
- `"engine NIF state is not installed for this runtime"` —
  `crates/aion/src/runtime/nif_state.rs:432`, `ctx.nif_private_data()` was `None`.
- `"activity engine failure: …"` — the AWL error wrapper, so it was hit inside
  an ACTIVITY-dispatching NIF.
- Aion installs the data ONCE: `crates/aion/src/runtime/handle.rs:141`
  (`SchedulerConfig::nif_private_data`).
- beamr 0.16.2 has exactly THREE sites that put it on a `ProcessContext`:
  `interpreter/opcodes/native_call.rs:169` (**gated on `if let Some(svc) =
  ctx.services`**), `interpreter/opcodes/trampoline.rs:166`,
  `scheduler/execution/core.rs:1507`. `native/context/mod.rs:557` and `:616`
  both default it to `None`.
- Failing call site: `plan_reviews(...)` at `staged_rounds.awl:237` — the plain
  activity call right after the THIRD `fork…join` in the `execute` loop.
- JIT correlation, NOT yet a cause: `~/.aion/server.log` "defining function
  funcidNN" at 07:25:52 (44), 08:06:08 (45), **08:06:17.845 (46)**. But
  `grep` of `beamr-0.16.2/src/jit` finds NO NIF call sites at all.

### ✅ SOLVED — it is a BEAMR defect. We are BLOCKED on a beamr release.

beamr 0.16.2's JIT re-enters the interpreter for ANY external call made from
compiled code, and BOTH re-entry edges construct a fresh
`NativeServices::default()` — every facility `None`:

- `jit/runtime.rs:165` `jit_call_interpreted` (external-call edge)
- `jit/runtime_closure.rs:197` `call_interpreted_closure` (closure edge)
- `interpreter/mod.rs:210-217` `run_with_registry` builds the empty bundle

`JitRuntimeContext` (`process/types.rs:269`) carries module/registry/jit_cache
but NOT the services, so those edges have nothing to restore.

**Trigger shape: JIT'd F → bytecode G → NIF.** A direct `Native` import is SAFE
(`jit/runtime.rs:139-145` deopts on `ResolvedImportTarget::Native`). That is
exactly why the two earlier fork/joins and the plain `fold_phase` call worked —
nothing had been compiled yet.

The JIT correlation is now DEMONSTRATED CAUSE, from the log itself: funcid46's
Cranelift IR at `~/.aion/server.log:120` is a one-arity function whose ENTIRE
body is `call fn6(v1, 2141, 5, 1, v0)`, and `fn6`'s signature
`(i64,i64,i64,i64,i64)->(i8,i64)` is precisely
`jit_call_interpreted(process, module_index, import_index, arity, args)`.

🔴 **BLAST RADIUS IS FAR WIDER THAN OUR NIF.** The same empty bundle drops
`atom_table`, `spawn_facility`, `link_facility`, `local_send`, `timers`,
`ets_facility`, `supervision_facility` and **`replay_driver`**. Any BEAM code
below a JIT frame that sends, spawns, monitors or compares atoms is running on
a gutted VM — and the replay_driver hole means a JIT'd frame could take a LIVE
path during replay. That is a durability violation, not a crash. We were lucky
it shouted instead of going quiet.

**Repro + patch PUSHED: branch `defect/beamr-jit-loses-native-services`.**
`crates/aion/src/runtime/jit_nif_boundary_tests.rs` is a DIFFERENTIAL — two
tests, identical three-module graph, the ONLY variable is whether compiled code
is in the scheduler's JIT cache; compilation goes through `JitCompiler::compile`
on `Module::function_instructions`, byte-identical to `CompilationJob::run`, no
faked entry. 0.32 seconds. VERIFIED AT MY OWN SEAT: control ok, JIT case FAILED
0 vs 1. `beamr-jit-services.patch` (5 files, ~70 lines) proven on an isolated
0.16.2 copy: pure-beamr repro 1p/1f → **2p/0f**, beamr's own `cargo test --lib`
**1726 passed / 0 failed**, non-regressive.

🔴 **The pin is RED and stays red until beamr ships. It is deliberately NOT on
main** — a red test blocks every gate. Branch only.

**NO AION-SIDE DEFENCE EXISTS in 0.16.2.** Every lever checked: `jit` is a
MISLABELLED optional feature (`scheduler/mod.rs:214` gates the `use` on
`threads`, not `jit`) so it cannot be turned off; `SchedulerServices` offers no
JIT choice; `dirty_cpu_threads: Some(0)` is a non-starter because we register
five production dirty NIFs (`engine_nifs.rs:129-153`); and
`jit_threshold = u32::MAX` is a MITIGATION not a gate — the counter saturates
and still fires — as well as an arbitrary limit. Deliberately NOT applied.

Sent to **Artemis Peach** with the three riders they should own: the mislabelled
`jit` feature, `tune_threshold` (`jit/profiler.rs:203`) having no production
callers, and a dirty NIF below a JIT frame mapping `DirtyCall` to a bare deopt
that discards the partially-executed compiled frame.

🔴 **DO NOT RE-RUN THARSIS before the beamr fix lands.** It dies in the same
place and costs another hour.

**The gates did their job on the way past.** Decoded gate result for
`d1-project-foundations`: `gate scope-fence exited 1` listing `.github/
workflows/ci.yml`, `clippy.toml`, `rust-toolchain.toml`, `rustfmt.toml`,
`spikes/s*/run.sh`. That is the fence catching a real out-of-scope write — the
loop was working, and the item would have gone to the review panel and been
folded back as blocked. The engine died before the panel ran.

## THE RUN AS IT EXECUTED (historical)

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

Everything below the divider that used to say "UNCOMMITTED" is now COMMITTED.
The sections are kept for their reasoning; ignore their staging language.

- `8416af54` docs(fleet) — earlier state + #80/#81.
- `d2b32d84` **#79 item-stage fix** — clippy exit 0, 176 passed / 0 / 0.
- `45fdaaee` docs(fleet) — the pre-compaction bank.
- `be0112bf` **feat(awl): send** — the whole signals lane + 8 aion-server Send
  arms + 3 console sites. aion-awl clippy 0 / **467 passed / 0 / 0**.
- `e78f4b44` **fix(server): #81 chmod** — 8 cases, 3 auto-repaired, 5 still
  refusing by design. clippy 0 / **658 passed / 0 / 0**.
- `93e961f6` **fix(awl): swallowed rollback** — my rider. clippy 0 /
  awl::documents **7 passed / 0 / 0**.
- `b355fc41` docs(fleet) — the #82 evidence chain.
- `5008a747` **fix(staged-rounds): verify_integration rider** — gated in the
  REAL tree this time: clippy 0 / **178 passed / 0 / 0** across 8 suites.
  #79 now CLOSED, both halves.
- `9e544085` + `eff9bd88` docs(fleet) — `THARSIS-RUN-1-POSTMORTEM.md`, what the
  three agents actually built, and the re-run checklist.

**Worker release binary REBUILT** (exit 0). The RUNNING worker pid 49833 still
executes the old code from its own inode and MUST be restarted before run #2.
`target/release/aion` is still MISSING — rebuild it after #82 lands, not before
(the fix changes `crates/aion`, so building now is wasted work).

**Canvas VERIFIED live, not assumed:** `/awl/documents` lists `staged_rounds.awl`;
`/awl/check` returns **0 diagnostics, 12 steps, 11 edges, 32 statement nodes**,
and the `execute` step projects the loop with all four nested parallel forks and
their join bindings in the right order. The server hands the canvas everything
it needs; I cannot verify the browser render from here.

**THREE AGENTS IN FLIGHT:** #82 NIF root-cause (`crates/aion/src/runtime/`,
already scaffolding `jit_nif_boundary_tests`), the worker-scaffold lane
(`crates/aion-package/src/codegen/awl_worker/` — 10 files, all under 500 lines,
and the 135-line `emit_declaration` that was failing workspace clippy is gone,
longest fn now 71), and #80 admission (`aion-server` + `aion-package`).

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

## #81 PERMISSIONS — LANDED IN THE TREE (uncommitted), Tom's repeat complaint

Root: `ConfinedDir::require_private_mode` (`filesystem.rs:183`) and
`validate_private_root` (`:525`) refused on `mode & 0o077` telling the operator
to `chmod 700` — while `harden_tree` (`:334`) ALREADY chmods the whole data
tree to 0700/0600. It just ran AFTER the refusal that made it unreachable.

Fix: `ConfinedDir::ensure_private_mode` tightens in place when the server's euid
owns the root, decided entirely against the ALREADY-OPEN descriptor (`fstat` via
`dir_metadata`, `fchmod` via `rustix::fs::fchmod`) so there is no second path
resolution and no TOCTOU window; the applied mode is re-read from the descriptor
because some filesystems accept a chmod and discard it. `ensure_private_home`
now goes through `open_or_create`, so `~/.aion` is CREATED owner-only if absent
(it previously returned `Ok` on NotFound and created nothing).
`filesystem.rs` split: `filesystem/ancestors.rs` holds the ancestor gate — a real
seam, roots we own and may repair vs ancestors we must never touch. 369 + 75 lines.

Still refusing, deliberately, each pinned: world-writable ancestor, foreign-owned
ancestor, macOS ACL granting a foreign principal, symlinked root (target's mode
explicitly left alone), foreign-owned root (its own message now, naming path /
mode / owner uid / server uid — the old one named a `chmod` that would have
failed for the operator too).

Gates: `cargo clippy -p aion-server --all-targets -- -D warnings` exit 0;
`cargo test -p aion-server` **658 passed / 0 failed / 0 ignored**; fmt clean.

`RELEASE-0.9.2.md` — I withdrew the "run `chmod 700 ~/.aion`, that's the privacy
gate doing its job" paragraph. That was the sentence Tom actually read.

## MY OWN RIDER ON TOP (uncommitted): the swallowed rollback

`awl/documents.rs:128` had `if let Ok(workspace) = ConfinedDir::open(root)` and
`let _ = workspace.remove_file(…)` — TWO swallowed errors in the rollback that
removes a document whose first revision failed to store. Cost of the swallow is
not a stray file: the orphan makes every retry of that create fail with `Exists`
for a document that was never created, with nothing anywhere saying why. It also
ran raw sync I/O inline on the async runtime while every other call in the module
goes through `blocking`.

Fixed: new `rollback_created_document` on the blocking pool, NotFound-tolerant
(nothing to undo is not a failure), and a new `DocumentError::CreateRollbackFailed
{ path, reason, rollback }` carrying BOTH causes — plus its own
`DocumentCreateRollbackFailed` HTTP error type, because a caller that cannot tell
it from a plain I/O error cannot explain it to the operator.

Three tests, all passing, incl. the one that pins the ACTUAL defect
(`a_rollback_that_cannot_remove_the_document_reports_it` — chmods the workspace
0500 so `remove_file` fails with PermissionDenied and asserts it propagates;
root-guarded at RUNTIME with a logged skip, never `#[ignore]`).
`awl::documents` **7 passed / 0 failed / 0 ignored**; `cargo clippy -p aion-server
--all-targets -- -D warnings` **exit 0, zero diagnostics**. 249 code lines.

## STILL OUT / BLOCKING THE COMBINED GATE

- **Scaffold agent** (`crates/aion-package/src/codegen/awl_worker/`,
  `aion-cli`) — still running. Its `declaration_rs.rs:168 emit_declaration` is
  135/100 lines and currently FAILS workspace clippy. Not fallout from the
  other lanes.
- **#82 NIF root-cause agent** (`crates/aion/`) — just dispatched. Disjoint from
  the `aion-server` work above.

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
