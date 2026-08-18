# fleet_dev on the box — gap read (Vesper, 2026-08-18T12:0xZ)

Commission: Waffles seq=105 (Tom's word). Read `workflows/fleet-dev/fleet_dev.awl`
against deployment on aion-workflows (192.168.50.205), name the honest delta.
Not a build. Read done against the whole document (345 lines) plus the proven
box artifacts (investigate flight, repo_gates on rocketfish).

## D1 — The five mechanical actions are placed for home use, and the move changes their error contract

`plan_check` / `fanout` / `fanin` / `report_check` / `gates` are `run "zsh $script"`
bodies — executed by the SERVER against a tree the server can reach. That is the
home shape: server and blackboard tree share the Mac. On the box the tree lives on
the box, so all five must move to a box worker document served by `aion worker awl`
(the investigate_worker / repo_gates_worker treatment: sealed scripts at
`/home/aion/fleet/bin/`, sha256s in the runbook).

The part with teeth: fleet_dev routes on `RunOutcome.exit_code` (gate red routes on
`exit_code != 0`), but under worker `run json` a nonzero exit is an ACTIVITY FAILURE
and stdout is DISCARDED — repo_gates_worker.awl:38-42 states this exactly, and its
runner therefore exits 0 on red gates and carries the real exit codes as JSON data.
So each of the five gets that proven facing: an exit-zero-always wrapper emitting a
typed record carrying the inner exit code + streams; the workflow's routing
expressions keep their shape, reading the typed field. Without this, a red gate on
the box becomes a retried-then-terminal activity failure with its findings thrown
away, instead of routing to `gates_red`.

Reuse inside D1: `gates` is essentially served already (repo_gates_worker +
gate-runner, box-proven on rocketfish all day). `plan_check`/`report_check` are
trivial new sealed scripts (exists-and-non-empty over `.fleet/`). `fanout`/`fanin`
are the real ports: worktree cut + ordered merge, confined under the run root.

## D2 — No provisioning or delivery legs exist in the document

Authored fleet_dev assumes the dispatcher's tree pre-exists and the merged result
stays on the operator's desk — both true at home, both false on the box. Needed:

- **clone-in**: investigate_ops `clone.sh` reuse, one extension — clone at a named
  ref (current script takes default HEAD; a dev fleet builds from a named
  branch/commit).
- **push-with-receipt** (the genuinely NEW activity, as Waffles predicted): push the
  merged primary branch back to the estate; typed refusal on rejection
  (non-fast-forward, auth, unreachable remote); receipt carries
  {branch, pushed_commit, remote}. Same exit-zero-always facing.
- **teardown**: investigate_ops `teardown.sh` reuse — which requires fanout to cut
  the leg worktrees UNDER the run root (`<root>/legs/<leg-id>`), so the one gated
  subtraction covers primary tree + worktrees + `git worktree` metadata together.

## D3 — Run-root addressing cannot use workflow_id

`repo` and every `Leg.repo` are dispatcher-fixed paths at dispatch time, but
`aion start` mints the workflow id (no `--workflow-id`, the standing gap), so leg
paths cannot embed it. investigate solved per-run addressing with `{workflow_id}`
harness expansion — unavailable for `legs[]` built pre-start. Box answer: the run
root is keyed by a dispatcher-chosen run name carried in inputs
(`/home/aion/fleet/runs/fleet-<name>`); clone, fanout, fanin, gates, push, teardown
all take it. Confinement unchanged: everything under `/home/aion/fleet/runs`.

## D4 — The counsel seat has no box story yet

`command "/opt/homebrew/bin/npx"` is a Mac path; the box (Debian 13) has no
node/npx. The dev (norn) seat is flight-proven; counsel (ACP Claude) is the
unproven seat and needs: node+npx provisioned on the box, the box-absolute command
path, AND Anthropic credentials on the box — which lands squarely on the open
long-run fleet identity question at Tom's desk (his auth vs a dedicated
credential). Also both worker launches take the pinned-PATH treatment (bitten
twice today: worker spawn env, detached shells).

## D5 — Capacity and limits become box questions

`concurrency 3` norn builders + cargo-building gates on 16c/31G KVM: RAM is the
constraint; gate legs inherit the worker's ulimits (the #62 EMFILE lesson — verify
the worker's soft nofile, rocketfish needed 524288); disk = primary clone + N
worktrees + target dirs. And #64 lives on this box class: any gate leg touching
aion-awl `runtime_codecs` can hang the box's beamr VMs — gate composition carries
the #64 exclusion, declared per run, until Artemis lands her fix.

## D6 — Tom's constraint, checked against the document

"Mechanical steps as deterministic structured activities, typed I/O, no agent
improvising git operations": the document already holds this line — fanout/fanin
are sealed scripts routed on results, the judge never re-runs gates, plan_check /
report_check catch eloquent-but-empty replies mechanically. The box port keeps
every one of those seams and D1's re-facing makes the I/O *more* typed, not less.
Nothing in the box delta weakens the constraint.

## Sum

Waffles' prior estimate ("ops worker serves half; build = push script + activity
plus wiring") is right in spirit and light by one item: the five mechanical
actions must MOVE to the box worker with the exit-code-as-data re-facing (D1) —
that is wiring plus a real error-contract change, proven pattern available. New
bytes: push.sh + plan_check.sh/report_check.sh (trivial) + fanout.sh/fanin.sh
ports + the worker document + input rewiring (run name, workspace_root). The one
piece needing a word from Tom before a supervised first run: the counsel seat's
credentials on the box (D4).
