# RESUME STATE — probationary re-admission (Waffles' ruling, 12:44Z 2026-07-31)

**Read this first on resume.** Work is IN FLIGHT and UNCOMMITTED.

## Where things stand

`origin/main` = **`bbc6f21b`** (all earlier work pushed and green).
Closed today: **#58**, **#116**, **#118**. Open: **#117**, **#119**, **#120**
(battery half landed in `bbc6f21b`; the CI/nextest half is a board item).

## IN FLIGHT — uncommitted, builds clean, NOT yet gated

`crates/aion-server/src/worker/heartbeat.rs` — probationary re-admission:

- `reachability: HashMap<WorkerId, Reachability>` (was `Instant`), where
  `Reachability { consecutive_answers: u32, proved_at: Option<Instant> }`.
- `const DISPATCH_PROBATION_PINGS: u32 = 2` — **K is mine, rationale is in the
  doc comment on the constant.** One success re-admits a link that answered once
  by luck, so a link answering one probe in three would flap forever; two is the
  smallest K that distinguishes "answered" from "answering". Three+ charges every
  honest worker an extra probe cadence of undispatchability to catch a dishonest
  one.
- `register_connection` now opens an UNSERVED probation (zero answers, no proof)
  instead of seeding reachability. **This reverses my earlier reading** that the
  handshake is a completed round trip: the ack is SENT, and nothing reports it
  was RECEIVED.
- `record_dispatch_reachability` increments the consecutive counter.
- **NEW** `record_dispatch_unreachable(worker_id)` — resets the counter to zero.
  This is what makes the probation consecutive rather than cumulative.
- `is_dispatch_reachable` / `unreachable_workers` now go through
  `Reachability::is_proved(now, window)`.

## OWED, in order

1. **Wire `record_dispatch_unreachable` into the probe.**
   `crates/aion-server/src/worker/liminal_liveness.rs` — `apply_answer` already
   calls `record_dispatch_reachability` on success and emits two distinct WARNs
   on failure (`PingFailure::Unaskable` / `Unanswered`). **Both failure arms must
   now also call `record_dispatch_unreachable`**, or the probation never resets
   and the flapping fix does nothing. Without this the code compiles and is
   silently inert — check it deliberately.

2. **Two existing tests WILL FAIL and must be rewritten, not deleted.**
   - `heartbeat::reachability_tests::registration_seeds_reachability_so_a_new_worker_is_dispatchable`
     — its premise is now false BY RULING. Rewrite as
     `registration_opens_a_probation_and_does_not_grant_eligibility`, plus a new
     test that K answered pings DO grant it, and that K−1 do not.
   - `registry::tests::a_pump_alive_worker_the_server_cannot_reach_is_not_selected`
     — its baseline ("freshly registered ⇒ selectable") and its recovery arm
     ("one answered ping ⇒ selectable again") both assume the old seeding.
     Recovery now needs K pings. **Keep both as controls** — the test is only
     non-vacuous because it shows selection turning back on.
   - Add the flapping pin itself: alternate answer/no-answer and assert
     eligibility NEVER returns. That is the defect being fixed and nothing
     currently covers it.

3. **Gates**, `--all-features` throughout:
   `cargo clippy -p aion-worker -p aion-server --all-targets --all-features -- -D warnings`
   then `cargo test -p aion-worker -p aion-server --all-targets --all-features`.
   Baseline before this change: **1019 passed / 0 failed**.

4. **SEQUENCING — my call, and I have made it: THE FIX PRECEDES THE GREEN
   RE-RUN.** Waffles left it open. Reason: a worker cycling in and out of
   eligibility during a long shakedown produces exactly the intermittent,
   hard-to-attribute evidence that costs hours to interpret — and the whole point
   of the re-run is to produce readings nobody has to caveat. Certifying with
   flapping documented as a before-state would put an exception clause on every
   line of the result, which is the same mistake as running acceptance over a red
   baseline. Do not start the re-run until this lands green.

## 🔴 DISK — the numbers changed and my old filing was wrong

```
NOW    aion/target 1.4 GiB (release only; target/debug DELETED)   free 91 GiB
PEAK   aion/target 55 GiB during a full house battery
```

**The aion house battery costs ~55 GiB.** `scripts/battery.sh` runs BOTH
`cargo test --workspace` and `--workspace --all-features`; those are different
feature resolutions and therefore different artifact sets, not a rerun of one.
34 GiB for all-features, 55 with the default set added.

Filed with Athena: **STEADY 2 GiB · SPIKE +55, named cause "the house battery",
one in flight, declared before it starts, reclaimed the moment it ends.**

**Standing commitments — these are mine to keep, not Athena's to catch:**
- Declare the block to Athena (`dm:45cf420e-621d-467b-b96c-5bffb62c04f3`)
  BEFORE starting a battery.
- **Watch free myself during it and abort at 45 GiB.** On 2026-07-31 I did not,
  and free went to 38 — through her 40 tripwire — before I read it.
- Reclaim (`rm -rf target/debug`) the moment the block ends, and report the
  number so she can re-base.
- `target/release` is NEVER touched: Tom's live server `99844` runs from a
  deleted binary in there (#108).

## Lessons that must not be re-learned

- **A measurement reported outside the scope it was taken in is an estimate
  wearing a receipt.** My 13 GiB was real and it was the two-crate gate scope;
  "13 is aion's steady" was never measured. I generalised it and was out by 4×.
- **A count is only a coverage claim if the run reached the end** (#120). Count
  TARGETS, not tests — a per-test tally is a summary of an unknown denominator.
- **A denominator the instrument chooses cannot check the instrument.**
- **A red baseline is a blindfold, not a footnote.**
