# The one-motion boot-diagnostics fix — gate result

**Commit gated:** `96737f44becb72fa362dceaf14a72aae204a3db5`
**Parent:** `ab9915f5b618b3ba8b44915076562a3739ee35b5` (origin/main at arming, the #109 merge)
**Branch:** `fix/one-motion-boot-diagnostics`
**Log:** `battery-20260821.log` · **Venue:** `VENUE-NOTES.md`

## Declaration control

```
declared : 24 legs
executed : 24 legs
VERDICT: exact match — all 24 declared legs executed, and nothing else.
```

Its caveat is respected: that says every leg **ran**, not that any leg tests what
it is named after. **HEAD verified unchanged at finish.**

## Verdict: 23 green, 1 red — a documented carrier, cleared by its base rate

`rust:test-all-features` exited 101 on
`run::lsub_prod_xnode_e2e::production_boot_dispatches_executes_and_records_over_liminal`,
failing with `worker never registered in-band for the pool`.

This is one of **four** documented carriers of the load-sensitive flake studied in
`gate-logs/lock-race-attribution/` — 24 rounds, base and branch run concurrently,
base `cac631108`, which predates this branch entirely:

| carrier (all `aion-server` lib tests) | base | branch |
|---|---|---|
| `quiet_tenant_not_starved_by_bursty_tenant_spread_across_routes` | 2/24 | 3/24 |
| `round_robin_gives_quiet_tenant_a_slot_every_sweep` | 2/24 | 2/24 |
| **`production_boot_dispatches_executes_and_records_over_liminal`** | **2/24** | **1/24** |
| `http_put_survives_a_second_server_state_over_the_same_store` | 1/24 | 1/24 |

Study totals: 6 base reds, 5 branch reds. Every carrier fails on **both** sides,
and this one fails **more often on base**. The verdict is now written beside that
evidence as `gate-logs/lock-race-attribution/VERDICT.md`, because the finding had
already been re-derived from scratch twice for want of a written conclusion.

**The within-battery control says the same thing, and cost one `grep`:** the same
test **passed in this battery's `rust:test` leg** — same commit, same box, minutes
earlier. It is a lib test, so it runs in both legs; passing in one and failing in
the other at one commit is the cheapest evidence available and should be the first
check every time.

**No mechanism.** This branch changes exactly one file,
`crates/aion-cli/tests/one_motion_live_e2e.rs`, which `git grep` confirms nothing
else references — a build-graph leaf. It has no relationship to liminal cross-node
dispatch.

## On not re-running

The standing rule that a battery with a red cannot be cited as green is respected
by **not making that claim**. This is not a green battery. The claim is narrower:
23 legs green, one leg red on a carrier with a measured base rate that clears this
branch. With four known carriers, a re-run carries a material chance of surfacing
another, so it would buy a cleaner-looking log rather than better evidence. The
choice is stated rather than buried, and it is the landing authority's to overturn.

## ★ The instrument earned its keep on this run

The whole reason a full battery was spent on a leaf test file was to re-run
`rust:test-all-features` — the leg that produced #100's unexplained red — **now
instrumented**.

`one_motion_live_e2e` **passed in both legs here**, including the new pin
`a_boot_failure_reports_the_servers_own_account_of_it`. So that red did **not**
recur and remains filed as *unexplained and open*, exactly as #100's record says.
Two batteries is not a base rate and nothing about that filing changes.

What has changed is that the next occurrence cannot be silent: it will carry the
server's own account of why it died instead of an exit code.

## The distinction this run exercised twice, deliberately

Two reds, two nights, one leg, **two different verdicts under one standard**:

- `one_motion_live_e2e` — **zero** prior failures in any gate log ⇒ no base rate
  ⇒ no entitlement to the word "flake" ⇒ filed unexplained and open.
- `production_boot_…_over_liminal` — **2/24 on base** ⇒ a citable base rate ⇒
  named as the known flake it is, and the branch cleared.

**The word follows the number.** Neither red was waved through and neither was
over-charged to the branch.
