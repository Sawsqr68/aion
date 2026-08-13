# `gates` — the aion lane battery, run on Aion

A first-party workflow built to BECOME the estate's gate runner (#57). It is
not the gate of record yet: it runs IN PARALLEL with the hand battery under
the F3 discipline below, and cut-over happens on the coordinator's word, not
on a count. One manual start runs seven legs against a named checkout and
leaves three records behind: the run's own event history (the instrument),
the retained full logs (the box's record), and a small manifest file derived
from the history (the record the lane commits).

## What it runs

Seven legs, every one returning `GateResult {gate, exit_code, passed,
output_tail}` — the command's REAL exit code as data, never a derived
pass-claim. `passed` is true exactly when the command exited 0 AND the leg's
coverage floor held.

| # | Leg | Command |
|---|---|---|
| 1 | `fmt` | `cargo fmt --all -- --check` |
| 2 | `clippy_workspace` | `cargo clippy --workspace --all-targets -- -D warnings` |
| 3 | `clippy_server` | `cargo clippy -p aion-server --all-targets -- -D warnings` |
| 4 | `test` | `cargo test --workspace --no-fail-fast` + coverage floor (below) |
| 5 | `bun` | `bun test` in `apps/aion-ops-console` + nonzero-tests floor |
| 6 | `embed` | the non-mutating embed freshness check (below) |
| 7 | `gates_worker` | fmt + clippy (`-D warnings`) + tests of `workflows/gates/worker/` — the battery gates its own instrument |

Clippy is TWO legs because the two invocations are two different programs:
`--workspace` resolves `serde_json/preserve_order` ON by feature unification
and `-p aion-server` resolves it OFF (the #76 posture record).

The `embed` leg is the non-mutating decomposition of
`cargo xtask verify-ops-console`: the xtask verb regenerates the ts-rs wire
types in-tree (`AION_UPDATE_BINDINGS=1`), which rewrites tracked files on the
stale path, and this battery is ruled to mutate nothing. So the leg asserts
the wire types (write-nothing mode), rebuilds the console
(`bun install --frozen-lockfile && bun run build` into the gitignored
`dist/`), and diffs the rebuild against the committed
`crates/aion-server/ops-console-embed/`. The diff excludes dotfiles and
`*.map` on BOTH sides — strictly more permissive than the xtask compare,
which filters the committed side only.

The `gates_worker` leg exists because the worker crate is its own cargo
workspace (root membership is a named follow-up), so the workspace-wide legs
never touch the crate that runs the battery. Its `Cargo.toml` carries the
workspace's deny-level lint block verbatim, and this leg holds it to it.

**Sequencing is reasoned, not decorative.** The workspace cargo legs share
one `target/` build lock, so fanning them out would only serialise them on
the lock while confounding their timings (and box contention is the #85
flake trigger). Exactly one workspace-lock-holder runs per layer: step one
forks `fmt` (rustfmt takes no build lock), `bun`, and `clippy_workspace`;
then `clippy_server`, `test`, `embed`, and `gates_worker` run in sequence;
the manifest leg runs last, on green AND on red.

## An unmeasured gate is never reported — in either direction

The runner (`gate-runner.zsh`) splits VERDICT from INSTRUMENT:

- A **red gate is data**: the runner exits 0 and reports a `GateResult`
  carrying the gate command's real exit code. The workflow routes `failing`
  with ALL seven results.
- A **broken measurement is a refusal**: a required tool absent from PATH
  (checked per leg, for that leg's own binaries), a cargo SUBCOMMAND whose
  component is not installed, a workspace that does not exist or cannot be
  entered or whose HEAD git cannot read, a gate command dying with exit 127,
  a coverage floor that cannot be evaluated, an unwritable log — each makes
  the runner itself exit non-zero, which the worker surfaces as a terminal
  activity failure. The run fails loudly at that leg; no `GateResult` exists
  to mistake for a verdict.

**Why the cargo subcommands get their own probe.** `cargo` existing does not
mean `cargo fmt` or `cargo clippy` can run — those are the separate `rustfmt`
and `clippy` components. Measured on this box, `cargo <missing-subcommand>`
exits **101, not 127** — the same code clippy uses for a real `-D warnings`
finding — so without a probe a host lacking either component writes
`{gate: "clippy_workspace", exit_code: 101, passed: false}` into the record,
indistinguishable from a genuine finding. Each leg asks its subcommands for
`--version` first, and a silent one is a broken measurement. This is live the
moment the workflow runs on a second box.

## Coverage floors: an exit code is not a coverage claim

The `test` leg re-applies the estate's target/result PAIRING control to its
retained full log (ported from `scripts/battery.sh paired_test`, the
#118/#120 record; the naive header-count-vs-verdict-count comparison was
measured there to false-fire on green runs), plus a nonzero-targets floor.
The `bun` leg requires bun's own nonzero `Ran N tests` line. A floor
violation is a RED leg whose `output_tail` begins `COVERAGE CONTROL FAILED:`
— a distinguishable reason, never a green over nothing. A floor that cannot
be EVALUATED (the scan itself failing) is a different thing again: that is a
BROKEN measurement, not a red leg, because "we could not verify this leg" is
not "this leg failed" and must not attribute a fault to the code under test.

**Two limits, both stated, neither closed here.**

1. The floor is evaluated only when the gate command exited **0** —
   deliberately, because the dangerous direction is the vacuous green and a
   red leg is already red. The cost: a RED `test` leg carries **no coverage
   verdict**, so `exit_code: 101` on that leg must not be read as "a test
   failed". It may be a truncation. Answering which is a hand pairing check
   over the retained full log (that is exactly how the r2a `outbox_e2e`
   SIGSEGV was confirmed), not something this workflow has answered.
2. The pairing control is blind to a target that starts and reports
   `running 0 tests`: targets=1, unpaired=0, floor holds, green over nothing.
   This is a hole in the estate's SHARED control — `scripts/battery.sh`'s
   `paired_test` has it too, which is why a faithful port reproduced it — so
   it is a **joint docket item against both instruments**, written up with
   its trap in [`docs/operations/lane-gates.md`](../../docs/operations/lane-gates.md).
   Neither instrument may close it alone.

## Full logs are retained

Every leg's complete combined output is kept at
`<workspace>/gate-logs/gates-runs/<workflow_id>/<gate>.log` (gitignored via
`gate-logs/.gitignore` — a stated home on the measuring box, not a committed
artifact; multi-megabyte cargo logs do not belong in history). Beside them,
`identity/<gate>.json` holds each leg's own capture of the tree and the
runner it measured (below). The manifest names the directory (`logs_dir`).
The 40-line tails in workflow history are for routing and reading; the
retained log is for the #118 class of question a tail cannot answer.

**The `gates_worker` leg rebuilds the worker crate's own `target/`** — the
crate is its own cargo workspace, so its build artifacts are separate from
the repository's. That directory (~3.8 GiB when warm) is recreated by the
first run of every battery, on both instruments. Under Tom's disk-headroom
ruling, `workflows/gates/worker/target/` is a named thing to clear when the
box is tight — and it is one more argument for the root-membership follow-up,
which would fold it into the one workspace `target/`.

## The manifest (the F2 ruling)

The run's event history is the instrument, not yet the record. The final leg
derives a manifest MECHANICALLY — verdicts only from `aion describe`, server
identity only from the server's own `GET /build` (with a 30s bound), the
measured checkout from `git` with every exit code checked:

- per-leg `{gate, activity_id, exit_code, passed}` in dispatch order, and one
  sha256 digest over the output tails (unit-separator-joined, same order);
- the run id and package content hash, read from the run's own
  `WorkflowStarted` event;
- **the instrument block**: the runner script's resolved path and sha256,
  every gate command (printed from the same arrays the runner executes), the
  resolved toolchain (`zsh`, `cargo`, `rustc`, `bun`, `jq`, `git`, `aion`,
  `curl`, `shasum` — each by path and version line), and the PATH it all
  resolved under — so the record names WHAT ran, per the estate's
  measured-claims-name-instrument-and-sha law;
- the workspace's `head_commit` and the runner's `runner_sha256` — **as the
  legs observed them**, not as the manifest leg could sample them (below);
- the tree's porcelain status **inlined into the manifest as lines**, plus
  the sha256 over the exact bytes git emitted, so the pinned hash has a
  preimage a reader actually has. Reconstruct it by joining the lines with
  `\n` and appending one trailing newline; an empty tree is `[]` and hashes
  the empty byte string. A line count cannot attribute and a failed git must
  refuse rather than count zero, so the record is the LINES and git's exit
  code is checked at every call;
- `derived_at` with an explicit UTC offset, and `logs_dir`.

**Untracked mode is `normal`, and the reason is measured.** `-uall` expands
untracked directories to every path beneath them: on this box that is 78,867
lines / 8.8 MB (measured 2026-08-11), which can be neither inlined nor
committed — so the earlier shape had to pin it by reference to a file under
`gate-logs/gates-runs/`, which `.gitignore` excludes, leaving the committed
record holding a hash whose preimage was never committed and is deleted with
the logs. `normal` names every dirty tracked file individually and every
untracked directory by its own path: 16 lines / 532 bytes on the same tree.
Attribution comes from recording the lines, not from expanding a snapshot
directory to eighty thousand paths.

**What was measured is captured at the leg, not sampled at the record.**
Every gate leg writes `gate-logs/gates-runs/<workflow_id>/identity/<gate>.json`
— the workspace HEAD and the runner's sha256, read *before* it runs its gate
command. The derivation binds itself to those observations: there must be one
per declared leg, they must agree with each other, and they must agree with
what the derivation itself reads. Any disagreement means the tree or the
instrument moved underneath the battery, and the derivation REFUSES rather
than recording the later of the two readings. This is not theoretical: the
lane's own r2a run had the 0.13.8 release land mid-run, moving HEAD
`712c8bcf4 → 4e1dec600` between the fmt leg and the manifest leg. The
manifest also carries `head_commit_at_derivation` and
`runner_sha256_at_derivation` beside the observed values, named as what they
are.

For a run whose legs left no observations — they predate the capture, or the
log directory was cleared — the derivation refuses unless the operator passes
`--unobserved-instrument-post-mortem`. That flag does not fill the gap in: it
records it. `head_commit` and `runner_sha256` are then `null`, only the
at-derivation readings appear, and `instrument.identity_capture.note` says so
in the artifact.

The workflow passes `expected_legs: 7` from its own document — the one place
the legs are declared — and the generator REFUSES a history holding any
other number of completed gates, in both directions, writing nothing. The
green outcome additionally requires `receipt.legs_recorded == 7` in the AWL
guard itself, so the participation predicate holds in the durable layer too.

**`status_at_derivation` is always `"Running"`** when the manifest is
written by the in-run leg — the leg runs inside the run, by construction.
The verdict fields are the leg table and `legs_failed`; terminal status is
confirmed against the run while it lives, and a post-mortem re-derivation
(below) shows the terminal status. A committed manifest is evidence the legs
it records completed as recorded — not that the run reached `Completed`.

The generator is also hand-invokable for post-mortem on a run that died
before its manifest leg:

```sh
zsh -f workflows/gates/gate-runner.zsh manifest <workflow-id> 127.0.0.1:50051 \
  http://127.0.0.1:8080 <out-path> 7 <workspace> \
  [--unobserved-instrument-post-mortem]
```

## Running it

Deploy the document, serve the queue, start a run — all by hand:

```sh
aion --endpoint 127.0.0.1:50051 deploy workflows/gates/gates.awl

cargo build --release --manifest-path workflows/gates/worker/Cargo.toml
workflows/gates/worker/target/release/lane-gates-worker \
  --address 127.0.0.1:50061 \
  --path "$HOME/.cargo/bin:/opt/homebrew/bin:/usr/bin:/bin" \
  --concurrency 4 \
  --reconnect-initial-backoff 1 --reconnect-max-backoff 30

aion --endpoint 127.0.0.1:50051 start gates --input '{
  "workspace_path": "/absolute/path/to/aion",
  "manifest_path": "/absolute/path/to/aion/gate-logs/<lane>/manifest.json",
  "server_grpc": "127.0.0.1:50051",
  "server_http": "http://127.0.0.1:8080"
}'
```

Every worker setting is named by the operator — the liminal address, the
PATH the gates resolve tools from (recorded into the manifest), concurrency,
and both redial backoffs; a missing flag is refused with every absent name
listed. The runner script's location is DERIVED from `workspace_path`
(`<workspace>/workflows/gates/gate-runner.zsh`), never accepted as an input:
the manifest pins those bytes by sha256.

The worker spawns the runner as `zsh -f` with the environment cleared to the
named PATH, so no host dotfile (`~/.zshenv` included) can widen the gate
environment or leak credentials into logs that get hashed into records.

**Wait out dispatch probation before starting a run.** The server's liveness
probe grants dispatch eligibility only after the worker answers two
consecutive liminal pings (a quarter-heartbeat-window cadence apart); a run
started before that fails terminally with "no liminal worker registered for
the row's pool" after the outbox's few-second retry budget. Start the
worker, then start runs a couple of probe rounds later.

Every command a leg runs lives in `gate-runner.zsh` — ONE committed,
reviewed place, executed and recorded from the same arrays — and the worker
crate is a thin containment adapter. Its contract test compiles the in-tree
`gates.awl` and diffs the advertised descriptors against the DOCUMENT's
derived contract — the same field-by-field comparison the server's admission
gate applies, run at the desk (admission against the deployed contract
happens at connect).

**Cancellation is real.** Every runner invocation runs under the SDK's
process-group containment: cancelling the activity (or the workflow)
SIGTERM-then-SIGKILLs the runner's whole process group — cargo and
everything under it — and reports a CANCELLED MEASUREMENT, never a result.

**Why a liminal SDK crate and not `aion worker awl`?** The server dispatches
a deployed AWL workflow's activities through its liminal outbox, and
dispatch ELIGIBILITY is earned by answering the liveness probe. When this
crate was written the probe enumerated only liminal connections, so a
gRPC-registered worker (`aion worker awl` / `worker shell`) could never be
asked a question and stayed dispatch-ineligible for the life of the process
(#197). That closed at fc98ea1b7 (0.14.0): the probe now reaches
gRPC-delivered workers, so both paths earn eligibility. This crate remains
the served worked example; the gRPC worker path is a peer, not a dead end.

**MANUAL START ONLY.** No cadence, no CI trigger, no schedule: a schedule is
a value the operator names, and none is named. The battery is heavy (~30-60
minutes); never run it concurrently with hand gates or other heavy work on
the same box.

**Slice two** (a remote box) is task-queue routing with ZERO workflow-shape
change: run the same worker binary on the other host with `--address`
pointed at this server and that host's own `--path`, and pass that host's
checkout paths as inputs.

## The parallel-run discipline (the F3 ruling)

This workflow runs IN PARALLEL with the hand battery
(`scripts/battery.sh`) until FIVE consecutive real lanes agree completely
over the shared legs. The two instruments do NOT run the same battery — the
delta is enumerated, gate by gate, in
[`docs/operations/lane-gates.md`](../../docs/operations/lane-gates.md), and
the agreement predicate is defined over the overlap only. ANY disagreement
on the overlap resets the count to zero and files a board item, regardless
of which side was wrong. Cut-over requires the enumerated delta to be either
absorbed into the workflow or separately ruled retired — never implied by a
count of five.

## What a green run does not prove

A green battery proves what these seven commands measure against the named
checkout, nothing more — and the checkout is pinned by the `head_commit` the
legs themselves observed plus the porcelain lines inlined in the manifest, so
read those before citing a run:
a battery run against a dirty tree is evidence about that dirty tree. It
does not run the fifteen house-battery gates outside the overlap (see the
delta table), does not review a diff, and does not replace the reviewer.
