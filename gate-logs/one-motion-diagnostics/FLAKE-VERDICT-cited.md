# Verdict — lock-race attribution, 24×2

**base** `cac631108` · **branch** `fca0cfb30` · 24 rounds, base and branch run
**concurrently each round** · 2026-08-19T10:11:50Z → 10:31:21Z

This directory held 48 raw logs and a TSV with **no conclusion**. That is why the
finding has now been re-derived from scratch twice — once on 2026-08-20 for
aion #54, and again on 2026-08-21 when a different carrier fired. The verdict is
written here so there is no third time.

> ⚠️ **Provenance, stated because it limits this file.** The 48 raw logs, the
> `.code` exit files and the TSVs are tracked **only on the `investigate-workflow`
> records branch — none of them are on `main`.** If you are reading this from
> `main`, the regeneration command below will find nothing, and that is expected
> rather than a broken citation.
>
> This file is therefore written to be **self-contained**: the counts in the
> table are the finding, and they are quotable without the logs. A conclusion
> that can only be checked where its evidence lives is a conclusion most people
> cannot check at all.

## Totals

**6 base reds, 5 branch reds.** The change under study is not attributable: the
base side failed *more*.

## Carrier census — FOUR tests, all `aion-server` lib tests

| test | base | branch |
|---|---|---|
| `worker::outbox_dispatcher::tests::quiet_tenant_not_starved_by_bursty_tenant_spread_across_routes` | 2/24 | 3/24 |
| `worker::outbox_dispatcher::tests::round_robin_gives_quiet_tenant_a_slot_every_sweep` | 2/24 | 2/24 |
| `run::lsub_prod_xnode_e2e::production_boot_dispatches_executes_and_records_over_liminal` | 2/24 | 1/24 |
| `api::http::worker_deployments::tests::http_put_survives_a_second_server_state_over_the_same_store` | 1/24 | 1/24 |

Regenerate either column with:

```sh
grep -h '\.\.\. FAILED' base-*.log \
  | sed 's/^test //; s/ \.\.\..*//' | sort | uniq -c | sort -rn
```

**Every carrier fails on both sides**, and two of the four fail more often on
base. These are load-sensitive box/store properties surfacing through whichever
test is unlucky, not defects of the named tests.

## How to use this

1. When one of the four fires in a battery, **cite the row above** — do not
   re-attribute it to the branch under test, and do not re-run this study.
2. **Check the within-battery control first**: these are lib tests that run in
   both `rust:test` and `rust:test-all-features`. Passing in one leg and failing
   in the other, at the same commit, is the cheapest possible evidence and costs
   one `grep`.
3. 🔴 **A battery with a red cannot be cited as green either way.** The cost of
   these flakes is the re-run, and the deeper cost is that an intermittently-red
   gate teaches people to wave gates through.
4. 🔴 **Check whether you supplied the load yourself** — a build in another
   worktree during a battery has caused exactly this. Say so at the time; "the
   box was busy" is unfalsifiable afterwards.

## What a base rate does and does not license

A measured base rate is what entitles the word **flake**. Without one, the honest
filing is *unexplained and open* — a narrower claim, and the correct one.

Worked contrast from 2026-08-21, both in the same leg on the same night:

- `production_boot_…_over_liminal` fired → **2/24 base vs 1/24 branch** → cited
  as a known flake, branch cleared.
- `one_motion_live_e2e::one_motion_run_compiles_deploys_starts_awaits_and_reports_the_result`
  fired → **zero** prior failures in any gate log on this box → **not** called a
  flake, filed as unexplained and left open. (It did not recur, which is still
  not a base rate.)

**The word follows the number.**

## Do not "fix" these by raising timeouts

The actionable question is whether a wait's bound is merely too tight for a
loaded box or whether it is a **liveness bug that load exposes**. Those want
different fixes, and the distinction is measurable under controlled load. A bound
raised until the flake stops is how a liveness bug gets buried — aion **#7** is a
shard-read timeout that already caused a permanent workflow wedge here.

---

# UPDATE 2026-08-21 — carrier 3 is DIAGNOSED AND FIXED

`run::lsub_prod_xnode_e2e::production_boot_dispatches_executes_and_records_over_liminal`
(2/24 base, 1/24 branch above) has a cause and a cure. The other three rows are
untouched and remain unexplained.

## Cause

The test waited a fixed **5 seconds** for `select_worker` to return a worker. But a
freshly connected worker is **dispatch-ineligible by design** until it serves an
opening probation: `Reachability::is_proved` requires `DISPATCH_PROBATION_PINGS`
(= 2) consecutive answered liveness pings, at a probe cadence of
`sweep_interval(heartbeat_window)` = `window / 4` — **7.5s** on the default 30s
window, so **~15s** before eligibility.

Which raises the obvious objection: if probation always took ~15s the test would
always fail, and it passed about half the time. The resolution is not a second
mechanism, it is that `dispatch_ineligible` starts **empty** and `select_worker`
filters only against what the probe has already published:

- **no probe round inside the wait** → the set is empty → the worker is selected
  immediately → PASS;
- **a round lands inside the wait** → the fresh worker is correctly withheld for
  its probation → ~15s > 5s → FAIL.

Load-sensitive exactly as this study describes, and now with a direction: load
slows the test's own progress, which gives the probe time to land a round.

## 🔴 The passing runs were the wrong ones

When it passed, the test dispatched to a worker that had **not served its
probation** — a path production does not permit, because production parks those
dispatches. So this was never "a good test that occasionally trips": it was a test
outrunning a safety gate and failing when the gate won.

## Cure — derived, not raised

The wait now targets dispatch **eligibility** with a patience computed from the two
values the server itself derives it from: `sweep_interval(configured_window) *
(DISPATCH_PROBATION_PINGS + 1)`. The test and the probe read the same source, so
they cannot drift, and no constant was invented.

This is emphatically **not** the timeout-raising that section "Do not fix these by
raising timeouts" warns against. That warning is about raising a bound until a
symptom stops. Here the bound is a function of the mechanism that sets it, and the
change makes the test model production rather than race it.

## Confirmed by prediction

```
BEFORE  13 / 25 failures
AFTER    0 / 20 failures
```

Same box, same session, same venue. (That 13/25 is not this study's 2/24 — rapid
back-to-back runs on a box already carrying a live server. The rate is not
transferable between venues; the mechanism is.) If the rate were unchanged,
P(0 in 20) ~ 4e-7.

**Instrument note.** This was found by the registration diagnosis added the same
night: the failure had reported one sentence for three different worlds, and once
it reported the listener state, the registry contents, the pool census and the
dispatch-eligible set, the cause was legible on the first real occurrence. A
failure that names the fact and withholds the cause is why this carrier survived
two full re-derivations of this study.

# 🔴 UPDATE 2026-08-21 — EVERY RATE ABOVE IS A PRE-PROFILE NUMBER

The `fleet/rb-lockfix-result` landing introduces workspace build profiles that
did not exist when any rate on this page was measured:

    [profile.dev]   debug = 0, incremental = false
    [profile.test]  debug = 0, incremental = false

`origin/main` carried **no `[profile]` sections at all** before it. Every rate in
the carrier census — 2/24, 2/24, 2/24, 1/24, and both branch columns — was
measured under cargo's default `debug = 2, incremental = true`.

## Why that matters more here than it would elsewhere

These are **load- and timing-sensitive** failures. That is the whole reason this
page exists. A build-profile change is not neutral with respect to them:

- Measured at the desk on one crate's full target set, clean slate each arm:
  **1913 MiB / 50.7s** at the old default → **862 MiB / 38.5s** at `debug = 0`.
- The lockfix delivery's own report records the sharper case: a SHA-256 of the
  whole test binary collapsed from **60s+ to 18s** under `debug = 0`, and that
  collapse is what moved carrier 4 **into** its race window. The profile change
  is documented, by the delivery itself, as having changed which side of a race
  a test lands on.

A change that moves one test into a race can move another out of one. Nothing
here says the rates got better or worse — it says **they were taken in a
different world**.

## The rule this page now carries

🔴 **Do not cite a rate from the census above as the base rate for a red
observed after this landing.** "Known flake, 2/24" would be a control from a
build profile that no longer exists — and that citation is precisely the thing
that stops the next person investigating. That is the failure this page was
written to prevent, so it must not become the way this page causes it.

**What is still legitimate after the landing:**

- The **within-battery control** — the same lib test running in both `rust:test`
  and `rust:test-all-features`, passing in one leg and failing in the other at
  one commit. It compares two runs in the *same* world, so no rate is needed and
  the profile change cannot touch it. It stays the cheapest evidence available
  that a red is not attributable to a branch. It costs one `grep`.
- The **mechanisms**, all of which are properties of the code and not of the
  profile: carrier 3's opening-probation race (diagnosed and cured, see the
  2026-08-21 update above), and carrier 4's inherited-OFD race (diagnosed by the
  fleet — the writer lock was held by a `posix_spawn`ed child of a *different*
  test, caught in its clone→exec window holding an inherited copy of the lock's
  open file description; flock ownership rides the OFD, so the copy IS the lock).

**What is not legitimate:** the numbers, as numbers, for anything measured after
this lands.

## Marked, not re-derived — and said plainly

I am **not** re-deriving four base rates here. Each one costs a 24-leg battery,
and carriers 3 and 4 are now cured, so two of the four would be re-measuring
something that no longer fails for the recorded reason.

Naming the gap is the honest move; quoting a number I have not measured in the
new world would be worse than admitting I do not have one. If a post-landing red
needs a base rate, **measure it then, in the world it will be spent in, and
write the profile beside it** — which is the discipline this page should have
carried from the first day.

⇒ Every future rate recorded on this page states the build profile it was
measured under. A number without its world is not a control.
