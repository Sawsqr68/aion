# Gates through Aion — the lane battery as a workflow

[`workflows/gates/`](../../workflows/gates/) holds a first-party Aion
workflow built to BECOME this repository's gate runner (#57 slice one). It
is **not the gate of record today**: it gates in parallel with the hand
battery under the F3 discipline below, and `scripts/battery.sh` remains the
house battery until cut-over is ruled. This page records the doctrine and
the exact delta between the two instruments, so neither can be retired by
implication.

## What runs, in one line

`aion start gates --input '{...}'` runs fmt, clippy twice (both feature
resolutions), the workspace test suite (with the estate's coverage-pairing
floor), the console's bun suite (nonzero-tests floor), the non-mutating
embed freshness check, and the battery's own worker crate against a named
checkout — each leg returning its command's real exit code as data. The
mechanics, the exact commands, and the serve instructions are in the
[workflow README](../../workflows/gates/README.md) — one home, not two.

## The manifest contract (F2, as ruled)

The run's event history is the INSTRUMENT, not yet the record. Every lane
that runs the battery through Aion commits a small manifest DERIVED
MECHANICALLY from the run's own history by the workflow's final leg —
per-leg exit codes, a digest of the tails, the run id, the server's own
build identity, the measured checkout's commit and porcelain status, and the
INSTRUMENT block (the runner script by path and sha256, every gate command,
the resolved toolchain and PATH — the estate's
measured-claims-name-instrument-and-sha law applied to gates). THAT file is
the load-bearing evidence; the run id inside it is corroboration while the
run's history lives. The generator is part of the workflow, runs on green
and on red, refuses a history whose completed-leg count differs from the
seven the document declares (both directions), and the AWL green guard
independently requires `legs_recorded == 7`. There is never a
hand-maintained second copy.

Two properties of that record are load-bearing and were bought at the r2
review's price:

- **The identity is captured at the leg, not sampled at the record.** Each
  gate leg writes the workspace HEAD and the runner's sha256 before it runs
  its command; the derivation binds itself to those observations and REFUSES
  when they disagree with each other or with what it reads at derivation
  time. Sampling in the manifest leg alone leaves a twelve-to-thirty-minute
  window in which the tree or the instrument can move — the lane's own r2a
  run had a release land mid-battery. A manifest that cannot say which tree
  it measured is not written; a post-mortem derivation over a run with no
  observations requires an explicit operator acknowledgment and then records
  the gap (`head_commit: null`) rather than filling it in.
- **Every hash the record pins has a preimage the record carries.** The
  porcelain status is INLINED as lines, not pinned by reference to a file in
  a gitignored directory — a committed hash whose preimage is never
  committed attributes exactly as poorly as the file count it replaced.
  Untracked mode is `normal` for a measured reason: `-uall` on a box with
  standing snapshot directories produces an 8.8 MB preimage that can be
  neither inlined nor committed, while `normal` names every dirty tracked
  file and every untracked directory in 532 bytes.

The scripts doing this derivation are themselves instruments and carry their
own defect class: see `docs/operations/shell-instrument-traps.md` — the
family reference for shell code that fails silently into a plausible value
(the `status` and `path` zsh-special shadowing traps this lane paid for at
r1 and r2), and the sweep that checks a new script against the class.

`status_at_derivation` in a manifest written by the in-run leg is always
`"Running"` — the leg runs inside the run, by construction. The verdict
fields are the leg table and `legs_failed`; a committed manifest is evidence
the recorded legs completed as recorded, not that the run reached
`Completed`. Terminal status is confirmed against the run while it lives, or
by a post-mortem re-derivation.

## The two instruments are NOT the same battery (the B4 record)

The workflow's seven legs against `scripts/battery.sh`. Shared legs — the
F3 comparison surface — first:

| Workflow leg | House battery gate | Note |
|---|---|---|
| `fmt` | `rust:fmt` | identical command |
| `clippy_workspace` | `rust:clippy` | identical command |
| `test` | `rust:test` | identical command; both instruments carry the target/result pairing control — and both carry its docketed blind spot below |
| `bun` | `console:test` | identical command |
| `gates_worker` | `gates:worker-crate` | added to BOTH instruments in the r1 discharge — the battery gates its own instrument. Its three checks (fmt, clippy, test) run on BOTH instruments without short-circuiting: the exit code is the first failure's, but every check answers, because `&&`-chaining hides two diagnoses behind the first red |

Workflow-only legs (the hand battery does not run them): `clippy_server`
(the preserve_order-OFF resolution of the server crate), `embed` (the
non-mutating verify-ops-console decomposition).

