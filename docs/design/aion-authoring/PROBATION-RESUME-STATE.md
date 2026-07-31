# PROBATION — LANDED. Record, costs, and what is next.

**Status: done, gated, committed `bfe3941d`.** Supersedes the resume state that
lived at this path; the OWED list it carried is discharged in full.

## What landed

Waffles' probationary re-admission ruling, in two commits:

- `33901163` — the tracker half. `Reachability { consecutive_answers, proved_at }`
  replaces a bare `Instant`; `register_connection` opens an UNSERVED probation
  instead of seeding eligibility; `DISPATCH_PROBATION_PINGS = 2` (K is mine, the
  rationale is on the constant).
- `bfe3941d` — **the half that made it real.** The reset path existed and was
  never called. Both failure arms of `LivenessProbe::apply_answer` now call
  `record_dispatch_unreachable`, so the counter is CONSECUTIVE rather than
  cumulative.

## 🔴 The finding worth keeping

**A whole feature was dead at the one seam its unit tests did not cross.** All
eight tracker tests were green against a probe that never reset anything,
because they drive the tracker DIRECTLY. The probation would have counted up
forever and a link answering one probe in three would have accrued eligibility
and flapped — exactly the defect it was built to stop.

The cure is `every_failed_probe_class_withdraws_dispatch_eligibility`, which
goes through `apply_answer` rather than the tracker, plus its success-path
control. **Watched failing both ways:**

| mutation | test that fails |
|---|---|
| probe drops the reset | `every_failed_probe_class_...` on its own assertion, first class |
| counter counts cumulatively | `a_link_that_answers_every_other_probe_never_becomes_eligible` at probe 2 |

## Behaviour change an operator will notice

**A newly connected worker is undispatchable until it has answered K probes** —
up to two probe cadences, i.e. **half a heartbeat window** (15s on a 30s
window). Dispatches in that gap PARK and are placed once eligibility is earned;
they are not failed. Evidenced by the real-server e2es passing, including the
95-second fan-out. This is the price of the ruling and it is deliberate: the
handshake ack is SENT and nothing reports it RECEIVED, so connection is a
channel, not proof the channel carries.

Withdrawal on a failed probe is now IMMEDIATE, not deferred to window expiry.
Both WARN lines were rewritten because they claimed the old timing.

## Gates

`cargo clippy -p aion-worker -p aion-server --all-targets --all-features -- -D warnings` → **0**
`cargo test` same scope `--no-fail-fast` → **1024 passed / 0 failed**, 45 targets,
48 result lines, exit 0. (Baseline 1019; +5 tests, all new.)

## 🔴 NEXT, and the constraint on it

Waffles' `5c433ade` close needs the shakedown green re-run. **The house battery
cannot run tonight without breaching my own 45 GiB free floor**, and this is
arithmetic, not a guess:

```
free now ~68 GiB, target/debug 22 GiB
house battery peak = ~46 GiB of target from clean (INFERRED from two
  clean halves: 22.77 default + 23.35 all-features, both measured tonight).
  The 55 that justified the split was a tree holding THREE build
  configurations at once and is not comparable — see below.
  reclaim first: free 90 - 55 = 35   ← below floor
  keep artifacts: free 68 - 33 = 35  ← same endpoint, same breach
```

**Plan: split it into two declared blocks with a reclaim between them** —
`--workspace` (default features) alone, reclaim, then `--workspace
--all-features` alone. Each is ~34 GiB, so free stays ~56 throughout. It costs a
full recompile between halves and buys never going under the floor. Declare each
half to Athena separately.

## Disk, measured — why my estimates keep coming in low

Predicted +5 to +8 GiB for this two-package block; **actual +15.8 and climbing**
against Athena's 13:00:31Z baseline of 6.15. Low by ~2×, the second miss in a
row in the same direction. The mechanism, measured rather than reasoned:

```
two-package --all-targets --all-features, at peak:
  linked test executables   4.33 GiB over  75 files
workspace, DEFAULT features, no --all-targets, at peak:
  linked test executables  10.09 GiB over 221 files   (verified at two seats)
```

🔴 **CORRECTED TWICE. `--all-targets` is NOT the multiplier and neither is a
second feature resolution — the multiplier is the TARGET COUNT ACROSS THE
WORKSPACE.** `cargo test` already links every lib-test and integration target;
`--all-targets` only adds benches and examples on top. Each binary carries the
whole crate graph plus debuginfo (largest 223 MB, mean 47 MB over 221).

Both wrong stories took a REAL measurement and bound it to the wrong variable.
**A mechanism claim needs a second data point that VARIES the proposed cause.**
Class estimate for `-p a -p b --all-targets --all-features` here: **+12 to +18
GiB**. For `--workspace` default features: **+20 to +26**, measured at +22.77.

## Shakedown re-run — SPLIT INTO TWO BLOCKS

