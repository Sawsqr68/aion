# Fleet smoke: THE PIPELINE FLEW. State at 2026-07-30 ~06:00Z

Supersedes the ~05:35Z revision. Written immediately before a context
compaction; this is the resume point.

## WHAT IS PROVEN (live, on a real server — not inferred)

The staged-rounds fleet pipeline **executed end to end for the first time**.
Workflow `01fba187-f224-4a21-b1e7-72dfcf70429d`, run
`84f959f9-d915-4512-ab1f-91414d8902fa`, on the scratch server.

All five worker connections registered (`registered liminal worker in-band` ×5
in the server log at 05:47:12, ZERO `WORKER_CONTRACT_MISMATCH`). Then 16
dispatches, every one `ActivityCompleted`, in this order:

```
05:51:45 planner
05:52:46 fold_phase        05:52:46 provision_item   05:52:47 dev_item
05:54:14 gate_item         05:54:15 plan_reviews
05:54:15 review_item       05:54:15 review_item   <-- the panel, genuinely parallel
05:55:15 fold_phase        (round 1 rejected -> item cycled WITH feedback)
05:55:15 provision_item    05:55:15 dev_item
05:57:03 gate_item         05:57:04 plan_reviews
05:57:05 review_item       05:57:05 review_item
05:57:45 fold_phase        -> outcome `spent` -> wrap_exhausted (max_rounds = 2)
```

Concretely proven by that run: a real Norn planner turn decomposed the brief
into phased items; `provision_item` created a real git worktree and branch
(`staged/01fba187…/add-sub-function`); a dev agent wrote real code; the gate
battery ran the operator's REAL `cargo fmt`/`build`/`clippy`/`test` in the
item's worktree and recorded `test subtracts_two_numbers ... ok` plus a real
diff; a two-lens panel ran concurrently and returned verdicts; `fold_phase`
judged, rejected, and cycled the item with the rejection attached; the second
round ran the whole shape again; and the bounded loop then reported the honest
`exhausted` disposition rather than a false success.

## WHAT IS NOT PROVEN

No run has reached `completed`. Nothing has merged, so `merge_once`,
`fix_conflicts`, `verify_integration` and `fix_integration` have NEVER executed.
The tharsis whole-project run has not been started.

## Commits landed this session (all on main, NOT pushed)

- `798677ee` — **#78**: contract admission demanded the whole queue of every
  connection (the P0 that made the queue unservable).
- `a78734d3` — the AWL signal-sender design + the previous state doc.
- `00bddc26` — `smoke-input.json`, the throwaway first-flight input.
- `5f722697` — **the worker advertised nothing, and never sent Norn an
  account** (both faults, see below).
- Earlier, unrelated: `9a44483c`, `3c79535f`, `e6bf8679`, `036f976d`,
  `3ac1b3ec`, plus the original worker commit.

Gates re-run at my own seat, never taken on report: worker `cargo clippy
--all-targets -- -D warnings` exit 0; worker `cargo test` **154 passed / 0
failed / 0 ignored** (was 136); `cargo clippy -p aion-package -p aion-server
--all-targets -- -D warnings` exit 0; aion-package lib compatibility 11/11;
aion-server `worker_contract_registration` 4/4; aion-server `--lib worker::`
211 passed / 0 failed / 0 ignored.

## The three faults that had to be fixed to fly, in the order they surfaced