**House-battery gates the workflow does NOT run — fifteen, enumerated.**
Each exists for a written reason (`scripts/battery.sh`'s own comments);
none may be retired by implication:

| Missing gate | Why it exists |
|---|---|
| `rust:test-all-features` | a genuinely different feature resolution, not a slower copy |
| `posture:worker-default` | #119 build-posture control (liminal transport OFF arm) |
| `posture:worker-liminal` | #119 (liminal transport ON arm) |
| `posture:proto-default` | #119 (generated stubs OFF arm) |
| `posture:proto-generated` | #119 (generated stubs ON arm) |
| `posture:serde-json-sorted` | the #76 preserve_order split, OFF arm pinned by name |
| `posture:serde-json-insertion` | #76, ON arm pinned by name |
| `rust:preserve-order-pins` | the ordering-sensitive pins under the non-vacuous resolution |
| `gates:costly-announced` | skipped costly proofs must announce themselves (#86) |
| `console:typecheck` | the console's tsc gate |
| `python:aion-worker` | proto-drift remediation for the wire-dead-SDK incident |
| `python:aion-client` | ditto |
| `ts-worker:proto-drift` | ditto — the direct remediation gate |
| `ts-worker:typecheck` | TS worker SDK typecheck |
| `ts-worker:test` | TS worker SDK suite |

## The parallel-run discipline (F3, as ruled)

The workflow gates in PARALLEL with the hand battery until FIVE consecutive
real lanes agree completely **over the shared legs in the table above** —
the agreement predicate is defined over that overlap only, because the two
instruments measure different sets outside it. Any disagreement on the
overlap resets the count AND files a board item, regardless of which side
was wrong — a disagreement is evidence about the instruments. **Cut-over
additionally requires the fifteen-gate delta above to be either absorbed
into the workflow (a named follow-up slice) or separately ruled retired,
gate by gate, by Waffles — five agreements license nothing about gates the
workflow never ran.** Cut-over happens on the coordinator's word.

## Manual start only

No cadence, no CI trigger, no schedule. A schedule is a value the operator
names, and none is named here.

## The design point: per-leg exit codes are the participation predicate

The estate law that a verdict-less death cannot vanish into a summed tally
(#195's shape) applies to gates as: EVERY LEG REPORTS ITS OWN EXIT CODE, and
the battery's verdict is computed over those recorded verdicts plus the
recorded leg COUNT (`legs_recorded == 7` in the green guard), never over a
sum. A leg that dies without a verdict fails its activity loudly and the run
fails with it. Inside the `test` and `bun` legs, coverage floors turn an
exit code over nothing into a RED leg with a named reason — an exit code is
a claim about failure, not about coverage (the #118/#120 record).

Three distinctions the reader of a manifest needs:

- A floor VIOLATION is a red leg. A floor that could not be EVALUATED is a
  broken measurement — a terminal activity failure, not a verdict about the
  code.
- The floor is applied only to a leg whose command exited 0, because the
  dangerous direction is the vacuous green. So a **RED `test` leg carries no
  coverage verdict**: `exit_code: 101` there does not mean "a test failed",
  and distinguishing a failure from a truncation is a hand pairing check over
  the retained full log (that is how r2a's `outbox_e2e` SIGSEGV was
  confirmed).
- A gate whose own tool or cargo SUBCOMMAND is missing is a broken
  measurement, never a red leg. `cargo <missing-subcommand>` exits 101 — the
  same code as a real clippy finding — so the runner asks each subcommand for
  its `--version` before the leg rather than reading 101 as a verdict.

## Docketed: the pairing control is blind to a target that runs zero tests

**Open against BOTH instruments jointly — `scripts/battery.sh paired_test`
and `workflows/gates/gate-runner.zsh coverage_floor_cargo_test`. Neither may
close it alone, and no lane may quietly change one of them.**

The control asserts PAIRING: every started target reports a result before the
next target starts, plus a nonzero target count. A target that starts and
prints `running 0 tests` / `test result: ok. 0 passed` is perfectly paired.
So `targets=1, unpaired=0`, the floor holds, and a run that executed **zero
tests** reports green. Measured live in this lane's own synthetic red run
`1268ede0-63d1-41c3-b25a-da2a7dc0a679`, whose `test` leg is exactly that:

```
gate=test exit=0 passed=true
     Running unittests src/main.rs (...)
running 0 tests
test result: ok. 0 passed; 0 failed; ...
```

This is not a regression and not a deviation introduced by the workflow: the
port of the house control is faithful, and the house control has the same
hole. It is a live gap in the estate's shared instrument, brought to parity
rather than closed.

**The naive fix has a trap that is already written down, and the docket
carries it so the next hand does not walk into it:** a per-TARGET
nonzero-tests rule false-fires, because many crates legitimately have 0-test
targets (a binary target with no `#[test]`s is normal, not a defect); and
summing verdict lines across targets is the heterogeneous-rows error
`scripts/battery.sh:186-200` warns about in its own comments. The right shape
is a RUN-level floor — the analogue of what the `bun` leg already has in
bun's own `Ran N tests` line — designed with both traps named, and landed on
both instruments in one pass so the F3 comparison stays leg-for-leg.

## No tree-mutating gates

Every leg is read-only against the checkout it measures — this is why the
embed leg decomposes `cargo xtask verify-ops-console` instead of invoking
it. The workflow writes exactly two things, and both are deliverables, not
gates: the manifest, and the retained per-run logs under
`gate-logs/gates-runs/` (gitignored; the stated, uncommitted home on the
measuring box).

## Known gap (#197, re-filed with the measured mechanism)

`aion worker awl` and `aion worker shell` register over gRPC into the SAME
worker registry the liminal outbox dispatcher selects from, with the correct
pool key — and are still never dispatched to, because dispatch ELIGIBILITY
is a verdict of the liminal liveness probe: eligibility requires two
consecutive answered liminal pings, the probe pings only liminal
connections, and so a gRPC-delivered worker's opening probation can never be
served. The selection miss is reported by a catch-all message ("no liminal
worker registered for the row's pool") that does not distinguish an empty
pool from an eligibility exclusion, and its `namespace: "liminal"` field is
a hardcoded diagnostic label, not the row's namespace. A corollary that
applies to LIMINAL workers too: a run started before a fresh worker's
probation clears (two ping rounds) fails terminally after the outbox's
few-second retry budget — start the worker, then start runs a couple of
probe rounds later. Until #197 closes, the queue is served by the liminal
SDK worker crate in `workflows/gates/worker/`.