The full house battery cannot run without breaching the free floor by either
route (arithmetic in the disk section below), so it runs as two declared
blocks with a reclaim between them.

```
BLOCK A  cargo test --workspace  (DEFAULT features, --no-fail-fast)
  exit 0 · 240 targets · 243 result lines · 3902 passed / 0 failed
  disk peak +22.77 against a 90.36 free baseline — inside the declared +20..+26
  log: .evidence/118/shakedown-default.log

BLOCK B  cargo test --workspace --all-features --no-fail-fast
  exit 0 · 240 targets · 243 result lines · 3889 passed / 0 failed
  disk +23.07 from a cleaned tree (watch's hand); free floor 64.93, never
  within 14.9 GiB of the shared 50 band
  log: .evidence/118/shakedown-allfeatures.log

⇒ SHAKEDOWN GREEN ACROSS BOTH FEATURE RESOLUTIONS. 480 target-runs,
  7791 test results, 0 failures, both exits 0.
```

🔴 **A green Block A is NOT evidence about Block B.** Different feature
resolution, different artifact set, and the four tests that were red on main
this morning ONLY COMPILE under `--all-features`. Reporting "the house battery
is green" off A alone would be reading a partial run as a complete one — the
same defect as #120 one level up.

**Count TARGETS, not tests.** 240 target-runs is a denominator; 3902 passing
tests is not.

🔴 And the denominator itself needs its noun stated, because I got it wrong in
the sentence that teaches this: **240 is 220 `Running` lines PLUS 20
`Doc-tests` lines**, not 240 `Running` lines. Right value, wrong noun, in the
one place claiming the noun matters. Doctests are a separate target class that
cargo runs LAST, after every integration target — so a mid-run read of a log
can show zero of them and be correct about that instant while saying nothing
about the battery.

## 🔴 Disk laws learned tonight, all measured

- **The free floor is a BAND, not a line.** Free swings ~1.5 GiB in both
  directions with the tree provably still, and that is a lower bound from one
  quiet window. An abort set at the edge of an unmeasured swing fires late, so
  the abort is **50**, which is also the watch's wake band — both seats then
  move on the same reading.
- **`du -sh` per sibling is not a measurement of that sibling.** cargo
  hardlinks `.rcgu.o` objects between `incremental/` and `deps/`: 32,266 of
  35,422 files in `deps` have link count > 1, and the 4.13 GiB of
  multiply-linked bytes belongs to BOTH. `deps` reads 18G alone and 13G inside
  a `du -h -d 1` walk; `incremental` reads 8.4G alone and 4.2G when its parent
  is in the same argument list. **Bank the TOTAL — it is invocation-independent
  — and take any split from ONE `du -h -d 1` walk.**
- **A reclaim estimate from `du -sh` on a hardlinked tree is an upper bound.**
  It is exact only when the deleted path contains every link to its own bytes.
  `rm -rf target/debug` returned the full 23G (67 → 90); deleting
  `incremental/` alone would return ~4.2 of its reported 8.4.
- **A measurement cited in an argument must be written to a file before the
  thing it measured can change** — and a reclaim is a scheduled, self-inflicted
  change. Evidence + the exact COMMANDS are banked in
  `.evidence/118/build-tree-decomposition-default-features.txt`; a banked result
  without its command cannot distinguish a different METHOD from a different
  WORLD.
- **A timestamp is the one field that travels with the data while being produced
  by something other than the measurement** — so it is the field most likely to
  be authored rather than read. Take stamps from `date -u`.

## Board

