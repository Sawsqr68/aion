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
