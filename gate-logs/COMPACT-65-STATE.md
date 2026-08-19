# STATE — 2026-08-14 ~21:57 local, pre-compact-65 — BRING-UP DONE, pilot next

I am **Vesper Lynd**; sign "— Vesper". Loop STOPPED. Read with COMPACT-64.
Waffles DM = dm:896955e1-86dd-4d6a-9c25-f3f978b189a9.

## TOM'S GO (via Waffles 11:42Z): 0.15.0 bring-up — EXECUTED. All below
## measured at my hand ~11:55Z (21:55 local).

1. **aion 0.15.0 INSTALLED**: ~/.cargo/bin/aion, `aion --version` = 0.15.0,
   sha256 head a8b8614513fbca2f. Install log scratchpad/install-0150.log
   EXIT=0. chmod 700 ~/.aion done (was already 700 — idempotent).
2. **SERVER UP: pid 56424** (`aion server` from repo root, nohup+disown,
   log gate-logs/server-0150-bringup.log). /build answers
   {"version":"0.15.0","built_at":"2026-08-14T11:46:19+00:00"}.
   Config ~/.aion/config.toml: store DELIBERATELY CWD-relative ./aion-data
   (fields absent by design — do NOT set them), :8080 http, :50051 grpc,
   liminal outbox :50061, MCP on, [worker_supervision] configured.
3. **STORE INTACT**: `aion versions` (from repo root) returns 40+ versions;
   norn_dev (c63f0a7a…, route_active) + repo_gates (05df3592…, route_active)
   both present. staged_rounds, mm_fleet_*, assistant (4e234a5b… active) too.
4. **SUPERVISED WORKER**: server spawned managed worker "mm-capture"
   pid 58869 (from [worker_supervision] config) automatically.
5. **ASSISTANT WORKER SEATED: pid 59932** — `aion worker agent
   ~/.aion/assistant.awl --liminal-address 127.0.0.1:50061 --identity
   assistant-worker` (doc extracted from binary via `aion assistant document
   --output`, embedded hash 39b02735…). Worker log
   gate-logs/assistant-worker-0150.log: "registered; serving through the
   composed agent harness", queue assistant, harness norn,
   cwd /Users/tom/.aion/clones ({workspace_root} EXPANDED — worked).
   SERVER side confirms: "registered liminal worker in-band,
   identity assistant-worker, task_queue assistant".

