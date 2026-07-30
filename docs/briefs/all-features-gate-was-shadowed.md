# Brief: the all-features gate has never run, and one brittle test is why

Task #71, **corrected**. It was recorded as "`libsql-backend` has no gate". That
was wrong, and the truth is worse: the gate exists, and it has never executed.

## What is actually happening

`.github/workflows/ci.yml` job `test` (line 140) runs two steps, in this order,
**in the same job**:

```yaml
      - name: cargo test --workspace
        run: cargo test --workspace
      - name: cargo test --workspace --all-features
        run: cargo test --workspace --all-features
```

GitHub Actions stops a job at the first failing step. The second step is the
**only** thing in the whole workflow that compiles `libsql-backend` — or any
other non-default feature — so whenever the first step fails, an entire coverage
dimension is skipped and the job reports one failure rather than "coverage did not
run".

The first step has been failing. Run `30192212491` (2026-07-26, the most recent
run of any kind) shows it precisely:

```
X test (workspace, default + all-features)
  X cargo test --workspace
  - cargo test --workspace --all-features      ← never ran
```

with the cause:

```
test compile_tests::flagship_fixtures_compile_under_gleam ... FAILED
thread '…' panicked at crates/aion-awl/tests/emitter/compile.rs:154:5
error: Failure opening tar archive
test result: FAILED. 31 passed; 1 failed
```

That is the `gleam` CLI failing to unpack a fetched package on the runner — an
environment failure, not a code defect. **One brittle, network-dependent test in
one job has been shadowing every non-default-feature test in the repository.**

It also explains the four red `libsql-backend` tests that went 91 commits
unnoticed: nothing was broken about *noticing*, because the lane that looks never
got to look.

Two further facts a fix must not paper over:

- Every recent CI run is `failure` or `cancelled`. A permanently red pipeline
  teaches everyone to stop reading it, which is the second-order damage here and
  the reason this is worth a brief rather than a one-line patch.
- The most recent run is a **scheduled** one on 2026-07-26. Nothing has run since.
  Establish whether that is simply "nothing was pushed" or whether the schedule
  has also stopped firing — do not assume.

## 🔴 R0 — established 2026-07-30: GitHub Actions is DISABLED on this repository

The instruction above said do not assume, so it was checked. The answer is neither
option it offered:

```
$ gh api repos/ablative-io/aion/actions/permissions
{"enabled":false,"sha_pinning_required":false}
```

Actions is switched off at the repository level. Both workflows report `active`, and
that is a red herring — a workflow's own state is irrelevant while Actions is
disabled for the repo, because **no run is ever created, for any event**. Proof: 18
commits were pushed to `main` on 2026-07-30 and `gh run list` shows no run for any
of them; the newest run of any kind remains the 2026-07-26 schedule. The last
push-triggered runs were 2026-07-25 22:29Z, so Actions went off somewhere between
those two timestamps.

The repository is **public**, so Actions minutes are free and a billing cap is an
unlikely cause — this reads as a deliberate toggle. Whoever turned it off may have
had a reason, so **re-enabling is an owner decision, not a repair.** It is one API
call (`gh api -X PUT repos/ablative-io/aion/actions/permissions -f enabled=true`)
and must not be made on the owner's behalf.

**This supersedes the framing of R1 without invalidating its work.** R1 was real and
correct — the all-features step genuinely was sequenced behind a brittle test in a
shared job, and it is now its own job. But R1 fixed the wiring of a gate whose power
is off. Until Actions is re-enabled, **no CI lane in this repository can catch
anything**, including every lane added on 2026-07-30 (all-features test, all-features
clippy, `typescript-sdks`, `typescript-proto-drift`). Local batteries are currently
the only gate this project has.

The honest record of how this finding evolved, because the shape of the error is
instructive: first recorded as "`libsql-backend` has no gate" (wrong — one existed),
corrected to "the gate exists and has never run" (true, and the cause of the four
stale failures), now corrected again to "and nothing else runs either, because
Actions is off". **Each layer was found by checking the next assumption down instead
of stopping at a satisfying explanation.** The first two answers were both plausible
enough to have closed the investigation.

## R1 — the all-features gate becomes its own job

