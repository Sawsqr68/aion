# aion#35 — retire the libsql store backend: builder's report of record

(Final message of the Opus builder, forwarded verbatim by Waffles 2026-08-18
02:47Z — the lane's harness could not write REPORT.md, so this text IS the
report. Banked at the steward's gate before landing. Steward's gate notes
follow in GATE.md.)

---

Ruling addressed. Two of the three redesigned and green; the third is red by
decision with its blocker stated.

MY EARLIER VERDICT WAS WRONG, TWICE OVER: I reported all three as blocked on
concurrent store observation. Two problems were stacked and I only ever saw
the first. The prior lane switched the harness config from backend="libsql"
to haematite without adding store.node_cache_budget — so the child server
exited 2 on a config refusal and never reached a lock. I read the lock error
from a later leg and attributed all three failures to it. With the config
fixed, the lock is the real constraint, and it yields to relocation for two
of the three.

THE REDESIGN: run_server_outbox_happy_path_fan_out_completes_once — boot 1
drives the fan-out to staged-with-no-worker and drains; the bytes then say
four rows exist, none Done, no completions. Boot 2 dispatches and completes.
run_server_outbox_restart_rearms_stranded_rows — the stranded state is
asserted in the window between the kill and the re-boot. That window also
proves a property nothing else in the file stated: a row settles Done on
dispatch, not on completion — all four are Done while ordinals 2 and 3 have
recorded nothing. Live synchronization moved to POST /workflows/history with
payload elision disabled, so projected events decode back into
aion_core::Event and the same counting helpers serve both doors. The barrier
is Child::wait throughout — no sleeps. Added stop_gracefully() (SIGTERM +
wait, the repo's existing idiom); kept the kill where an abrupt death IS the
fixture. Controls: staging no outbox rows reddens the happy path; a no-op
rearm_outbox_pending reddens the restart test. Three consecutive clean runs
(flake-probe-run-server-outbox.log). One assertion could not survive
post-mortem — the happy path watched live for the dispatcher to claim while
no worker existed; claiming consumes no attempt and the drain re-arms a
claimed row to Pending, so nothing distinguishable outlives the process. It
moved to where the sweep is DRIVEN: outbox_dispatcher's retry pin now asserts
the sweep took the row to the dispatch seam (OutboxRowDispatch::dispatch
returns no-worker as an ordinary error).

THE ONE THAT STAYS RED: run_server_outbox_live_reconciliation_rearms_stranded_claims_once,
original single-boot form, because that form is the honest contract. I built
the between-servers version and it passed — then checked whether it measured
anything. It was vacuous: passes with the reconciler's guard mutated to
false, passes with reconciliation entirely unconfigured. The boot-time re-arm
settles the planted rows before the reconciler sweeps, so the green was
attributable to the wrong mechanism and duplicated the restart test. A stale
claim can't arise naturally on a live server either — only a death between
claim and settle leaves Claimed, and that death ends the server the test
needs alive. CAPABILITY REQUEST: a haematite concurrent seam whose second
handle can do ROUTED reads (aion co-locates an outbox row on its workflow's
shard; ReadOnlyDatabase routes by the key itself, so today's observer can't
read these rows at all), or a server-side seam that can age a claim in place.

SEPARATE FINDING FOR ITS OWN LANE — haematite lock leak: HaematiteStore::open
on a locked directory returns Err WITHOUT releasing the lock file it opened;
the directory is then poisoned for that process. Measured: 714 consecutive
failures where a single unprobed reopen succeeds; spawn_blocking-controlled,
unchanged. Against haematite 0.8.3. Not touched in this lane. [Steward note:
minted as haematite#16.]

BATTERY OF RECORD: Summary [429.091s] 5076 tests run: 5075 passed (7 slow,
1 leaky), 1 failed, 0 skipped. check and clippy contribute ZERO findings;
findings.jsonl (5656 bytes) is the single deliberate red. Feature legs
unchanged against the control at d4317268f — no finding absent from the base,
14 pre-existing eliminated. Survivors (canary planted, proven visible,
deleted): five crate files — config/sections.rs, config/load.rs,
config/env.rs, aion-cli/src/server.rs, all refusal machinery, plus
run_server_outbox_e2e.rs carrying the red test's capability paragraph. No
live claim survives. Worktree aion-wt-35-libsql, branch retire-libsql,
a7fd114ebd6444d83cf88c5ffc8d4959156be71d, clean. Not merged, not pushed.
