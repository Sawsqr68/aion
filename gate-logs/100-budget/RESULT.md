# aion#100 — gate result

**Commit gated:** `b767a1ead58d0ef4c07b8d2cd679247179c1346e`
**Parent:** `8e0cb3f3877d5fad02229877e7862c37868e3aaf` (origin/main at arming, the #108 merge)
**Branch:** `fix/100-transport-budget-wiring`
**Log:** `battery-20260821.log` · **Venue:** `VENUE-NOTES.md`

## Declaration control

```
$ bash scripts/gates/verify-declaration.sh gate-logs/100-budget/battery-20260821.log
declared : 24 legs
executed : 24 legs
VERDICT: exact match — all 24 declared legs executed, and nothing else.
```

Its own caveat is respected here: that is a statement that every leg **ran**, not
that any leg tests what it is named after.

**HEAD was `b767a1ead` at launch and verified unchanged at finish** — moving the
ref alone is enough to void a run, so it is checked rather than assumed.

## Verdict: 23 legs green, 1 red

`rust:test-all-features` exited 101 on a single test:
`one_motion_live_e2e::one_motion_run_compiles_deploys_starts_awaits_and_reports_the_result`,
failing with `server exited during boot with exit status: 1` and no further output.
The other four tests in that binary, which also boot servers, passed.

### The two #100 pins ran and passed, by name

```
Running unittests src/lib.rs (target/debug/deps/aion_server-fae814998b4b3913)
running 1172 tests
test worker::bridge::tests::a_dispatcher_carries_the_transport_budget_of_the_tracker_it_was_given ... ok
test worker::bridge::tests::a_sink_derives_its_transport_budget_from_the_operator_window ... ok
```

The `running 1172 tests` line is part of the evidence: a block that runs zero
tests also prints `ok`.

## The red is not attributable to this commit

Four independent measurements, in the order they were taken:

1. **A within-battery control.** The same test **passed** in this battery's
   `rust:test` leg (log line 1885) — same commit, same box, same live server,
   minutes earlier. Same source, two runs, two answers.
2. **Prior batteries.** It passed in *both* legs of
   `gate-logs/83-list-runs/battery-20260820.log` and
   `gate-logs/241-remediation/final-battery.log`, each with
   `rust:test-all-features exit 0`.
3. **No mechanism.** The diff touches `worker_grpc.rs`, `state.rs`, `bridge.rs`,
   `transport_loss.rs` and three test files. A boot failure precedes any activity
   dispatch; nothing in the diff is on that path.
4. **Re-run in the death's own world — the decisive one.** Rebuilt under the
   gate's own resolution (`cargo test --workspace --all-features`) and confirmed
   by the binary's identity: phase 3 produced
   `one_motion_live_e2e-9936d71fcd3f58fb`, **byte-for-byte the unit named in the
   battery log**. Run 12× with the preceding binary (`new_scaffold_build`, the
   same hash the battery ran) looping as a load generator, driving load to
   **20.5** against the battery's 12–13.
   **12 runs, 12 named the target, 12 ok, 0 failed — siblings included.**

## What this result does NOT say

🔴 **It is not called a flake.** `grep -rl "…FAILED" gate-logs/` over every gate
log on this box returns **empty**: this test has no recorded prior failure.
"Flake" is a claim about a base rate, and there is no base rate to cite.

The honest statement is narrower and is the one that stands: **the failure is
unexplained and did not reproduce.** It is filed as open, not closed.

An earlier, larger claim was retired by measurement and is recorded so it is not
re-derived: 128 green runs (branch 24+20, parent 24+20) were collected under
`-p aion-cli --all-features` before the binary hashes showed that resolution
builds a **different unit** (`f939508da8191bf5`). Those runs are kept as record
but are not evidence about the program that died.

## The harness defect found on the way — fixed separately, on purpose

`wait_for_boot` reported only an exit status while both handles were
`Stdio::piped()`, so the server's dying words sat unread in the pipe. The prior
instance of this class in `gate-logs/160-tutorial-worker/cargo-tests.log` named
its cause in one line (`Address already in use (os error 48)`); this one could
not, which is why the red above is unexplained rather than merely uncaused.

The fix drains both handles on both failure paths, reaps before draining (EOF
needs the write end closed), and distinguishes the three outcomes an empty string
collapses: `<not piped>`, `<unreadable: …>`, and "wrote nothing". It is held as
`instrument-wait-for-boot.patch` and **deliberately not committed here** — a new
commit on this branch would void the attestation this file records. It lands on
its own, so the next occurrence names its own cause.

## Housekeeping during the run

- The `gates:worker-crate` leg dirties `workflows/gates/worker/Cargo.lock`;
  restored with `git checkout --` before this record was written.
- A disk alarm fired at 18 GiB mid-run. Cleared by removing
  `.worktrees/69-redial/target` (21G, build artifact only) after confirming all
  31 pid files dead, no uncommitted tracked files, all three commits present, and
  no live process under that path. Source verified intact afterwards. Free
  returned to 39 GiB. Two live lanes not mine were left untouched at any pressure.
