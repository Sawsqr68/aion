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