1. **#78 — admission demanded the WHOLE queue of every connection.** The server
   routes by (namespace × task_queue × node), so a worker serving several nodes
   opens one connection per node advertising only that node's actions; admission
   ignored node entirely and refused each connection for omitting the others'
   actions. Fixed with `dispatch_can_reach`, the admission-side mirror of the
   registry's `worker_matches_node`, threaded through both transports and reusing
   the registry's own `optional_node`. Refusal text also fixed: `expected
   "declared"` misled me into the wrong fix for a while; it now names why the
   connection owed the action.
2. **The worker advertised nothing.** All eleven activities registered handlers
   with no wire descriptor. Two different mechanisms: the six shell handlers via
   bare `register_activity`, and the five agent roles never touch the registry at
   all (they reach registration through `AgentHarnessConfig`, whose descriptor
   list was empty). Now sourced from the AWL DOCUMENT — `include_str!` +
   `aion_awl::compile` at startup — so the bytes advertised are the bytes
   deployed. `schemars` derivation was rejected: it is a second independent
   rendering, which is the drift the gate exists to catch.
3. **No Norn account.** Every agent run carries `--session-id` and
   `--resume-if-exists`, and Norn refuses an open-or-resume run with no
   `--account`. The worker never passed one, so all five connections registered
   cleanly and then EVERY agent dispatch died `norn: auth error`. `--norn-account`
   is now REQUIRED with a refusal that explains the consequence, passed as a
   LITERAL argument so an alias is never template-expanded. **This is a
   recurrence of a trap this estate already paid for once** (see the
   preflight-production-argv memory).

## 🔴 #79 — THE ONE BLOCKING CONVERGENCE (filed, NOT fixed)

The gate battery runs `cargo build`/`test` in the item's worktree, which creates
`target/`, and the worker's commit of the round's work is UNBOUNDED — it commits
whatever the battery left. Measured: `git diff --name-only main..staged/<id>/…`
= **96 files, 94 of them `target/`**, plus `Cargo.lock` and the one legitimate
`src/lib.rs`.

Both lenses then correctly rejected the item, both rounds, for breaching its
scope fence ("still contains 95 committed files outside the sole allowed path").
The review layer WORKED PERFECTLY; the bug is upstream of it. No repository
whose build output is not already gitignored can converge, and it is invisible
from outside: every activity green, the agent correct, the run still
`exhausted`.

Two candidate sites, to be attributed before fixing: `src/commit.rs` (the dev
role's post-turn commit) and `src/handlers/gate.rs` (the normalization commit
that is documented as committing a write-mode formatter's mutations). Task #79
carries the full options analysis; the recommendation is a TRACKED-ONLY commit
for the gate battery (a formatter only ever mutates tracked files, so no
heuristic is needed) and a scope-fenced or loudly-refusing commit for the dev
turn. The clean-tree assertion also cannot currently distinguish "a gate mutated
tracked files" (a real failure) from "a gate created untracked build output"
(normal) — that distinction is what makes the tracked-only fix safe.

FIXTURE CAVEAT: the smoke repo has no `.gitignore`, which is what surfaced this
so fast. Adding one makes that repo converge but is a WORKAROUND — the worker
behaviour is repo-independent and must still be fixed.

## SECOND FINDING FROM THE RUN (not yet filed): a terminal outcome reads as a crash

The run's status is `Failed` with `failure_reason: "workflow 2 returned error"`.
The structured detail is right there and is perfectly clear —
`{"tag":"AwlOutcomeFailure","outcome":"exhausted","payload":{…rounds:2, ready:[…
with the full rejection feedback …]}}` — but the human-facing reason is opaque
and indistinguishable from a genuine crash. `exhausted` is a DECLARED terminal
disposition of this document, not an error. An operator cannot tell "ran
correctly, ran out of rounds" from "blew up". This belongs with task #34's
"opaque parent failure_reason" and should name the outcome and summarise the
payload.

## Live processes — DELIBERATELY LEFT RUNNING

- **Scratch server pid 16798**, `./target/release/aion server --config
  <scratchpad>/fleet-smoke/server.toml`, `AION_HOME=/Users/tom/.aion-fleet-smoke`
  (mode 700). HTTP 127.0.0.1:**8099**, gRPC **50099**, liminal **50161**, all
  confirmed listening. haematite store at `~/.aion-fleet-smoke/hdata`. Deploy
  enabled. `staged_rounds` deployed, hash
  `5f0f3bf19ed9b6d240eb87bfcb3fe55a6d2c10988c84b448f2c240e7b53fd5a3`.
- **Worker pid 99712**, the release binary, five connections registered.
- Logs: `<scratchpad>/fleet-smoke/{server.log,worker.log}`;
  `server.log.pre-nodefix` holds the original whole-queue refusals, which are the
  evidence for #78.
- `<scratchpad>` =
  `/private/tmp/claude-501/-Users-tom-Developer-ablative-stack-aion/5849e0d8-4802-4869-8e0f-9f7fd187e198/scratchpad`

CLEAN UP WHEN DONE: kill 99712 and 16798, `rm -rf ~/.aion-fleet-smoke` (that
also removes the smoke repo). Only Tom's :8080 console is meant to be live.

## Tom's server — still HIS call, still untouched

pid 23302 listens on 8080 and 50051 only; no outbox liminal listener, so no
worker can attach there. Needs `[outbox] enabled/transport/liminal_listen_address`
in a config FILE plus a restart, and he has a pending decision about swapping
that server anyway. Facts established while proving this: there is NO
`AION_OUTBOX_TRANSPORT` env override (so env alone can never enable the
listener — worth its own task); `outbox.enabled` is refused with the memory
store; `store.backend = "libsql"` is refused by the release binary for lacking
the `libsql-backend` feature, haematite works; the path gate refuses a
`data_dir` under `/private/tmp` (ancestor mode 1777); and a project-local
`./aion.toml` plus the legacy `./aion-data` directory SHADOW `AION_HOME`, so
anything hermetic must set `store.data_dir` explicitly.

## Next steps, in order

1. Fix #79 (needs the design ruling in the task, do not guess). Re-fly the smoke
   run and prove it reaches `completed` — that is what exercises `merge_once`,
   `verify_integration` and the integration loop for the first time.
2. File the opaque-terminal-outcome finding against #34.
3. Only then the tharsis run:
   `./target/release/aion --endpoint 127.0.0.1:50099 start staged_rounds \
      --input-file examples/staged-rounds/awl/tharsis-input.json`
4. Push. Nothing is pushed yet; everything is local on main.

Worker start line (the `--norn-account` is now mandatory):

```sh
cd examples/staged-rounds/worker
./target/release/staged-rounds-worker \
  --address 127.0.0.1:50161 \
  --profiles-dir ./profiles \
  --identity fleet-smoke \
  --norn-account default \
  --max-parallel 4 \
  --ready-file <scratchpad>/fleet-smoke/worker.ready