Split `cargo test --workspace --all-features` into a separate job so no failure
elsewhere can skip it. A separate job also runs it in parallel, which costs
nothing in wall-clock.

`continue-on-error` and `if: always()` are **not** the fix. They would let the
step run while leaving both results fused into one job status, so a red
all-features run could still be read as "that flaky gleam thing again". The point
is that non-default-feature coverage gets a status of its own that names itself.

Apply the same reasoning to the other multi-step jobs in this workflow: any job
whose later step covers something no other job covers has the same defect.
Enumerate them rather than fixing only this one.

## R2 — `flagship_fixtures_compile_under_gleam` stops depending on the network

The test shells out to real `gleam`, which fetches packages. The house rule is
clear about the shape of the fix: a test needing an external tool gates **at
runtime** — read the environment, emit a `tracing::info!` skip line, return
`Ok(())` — and never `#[ignore]`.

The existing gate is not wrong in kind (the workflow comment says "Gleam-dependent
tests gate-and-skip when the CLI is unavailable"), it is wrong in *what it checks*:
it gates on the CLI being **present**, and the failure mode here is a CLI that is
present and whose fetch fails. Distinguish the two:

- `gleam` absent → skip, loudly, as now.
- `gleam` present and the compile fails **for its own reasons** → fail. That is
  the signal the test exists for.
- `gleam` present and the *fetch* fails → this is the case with no honest answer
  yet. Decide it deliberately and write the decision down. Vendoring the fixture's
  dependencies so no fetch is needed is the option that keeps the test meaningful;
  treating a fetch failure as a skip makes the test able to pass while proving
  nothing, which is the failure mode this whole cycle has been about.

A fourth option, found by reading `compile_generated_modules` rather than reasoning
about it, and probably the right one: the generated project's `gleam.toml` pins
`aion_flow` by **path** but takes `gleam_stdlib = ">= 0.34.0 and < 2.0.0"` and
`gleam_json = ">= 2.0.0 and < 4.0.0"` from Hex, so **every run of this test resolves
and downloads two packages over the network**. A bounded retry on a
fetch-classified failure costs nothing in rigour — it does not weaken the assertion
at all, it only tolerates a transient registry — and it is far lighter than
vendoring. Combine it with classification so the final message names an
infrastructure failure as one, and nobody reads "Failure opening tar archive" as an
AWL emitter defect.

Whichever is chosen: `gleam build`'s exit status currently reaches a bare `assert!`
with stdout and stderr interpolated (`compile.rs:154`), so the failure text is
available to classify on — no new plumbing needed. Pick one, state why, and make
the skip path impossible to confuse with a pass.

## R3 — only then, the four libsql failures

Once R1 lands, the four `libsql-backend` failures become visible and can be fixed
against a gate that will hold. They are `HTTP 409 NO_QUEUE_DECLARATION`: the
committed Gleam-scaffold fixture (`aion_outbox_fixture`) declares unscoped
activities, and the no-grandfather enforcement from `c7c66992` correctly refuses
it. The error message states the remedy itself — "re-deploy from a checked AWL
contract" — so the fixture is what changes, not the enforcement.

Do **not** do R3 before R1. Fixing the tests while the gate is still shadowed just
restarts the 91-commit clock.

The fourth test has never been measured at all:

```
cargo nextest run -j 1 --no-default-features \
  --features "libsql-backend,liminal-transport" -p aion-server \
  -E 'test(production_boot_dispatches_executes_and_records_over_liminal)'
```

Run it and report what it actually does, rather than assuming it shares the other
three's cause.

## Acceptance

- The all-features run has its own job status, and a deliberately introduced
  failure in the default `--workspace` job leaves it still reporting.
- `flagship_fixtures_compile_under_gleam` cannot pass without having compiled
  something, and its skip path is distinguishable from its pass path in the log.
- All four `libsql-backend` tests green, under a lane that would have caught them.
- Every currently-red CI job is either green or has a named, tracked reason. A
  pipeline nobody reads is not a gate.

## The reusable lesson, for the record

A gate sequenced behind a brittle check is not a gate. Whenever coverage lives in
a later step of a shared job, its absence reports as someone else's failure — and
the thing it was watching drifts for as long as nobody separates the two.