## DEV WORKER SEATED — 2026-08-14 12:01Z (Waffles ruled: pilot rides queue `dev`)
6. **The deployed norn_dev DOES carry a harness section** — question closed by
   reading `GET /awl/deployed/norn_dev/c63f0a7a…` (the #91 read-only surface).
   The repo file has none; the DEPLOYED bytes were deployed with operator
   policy written: kind norn, concurrency 1, reconnect 1s→30s ×100,
   env_pass PATH/HOME/USER/LOGNAME/TMPDIR, **cwd from repo** (per-job tree,
   #204/#228 shape — each dispatch's `repo` input names its working tree),
   args `--account default`. Reply is the TWO-field pre-#205 record —
   matches the 0.15.0 binary exactly. Saved verbatim to ~/.aion/norn_dev.awl
   (sha3b11048e…, byte-identical to served source).
7. **DEV WORKER: pid 74470** — `aion worker agent ~/.aion/norn_dev.awl
   --liminal-address 127.0.0.1:50061 --identity dev-worker`. Preflight
   `--check` clean (queue dev derived, only `delegate` advertised — `gates`
   is run-bodied/server-run, correct all-or-nothing). BOTH sides registered:
   worker log gate-logs/dev-worker-0150.log + server "registered liminal
   worker in-band, identity dev-worker, task_queue dev, connection_pid 2".
8. **Waffles' pilot (his 11:58Z msg)**: shelf exhausted; pilot = the
   runtime-view-delivery leg (kind's renderer from manifest at runtime, no
   rebuild), ONE bounded leg, under-an-hour envelope, floors-not-ceilings
   for the GPT-class norn worker. His explorer maps wiring now; brief within
   the hour. Manifold repo has two of his subagent worktrees (disjoint);
   pilot worktree is a third; ALL landings sequence at his desk. Battery
   flake protocol: mailbox-suite failures re-run solo before any verdict.

## ✅ PILOT COMPLETED GREEN ~12:47Z — workflow 766e0bfe-d8b4-48ba-b1b5-afd2f8991cca
## Route `done`: control exact-match, gate_exit 0 (all four R7 gates EXIT=0,
## composition 635/635 FIRST run — flake re-run never fired). Builder: 2 commits
## eef6428 (red-first) → e58973d (impl) atop f1e7ba0, tracked tree clean,
## STOP-REPORT none, one R4-permitted stated-not-done (refusals list rendered
## nowhere yet). Evidence: gate-logs/declared-faces-pilot/{final-describe.json,
## builder-report.md}; server gate logs untracked at worktree root (NOT builder
## dirt — told Waffles for his byte audit). Norn session 40d7248c-0f69-….
## Outcome sent to Waffles 12:5xZ; HIS re-gate + landing close Tom's milestone.
## Tom notified plain-language. Fleet-shape follow-up (Claude planner/judge legs
## via ACP) banked at Waffles' shelf, sequenced AFTER his re-gate.
## FULL RECORD: gate-logs/declared-faces-pilot/DISPATCH-RECORD.md
- Brief: docs d40cece (manifold declared faces, K6 leg 1). repo input =
  apps/manifold/.worktrees/declared-faces (f1e7ba0, was clean). Control
  received by dev-worker 12:15:23Z. Background watcher byh6r567a polls for
  terminal status. On completion: outcome (text+stop_reason) → Waffles →
  his re-gate → his landing. Preflight: npm ci was REQUIRED (no
  node_modules in worktrees); baseline gates green after 1 solo re-run of
  the known editor-focus flake; flake protocol built into gate.sh.

## REMAINING
- **SMOKE DONE 12:04Z**: `aion start shell_run --input '{"text":…}'` →
  workflow 4a6361de… run b38b0240… **Completed** on pid 56424 — engine
  start→dispatch→server-run-body→complete proven on 0.15.0. (Input contract
  probed honestly: empty input refused naming `text` — #95's boundary.)
- **WAIT ON WAFFLES' PILOT BRIEF** (his 11:58Z: within the hour). On
  dispatch: support the run on queue `dev`; his re-gate + landing close the
  loop. That end-to-end run = Tom's week-goal milestone. Everything owed
  from this seat for the bring-up is DONE and reported.
- API note: /workers, /availability etc. fall through to console SPA —
  real API paths differ; server log + worker log were the verification.

## Standing guards (verbatim, unchanged)
- ~/.aion-196-soak DO NOT rm. /Users/tom/.aion-binary-archive DO NOT rm;
  restore by RENAME (holds two 0.14.1 .prev binaries). liminal exclusion
  #126 (557eed7). NEW PIDS: server 56424, assistant-worker 59932,
  supervised mm-capture 58869 — these are THE live processes now; kill only
  these recorded pids and only with cause. Old pids (17893, 50822) are DEAD
  history. Decisions via Waffles; Tom-facing = PLAIN LANGUAGE. Bash cwd
  persists — absolute paths. Suite output to FILE with EXIT= inside. Both
  clippy resolutions before commits. NO Agent tool/workflows unless asked.
  NO ARTIFACTS. Loadavg ~24-40 from non-ours work — no battery-grade
  timing assertions on this box while loaded.
- #205 CLOSED (bc6703ef4 + §9c amendment 5715ab41a). #82 gate re-measure
  gate-logs/82-remeasure-20260814.md (8b775118a + amendment 101860b69):
  bounded pilot DISCHARGED; multi-hour gates on Artemis' #101 diagnostic
  chain (#196). 0.15.0 does NOT carry #205 — next cut does.
