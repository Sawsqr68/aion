# BRIEF — dispatch rigs route through the norn_dev workflow (Tom's standing rule)

DIRECTIVE: Tom, 2026-08-15 13:35Z, direct order — dev work is dispatched
THROUGH the deployed aion workflows, visible in the ops console. Detached
`nohup norn` dispatch is DEAD for dev work from now on. This lane converts
the three staged dispatch rigs to the workflow path. This brief is your
complete charter; execute it in full.

## Where you are

Your working directory is the aion main repository
(`/Users/tom/Developer/ablative/stack/aion`). You are NOT in a worktree —
you are editing untracked operational rig files only.

**HARD FENCE — you may create or modify ONLY these paths:**

- `gate-logs/246-adoption/dispatch.sh` (rewrite)
- `gate-logs/247-transcript-spool/dispatch.sh` (rewrite)
- `gate-logs/libsql-retirement/dispatch.sh` (rewrite)
- NEW files inside those three lane directories (per-lane `gate.sh`,
  `build-input.py` or equivalent, notes)

You must NOT: modify any git-tracked file (the gate enforces
`git diff --quiet`), commit, push, run cargo or any build, start or run any
workflow, execute any dispatch.sh, touch `~/.aion`, touch any server or
worker process, or write outside the three lane directories. The box is
carrying another seat's test battery — nothing you do may add load beyond
reading, thinking, and writing shell/python text files.

## The deployed contract you are targeting

The live aion server (127.0.0.1, HTTP :8080 / gRPC per config, version
0.15.0) has workflow type `norn_dev` deployed and route-active. Its
contract (from the seated `~/.aion/norn_dev.awl` — quoted here because that
file may be outside your reach):

```
workflow norn_dev
  timeout 4h
  input repo: String            # abs path; the builder agent's cwd — the tree travels with the run
  input control_prompt: String  # known-answer question, asked first
  input control_expect: String  # exact string the control reply must equal
  input brief: String           # the builder's full prompt
  input gate_script: String     # abs path; server runs `zsh $script`, real exit code decides green/red
  outcome done / gates_red / unmeasured
```

The control leg runs before the build: the agent is asked `control_prompt`
in the repo, and if its reply is not EXACTLY `control_expect`, the run
routes to `unmeasured` and no build happens. The gate runs after the build,
executed by the SERVER (not the builder): exit 0 routes `done`, anything
else routes `gates_red` with both streams.

**A proven exemplar lives in this repository — read all three files and
follow their shape:**

- `gate-logs/delivery-rejudge/build-input.py` — builds the five-field input
  JSON from brief bytes at fire time, wraps the brief with a builder
  preamble
- `gate-logs/delivery-rejudge/input.json` — the resulting shape
- `gate-logs/delivery-rejudge/gate.sh` — a server-run gate script (absolute
  paths, logs beside the lane, `EXIT=` lines, echoes per-leg results)

That lane was started with:
`aion start norn_dev --input-file <lane>/input.json --name "<label>"`
and completed on the live server. `aion start --help` is available to you
via bash (read-only; do NOT run `aion start` itself).

## The task

For each of the three lanes — `246-adoption`, `247-transcript-spool`,
`libsql-retirement` — rewrite `dispatch.sh` and author the supporting
files so that firing the script dispatches the lane's brief through
`norn_dev` on the live server instead of spawning a detached norn process.

Requirements:

**R1 — workflow dispatch, no detached norn.** Each converted `dispatch.sh`:
builds the lane's `input.json`, then runs
`aion start norn_dev --input-file "<lane dir>/input.json" --name "<lane name>"`,
captures the server's JSON response verbatim to `<lane dir>/start-response.json`,
and writes a UTC timestamp to `DISPATCHED.marker` as before. Zero
occurrences of `nohup`, zero direct `norn` invocations, in any of the three
scripts. The old `--append-system-prompt` seam does not exist on the
workflow path, so the brief text itself must carry the standards (see R3).

**R2 — worktree logic preserved.** Each script keeps an idempotent
create-if-missing worktree step, matching what the lane already uses:
`246-adoption` → `.worktrees/246-adoption`, `247-transcript-spool` →
`.worktrees/247-transcript-spool` (both already exist at main; the create
step guards with `[[ ! -d ]]` and branches off `main`), `libsql-retirement`
→ `.worktrees/libsql-retirement` created at fire time with
`git worktree add ... -b libsql-retirement main` exactly as the current
script does. The `repo` input field = the worktree's absolute path.

**R3 — the brief input derives from BRIEF.md bytes at fire time.** Each
lane's input builder reads `<lane dir>/BRIEF.md` when the script runs (so a
later amendment re-derives) and wraps it with a builder preamble modeled on
`delivery-rejudge/build-input.py`: work only in the named worktree,
commit-as-you-go, never push, never merge, never touch the live estate or
`~/.aion`; follow the brief's red-first ordering where it demands committed
reds; no lint bypasses, no `#[allow]`/`#[ignore]`, no test skips; reply
with a full delivery report (file-by-file changes, red-then-green evidence
with commit ids, every permitted choice made and why, every disclosed
non-goal).

**R4 — per-lane gate.sh, authored now.** Write a standing
`<lane dir>/gate.sh` for each lane, derived from that lane's own BRIEF.md
"Battery" section (read each one). Shape it on `delivery-rejudge/gate.sh`:
absolute paths only (the server's cwd is not the worktree), each battery
leg run in the WORKTREE with output to a log file in the lane dir plus an
`EXIT=<code>` line, per-leg result echoed to stdout, script exit nonzero
if any leg is red. Note the standing clippy law: `cargo clippy --workspace
--all-targets -- -D warnings` AND per-crate `-p` runs resolve features
differently — where a lane's battery says clippy ×2, the gate runs both
forms as separate legs. The gate script only RUNS at fire time (behind the
queue's walls, on a quiet box) — you author it, you never execute it.

**R5 — per-lane control leg.** Pick a control for each lane following the
delivery-rejudge pattern: a prompt that makes the agent read one stable
file in the worktree and reply with ONLY an exact value, plus the matching
`control_expect`. Verify each expected value at bytes (read the actual
file in this repository at main) before pinning it. State in your report
which file/value each lane uses.

**R6 — fire-gates preserved.** Every converted script keeps its existing
header comments about WHEN it may fire (queue order #246 → #247 →
libsql-retirement, behind Waffles' walls; libsql-retirement's ticket-number
amendment note). You never fire anything: conversion only.

## Report

Reply with: per-lane summary of what changed and every new file written;
the exact `aion start` invocation each script now performs; each lane's
control file/expected value and where you verified it; each gate.sh's legs
and the BRIEF.md battery text they derive from; any assumption you had to
make. Do not claim any verification you did not perform.
