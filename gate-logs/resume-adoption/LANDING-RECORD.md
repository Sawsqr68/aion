# Landing record — resume-adoption lane 1 (OPEN-IN-PART)

- **Status**: DRAFT, executes on battery green (pid 50968 re-run; first run's
  test leg was an ENOSPC non-run — see Battery below). Landing authority:
  Waffles' standing ruling 07:20Z, executed at Vesper's hand under the
  end-to-end handover (Tom's word via Waffles 07:55Z).
- **What lands**: 12 commits `168baa5f4..e05d3e3b4`, fast-forward of aion
  main (merge-base verified = origin/main 168baa5f4, v0.17.0), plus this
  record. The twelfth (`e05d3e3b4`) is the stale-pin correction described
  under Battery below — authored at Vesper's hand, not the builder's. Built by the resume-adoption norn builder (session ad933f99,
  pid 10147, exited 07:15:34Z); collected manually — the run's harness
  record was disowned by a mid-lane server generation, itself an instance
  of the defect family this lane addresses (aion issue #36).

## What this landing claims — sized exactly to its evidence

**Closed WITH evidence (the observed-incident face):** every incident in the
2026-08-17 family had the run's re-dispatch arrive at the SAME live worker
(the worker never died; only the server cycled). That face is closed by the
same-execution single-flight join (`agent_flights.rs` + liminal wiring),
which sits before the spawn gate — a redelivery joins the leader execution's
one outcome. Unit-proven; behavioral red/green at the harness boundary for
the spawn-gate self-adopt + foreign-refuse pair (74c249d8c → 85755d737 —
foreign refusal stays red, #246's control preserved).

**Closed by construction only (the worker-replacement face):** a replacement
worker re-dispatching into a surviving agent enters the spawn-gate adoption
path (same workflow/activity → Norn resume-if-exists invocation), but this
arm's acceptance evidence is exactly the explicit non-claim in the builder's
delivery report. Evidence is owed on board row 1 below.

**Also landing:** exhaustion records name exhaustion (behavioral red
836ef5a88 → b73246ddc); Paused histories with dangling attempts enter the
recovery sweep (6540c7c29 → d02da35a2); the 400ms false-error resume
deadline is removed — resume waits the recovery fence and errors only if
the recovered process dies (e350538ad → bbce645ac).

**The class proof is OPEN.** This landing does not claim the #36 truth fix:
no proof exists that `superseded:server-death` is absent for an adopted live
holder, no adoption event is recorded, the Running-path recovery still
re-dispatches, and the #36-verbatim e2e has never run.

## Board rows (verbatim, per the ruling)

1. Cross-process re-attach arm — worker killed under a live agent;
   replacement worker resumes the Norn session; session-ID continuity
   proves it (explicit non-claim in lane 1).
2. Issue #36 verbatim e2e — pause → restart → resume completes with one
   session, on an isolated in-worktree server.
3. Supersession-absence proof + adoption event — history stays truthful
   while the holder lives, on Paused AND Running paths.
4. Running-path truth fix — recovery offers dangling remote-agent attempts
   to adoption for Running as for Paused.
5. Readiness-pin hardening — the current pin asserts a constant helper and
   sits on no mutation path; a decorative test is worse than none.

All five are legs of the follow-on lane (FOLLOW-ON-BRIEF-DRAFT.md).

## Battery

- Run 1 (Waffles' hand, pid 16829): fmt rc=0, clippy rc=0 (full workspace
  `--all-targets -- -D warnings` — the whole tree, all test targets,
  compiles clean under strict lints), test rc=101 = NON-RUN: ENOSPC artifact
  (zero suite results; ld errno=28 / rustc-LLVM IO failure; cause = estate
  server's malloc log writing ~3GiB/min to an unlinked file, box bottomed
  at 924MB). The three compile-error reds in the chain (E0425/E0432) are
  thereby cleared of masking breakage — they were honest pre-fix reds.
- Run 2 (pid 50968, cold build after 112G target sweep, fired 07:48Z):
  rc=101 at 08:04:42Z, a REAL red — one failing test, zero ENOSPC, 105G free.
  **Diagnosed as expectation drift, not an acceptance miss.**
  `exhausted_retries_fail_the_run_and_reopen_continues_the_trail`
  (activity_retry_e2e.rs, assertion at :247) demanded
  `error.message == "retryable:upstream stream reset (delivery attempt 3)"`
  on a record of kind `Terminal` — which IS the defect the 04:40Z ruling
  ordered fixed, and which `b73246ddc` deliberately replaced with
  `"exhausted: spent 3 of 3 attempts; last refusal: retryable:upstream
  stream reset (delivery attempt 3)"`. The run's own history bytes in the
  gate log carry the new wording end to end: the fix works, the pin was
  stale. Sweep of all five `message == "retryable:` sites by verdict rather
  than by shape: await_paths.rs:200 PASSED at the gate (no-retry-policy arm,
  verbatim by design — not stale) and retry_outcomes.rs:39 is a mid-retry
  record (fine); the other three were stale.
- **A second finding, and the more serious one.** The collect script's test
  leg was `cargo test --workspace` with no `--no-fail-fast`, so cargo
  stopped at the first failing binary and `advisory_class_e2e` NEVER RAN —
  its 120 `Running` lines are absent from the log. Run solo it was red too,
  on two more pins of the identical old wording (:293 and :405). Unrun is
  not green. Both were invisible in a run that reported exactly one failure.
- Fix: `e05d3e3b4` updates all three pins to the new exact wording, equality
  strength unchanged. Solo verification at Vesper's hand after the fix:
  activity_retry_e2e 11 passed / 0 failed, advisory_class_e2e 3 passed /
  0 failed.
- Battery replaced for run 3 (Tom's ruling 2026-08-17): `cargo fmt --check`
  STRUCK — autoformat only, a format gate proves nothing. Test leg is now
  `cargo nextest run`, which runs every binary by construction and so
  retires the truncation class that hid the two advisory reds. Stages are
  `cargo check` → `cargo clippy` → `nextest`, each `--keep-going` /
  `--no-fail-fast` and `--message-format=json`, each jq-reduced to one
  finding per line in `FINDINGS.jsonl`.
- The extractor's own channel was proven before any verdict was read from
  it: `libtest-json-plus` is experimental, so a throwaway crate outside the
  worktree with one passing and one deliberately failing test was run
  through the exact battery invocation, and the filter returned the failing
  test with its panic text. An extractor whose silence has never been
  distinguished from its success is not an instrument.
- Run 3 (08:47–08:51Z): check 0, clippy 0, then the nextest runner died by
  SIGTERM with 4262/5085 tests unrun — **Tom's machine restart at ~08:52Z**
  (announced 08:44Z, boot 08:52, guard exonerated by its own event log). The
  24 reds it did record were all one cause: repo.hex.pm unreachable during
  the pre-restart degradation window — all 24 had passed at 08:26Z on the
  identical tree. Environmental, both halves; no code verdict.
- **Run 4 (09:31–09:39Z, post-restart quiet box, runner setsid-detached):
  GREEN — the single-run verdict this landing gates on.** check rc=0,
  clippy rc=0 (`--workspace --all-targets -- -D warnings`), nextest
  **5085/5085 passed, 0 skipped** (7 slow), findings extraction empty.
  One run, whole workspace, no stitching.
- Landed 2026-08-17 ~09:45Z: `git merge --ff-only e05d3e3b4` on main,
  pushed to origin (`168baa5f4..e05d3e3b4`). Executed by Vesper under the
  07:55Z formal handover.

## Post-landing actions

- Battery target cleaned (box owes Tom disk).
- Binary reinstall + server cycle decision: made at landing on its merits —
  the eleven commits change worker/engine paths exercised by the live
  estate, but the live server's guard cadence under console load argues
  against an elective cycle while Tom is driving; decision recorded here.
- Worktree disposition: branch retained until the follow-on lane opens
  (fresh worktree from landed main); runtime marker
  `.aion-held-execution.json` is NEVER committed.