```

Watch with `aion list`. `aion describe` DID work on this run (16 dispatches) and
is how both findings above were read — but it still dies over 4 MiB of history,
so it will not survive the tharsis run.

## Tom's other two asks

- **AWL worker language (#30)** — gap MEASURED, not guessed: worker scaffold
  generators already exist (`aion-package/src/codegen/activity_worker_rust.rs`,
  `…_python.rs`) and the Rust one emits a `register_activity_with_contract`
  chain. But `aion generate` is GLEAM-PROJECT-ONLY: its source of truth is
  `src/<package>_io.gleam` and it drives the Gleam toolchain twice. So on the
  `.awl` direct path nothing reaches those generators. Full detail on the task.
- **Signals (#24)** — the sender EXISTS in the engine (`Command::SendSignal`,
  correlation-keyed so replay-correct) and in the Gleam SDK (`signal.send`).
  Only the AWL surface lacks it, so this is a language-surface job over a
  finished mechanism. The hard part is ADDRESSING: `workflow.id` (self) is the
  only builtin and `spawn` yields no handle. Design:
  `SIGNAL-SENDER-DESIGN.md`.

## Process lesson, carried forward deliberately

A compaction can carry a FIX forward while stripping the EVIDENCE for it, so a
seat can honestly disown numbers it genuinely produced. That happened tonight
(transcript line 62556, 01:56:11Z). Defence: resolve the artifact, not the
memory — and in my own summaries carry battery numbers and every deliberately
left-running process forward explicitly. Hence the top two sections and the live
process list in this file.