Closed today: **#58**, **#116**, **#118**, battery half of **#120**.
Open: **#117**, **#119**, CI half of **#120**.
**Awaiting Tom (HOLD OPEN, #88):** is Gleam a supported authoring surface, or is
AWL the only one.

## Standing commitments — mine to keep

- **Declare a BLOCK, not a battery.** Anything that invokes a compiler in this
  tree, including a build appended to an edit command. That is how the last one
  went undeclared: it fell through my own taxonomy, and free does not care what
  I call it.
- Watch free during it. **Abort at 50, not 45** — the floor is a band at
  least 1.5 GiB wide, and a threshold at its edge fires late.
- Report the SETTLED number against the watcher's baseline, not my own remembered
  one.
- `target/release` is NEVER touched — Tom's live server `99844` runs from a
  deleted binary in there (#108).

## Lessons that must not be re-learned

- **A feature can be inert with every unit test green.** Test the seam that
  wires it, not only the unit that implements it.
- **A measurement reported outside the scope it was taken in is an estimate
  wearing a receipt** — and the scope of a column is itself a measurement, not a
  reading of its label.
- **A total that agrees with reality while its components disagree will fail the
  moment the components are used separately.** Steady/spike uses them separately
  by construction.
- **A count is only a coverage claim if the run reached the end** (#120).
- **A red baseline is a blindfold, not a footnote.**


## 🔴 THE 55 DOES NOT SURVIVE MEASUREMENT — and the split was right for the wrong number

Tonight's clean halves, each measured from `rm -rf target/debug`:

```
Block A  cargo test --workspace                  = 22.77 GiB
Block B  cargo test --workspace --all-features   = 23.35 GiB
house battery (both resolutions coexisting)      ≈ 46 GiB   ← INFERRED, not measured
```

The 55 came from a tree built by THREE different commands, cumulatively,
recovered from the session transcript rather than from memory:

```
12:31  rm -rf target/debug && cargo test -p aion-worker -p aion-server \
         --all-targets --all-features --no-run          ← clean, but two-package AND --all-targets
12:41  cargo test --workspace --all-features --no-run   ← ON TOP of the above
12:44  cargo test --workspace --no-run                  ← ON TOP of both
```

Three reasons it is not the house battery's footprint:

1. **It was never clean.** The workspace builds ran on a tree already holding
   the two-package artifacts.
2. **It includes `--all-targets`**, which the house battery does not use.
   Benches and examples are in that tree and in no gate.
3. The `du -sh target target/debug/deps` form has the parent/child trap — though
   here the TOTAL is unaffected, because `target` is the first argument and is
   walked before anything can claim its bytes. Only the `deps` sub-figure was corrupted.

**Consequence, and it does not reverse the split but it does re-price it:**
90 − 46 = **43.9**, under the 45 floor by 1.1 GiB — which is INSIDE the 1.52
noise band. So the unsplit battery would have finished AT the floor, a
coin-flip, not the 10 GiB breach the 55 implied.

★ **Being right is not the same as having been right for the reasons given.**
The next person planning against "the house battery needs 55" would
over-reserve by 9 GiB and split work that did not need splitting.

**Both figures must be labelled: 46 is INFERRED from two clean halves; 55 is
UNVERIFIABLE (that tree is gone). Neither is a measurement of the house
battery, and neither should be quoted as one.** What would settle it is one
clean full-battery run measured with `du -h -d 1` — cheap, and not tonight.


## 🔴 The two resolutions are NOT nested — 4 distinct tests each way

Block A ran 3902 tests and Block B 3889 on the same 240 targets. The 13-test
gap is real but it is a gap in EXECUTIONS, not in coverage. Resolved from the
test NAMES in both logs, not from counting `#[test]` attributes:

```
DISTINCT test functions that run ONLY under DEFAULT features   = 4
  deploy_commands.rs        deploy_subcommands_drive_a_live_server
  deploy_audit_log.rs       deploy_mutations_emit_structured_audit_lines
  deploy_api_e2e.rs         deploy_metrics_count_operations_and_denials
  worker_dispatch_delivery  denied_registration_fails_rpc_without_frames

DISTINCT test functions that run ONLY under --all-features     = 4
  run_server_outbox_e2e     run_server_child_process
                            run_server_outbox_happy_path_fan_out_completes_once
                            run_server_outbox_live_reconciliation_rearms_stranded_claims_once
                            run_server_outbox_restart_rearms_stranded_rows
```

The four default-only tests carry `#![cfg(not(feature = "auth"))]`, documented
at the file: they need dev-token mode, where `jwks_url` is a shared secret.
With `auth` compiled in it is a real JWKS endpoint fetched at boot, so boot
fails before any deploy mechanic runs. The four all-features-only tests are the
outbox suite fixed in `2460103d`, which only compiles there.

### 🔴 The other 14 of the "18" were an artifact of my own counting

`#[path = "test_support/gleam.rs"] mod gleam_test_support;` pulls a SEVEN-test
module into every including target, and integration targets do not set
`cfg(test)`, so those seven compile and RUN once per including target:

```
under DEFAULT       49 targets include it   335 executions
under --all-features 47 targets include it   321 executions
```

When `deploy_commands` and `deploy_audit_log` vanish under `auth`, they take
their COPIES of those seven with them — but the same seven functions still run
in 47 other targets. **They are not default-only in any coverage sense.** My
first reading of the diff called it "18 tests only under default"; the honest
figure is **4**, and the asymmetry is 4-vs-4, not 18-vs-5.

★ **3889 is a count of test EXECUTIONS, not of distinct tests.** For "did
anything break" that is the right noun. For "how much is covered" it is not —
**a measurement is a pair (VALUE, NOUN)**, and a per-target diff answers the
first question while looking like it answers the second.

The duplication is deliberate and guarded: `test_support/gleam.rs` carries
`the_shared_gleam_support_copies_have_not_diverged`, because the module is
copied across crates. Worth knowing before anyone "fixes" it — though 335
executions to check six copies is a ratio nobody chose on purpose.

★ **The load-bearing claim survives unchanged: neither resolution is a superset
of the other, so a green run of either is not evidence about the other.** That
is why the house battery runs both gates — now measured at the level of
distinct test functions rather than asserted from a count.
