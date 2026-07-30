# Fleet: THE THARSIS RUN IS LIVE. State at 2026-07-30 ~07:15Z

Supersedes `FLEET-SMOKE-STATE.md` as the resume point (that file remains the
record of the first smoke flight and its three faults; everything in it is
still true).

## LIVE RIGHT NOW — Tom's own server, not a scratch one

Tom gave the explicit order at 07:05Z: "just kill all the running ion processes
and spin it up for me". All three earlier processes were killed and the estate
was brought back up properly.

- **Server**, `./target/release/aion server`, started 07:08Z from the repo root,
  log `~/.aion/server.log`. HTTP **127.0.0.1:8080**, gRPC **50051**, liminal
  worker listener **50061**. Store: the repo-local legacy `./aion-data`
  (haematite, 64 shards) — Tom's real state, preserved across the restart.
  Authoring root `~/.aion/authoring`. Version 0.10.0, deploy enabled.
- **Worker**, `examples/staged-rounds/worker/target/release/staged-rounds-worker
  --profiles-dir ./profiles --identity tharsis-fleet --norn-account default
  --max-parallel 4`, log `~/.aion/worker.log`. **All five connections
  registered** (4 agent nodes + shell).
- **THE THARSIS RUN**: workflow `9da28cf9-5eaa-45b8-9b3d-eccc194dceed`,
  run `8fed4511-401c-49dc-b2e7-44d3624dffdf`, started **07:10:44Z**, status
  `Running`. Input `examples/staged-rounds/awl/tharsis-input.json` against
  `/Users/tom/Developer/ablative/dev-ops/tharsis` — 5 gates
  (fmt/build/clippy/test/scope-fence), 4 review lenses (correctness,
  regressions, brief_compliance, spec_fidelity), max_rounds 8, 15 doc pointers,
  a 5863-char brief. A real Norn planner process is alive and reading tharsis.
- A persistent Monitor (`bp7y2n4zk`) watches step transitions, terminal status,
  and either process dying.

### NEW CONFIG WRITTEN: `~/.aion/config.toml`

This is what makes the worker channel exist at all. `store.data_dir` and
`authoring.workspace_dir` are DELIBERATELY ABSENT — with the field unset the
resolver picks the legacy working-directory location (`./aion-data`,
`./aion-authoring`) over the home default, which is where the existing state
lives. Setting them would silently point the server at an empty store.

### Canvas

`staged_rounds.awl` (302 lines) is loaded into the studio via
`POST /awl/documents` + `PUT /awl/documents/staged_rounds.awl`, on disk at
`~/.aion/authoring/staged_rounds.awl`, and served by `GET /awl/documents`.
Console route is **http://127.0.0.1:8080/authoring** (routes extracted from the
asset bundle: `/authoring`, `/workflows`, `/registry`, `/actions`, `/failover`,
`/incidents`, `/namespaces`, `/kit`, `/assistant`). Awaiting Tom's read of it —
he considers the authoring surface the most important thing in the project.

## 🔴 #80 FOUND LIVE — a stale deployed version makes the queue unservable

Four of five connections registered; the shell connection was refused on every
dial. Cause: TWO versions of `staged_rounds` were loaded — `deb845e4…`
(`route_active: false`, older, whose `verify_integration` had no `base_branch`)
and `5f0f3bf1…` (routed, current). Contract admission gathers EVERY package
deployed on the task queue and demands the connection satisfy all of them at
once. A worker can advertise only ONE shape per action, so the historical
version made the queue permanently unservable by a correct, current worker.

This is not a curiosity: content-hash module namespacing exists precisely so a
long-lived workflow keeps running while a new version deploys (load-bearing
invariant 5). Admission turns that coexistence into mutual exclusion at the
worker boundary — the old version locks the door behind it. It sits directly
across the hot-load / living-software direction.

Failure MODE is the same silent class as #78: a WARN in the server log, a
retry-with-backoff in the worker log, the server reporting healthy, and the
worker merely looking asleep.

