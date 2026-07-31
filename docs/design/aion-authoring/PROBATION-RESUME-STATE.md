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
house battery peak = ~55 GiB of target, from clean OR from here
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
target/debug/deps         15 GiB
  of which linked test executables   4.33 GiB over 75 files
  largest single test binary          247 MB (aion_server lib test)
target/debug/incremental   4.9 GiB
```

**`--all-targets` is not a second feature resolution — it is ~75 separately
linked binaries.** "Mostly a relink" was the right mechanism and the wrong
magnitude: one relink of the aion-server test binary is 247 MB, and there are
dozens. The class estimate for `-p a -p b --all-targets --all-features` on this
workspace is **+12 to +18 GiB**, not +5 to +8.

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
  running; declared +30..+38, abort at 50 (not 45 — see the band, below)
  log: .evidence/118/shakedown-allfeatures.log
```

🔴 **A green Block A is NOT evidence about Block B.** Different feature
resolution, different artifact set, and the four tests that were red on main
this morning ONLY COMPILE under `--all-features`. Reporting "the house battery
is green" off A alone would be reading a partial run as a complete one — the
same defect as #120 one level up.

**Count TARGETS, not tests.** 240 `Running` lines is a denominator; 3902
passing tests is not.

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
- Watch free during it, abort at 45.
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