UNBLOCKED BY (workaround, not the fix): `POST /deploy/unload` with the stale
`content_hash` — it refuses a ROUTED version, which is correct. All five
connections registered immediately after. There is NO `aion undeploy` CLI verb;
the HTTP route is the only removal path and nothing tells an operator it exists.

Four candidate semantics and a recommendation are recorded on task #80. Needs a
ruling; do not guess.

## 🔴 #81 — REPEAT COMPLAINT, chmod/mkdir ergonomics

Tom, 07:11Z, angry that this recurred after he raised it ~2 weeks ago. The
standard he set: **a refusal an operator can only resolve by running a shell
command we could have run ourselves is a defect, not a documented step.**

Two distinct cases needing different answers: (1) a path WE own and need (aion
home, data root, authoring root) that is missing or too permissive — create it
owner-only and atomically, or tighten it ourselves and log what changed; (2) a
world-writable ANCESTOR we do not own (anything under `/private/tmp`, mode
1777) — keep refusing, that gate is correct, but name path + problem +
remediation. Surfaces: `aion-server/src/filesystem.rs` (~183 `0o077`, ~476
`0o022`, ~525 `0o077`), `config/resolution.rs`, `config/file.rs`,
`crates/aion-darwin-acl/`.

## In flight (two Opus subagents, neither may commit)

1. **#79** — fenced mechanical commits in `examples/staged-rounds/worker/`.
   Ruling recorded on the task: always stage tracked modifications; stage an
   untracked file only inside the caller-supplied fence (the item's `scope_in`);
   record every excluded path by name, including on a skip. Empty fence
   (integration fix, merge conclusion) = tracked-only. Baseline to beat: 154
   passed / 0 failed / 0 ignored.
   NOTE: tharsis gitignores `target/`, so #79 does NOT block the live run.
2. **#81** — the permissions taxonomy + fix in `crates/aion-server/`.

**Cally Ray has been briefed on #30 (the AWL worker language)** at Tom's
explicit request, with the measured gap: layer-1 command bodies already SHIPPED
(commit 95426218, task #60, `aion-awl/src/ast/command.rs` +
`checker/bodies.rs`), scaffold generators already exist
(`aion-package/src/codegen/activity_worker_rust.rs`), but `aion generate` is
Gleam-project-only so nothing on the `.awl` path reaches them. Paper of record:
`docs/design/WORKER-AUTHORING-STORY.md`. Suggested first cut: the file-kind
declaration and the checker's namespaced word-sets, because everything hangs off
it and it cannot be retrofitted cheaply.

## Commits: still NOTHING pushed

Local on main: `798677ee` (#78), `a78734d3`, `00bddc26`, `5f722697`,
`0b8d1d09`, plus earlier `9a44483c`, `3c79535f`, `e6bf8679`, `036f976d`,
`3ac1b3ec`.

## Next, in order

1. Watch the tharsis run. The half that MERGES items and verifies the union
   (`merge_once`, `fix_conflicts`, `verify_integration`, `fix_integration`) has
   NEVER executed — first contact is first contact, and finding real bugs there
   is the expected outcome, not a failure of the exercise.
2. Verify both subagents' bytes at my own seat before landing anything.
3. `aion describe` dies over 4 MiB of history, so a tharsis-scale run cannot be
   read through the CLI. Watching is through the console. This is itself worth a
   task if it bites.
4. File the opaque-terminal-outcome finding — DONE, folded into #34 as item (5).
5. Push.

## Tom's standing asks, current status

- **Fleet stress test on tharsis** — RUNNING, first time.
- **AWL worker language (#30)** — gap measured, handed to Cally Ray.
- **Signals (#24)** — sender exists everywhere except AWL; design at
  `docs/design/aion-authoring/SIGNAL-SENDER-DESIGN.md`; three open questions
  await a ruling. Addressing is the hard part: `workflow.id` (self) is the only
  builtin and `spawn` yields no handle, so parent-directed sends work today and
  child-directed ones need a spawn binding.
