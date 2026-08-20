# rb-lockfix landing — gate result

**Commit gated:** `31cf6cf7e` · **Branch:** `land/rb-lockfix`
**Contents:** the fleet delivery `113401a7b`, `origin/main f576fba4b` merged in,
plus two desk fixes (`efc62b55a`, `31cf6cf7e`).
**Log:** `battery-20260821.log` · **Venue:** `VENUE-NOTES.md`

## Verdict: 24 of 24 GREEN, declaration exact

```
0 of 24 gates failed.
declared : 24 legs   executed : 24 legs
VERDICT: exact match — all 24 declared legs executed, and nothing else.
```

**HEAD verified unchanged at the finish** (`31cf6cf7e`), stamped ×9 in the log.
**628 `test result:` lines, every one `0 failed`.**

### The tests that had to run, read off individual `... ok` lines

Each appears **twice** — once in `rust:test` and once in `rust:test-all-features`
— because a lib test that runs in only one leg is a coverage claim nobody checked.

| test | ok-lines |
|---|---|
| carrier 4 `http_put_survives_a_second_server_state_over_the_same_store` | 2 |
| carrier 3 `production_boot_dispatches_executes_and_records_over_liminal` | 2 |
| carrier 1 `quiet_tenant_not_starved_by_bursty_tenant_spread_across_routes` | 2 |
| carrier 2 `round_robin_gives_quiet_tenant_a_slot_every_sweep` | 2 |
| `lock_acquisition::transient_lock_is_retried_but_live_writer_still_fails_unchanged` | 2 |
| `lock_acquisition::spans_that_bound_no_waited_retry_are_refused_at_construction` | 2 |
| `the_embedded_template_matches_the_shared_cli_template` (byte-identity pin) | 2 |
| `external_package_cannot_dispatch_without_await_but_public_run_compiles` | 2 |

**All four flake-register carriers passed in both legs — 8 executions, 0 failures.**

## 🔴 This is run 4. Runs 1–3 are on disk and none of them is hidden.

| run | outcome | why it is not this verdict |
|---|---|---|
| 1 | aborted ~8 min, 2 legs green | I found the shipped template telling operators to **RAISE THE PATIENCE** at a lock refusal. A build input, so it could not ride post-gate. |
| 2 | aborted ~7 min, 2 legs green | Five `aion new` READMEs named the **wrong data directory**. Also a build input. |
| 3 | 23 green, 1 red, declaration exact rc 0 | `rust:test-all-features` red on a **hex.pm outage**, self-classified by the instrument as infrastructure — *"the boundary was never compiled and this run proves nothing"*. |
| **4** | **24/24 green** | this verdict |

Logs preserved as `battery-20260821-ABORTED-for-template-fix.log`,
`-ABORTED-2-for-readme-truth.log`, `-run3-hexpm-outage.log`. Every abort's reason
is written into `VENUE-NOTES.md` so none of them can read as a crash.

### Why run 4 is not a re-roll

The rule against re-runs exists so a flake cannot be rolled away until it hides.
Nothing is hidden here:

- the cause was **named by the instrument**, not guessed by me;
- it is **external** to this repository and this branch (an HTTP fetch to
  `hex.pm/api/packages/gleam_json/releases/3.1.0`);
- it was **verified resolved before re-firing** — `curl` to the identical URL
  returned **200 in 0.62s**, and sibling `gleam build` calls in the same leg had
  succeeded, so the registry was up for some fetches and not that one;
- the same assertion had **already passed at this same commit** in the
  `rust:test` leg (run 3, log line 4508), so no assertion was escaped;
- run 3's own declaration control was **exact, rc 0** — it was a complete,
  honest run that measured one thing it could not reach.

In run 4 that probe shows **2 `... ok` lines**, one per leg. The transient
diagnosis is confirmed, not assumed.

## 🔴 Finding recorded, not fixed here: the battery is not hermetic

A landing attestation can red because a third party's HTTP service hiccuped.
The cost is not the lost time — it is the reflex. This repository's own
`battery-costly-announcements.sh` writes the law in its comments about its own
repaired defect: *"an intermittently-red gate is the kind people learn to wave
through, and the wave-through is what eventually covers a real red."*

The cure is not more retries — the retry policy already exists and already
exhausted. Either pre-warm the Gleam dependency cache in the venue (the way a
warm `~/.cargo/registry` already makes us hermetic against crates.io), or route a
registry failure to the **`unproven`** verdict this very delivery brings
(`workflows/repo-battery/bin/verify.sh:51`). Full write-up:
`FINDING-battery-is-not-hermetic` in the desk notes; folded into the skip-family
change, which wants the same third verdict.

## What this battery does NOT settle

It does not re-price the **250ms lock-acquisition ruling**. That was fitted on a
16-core Linux VM from 3 samples of a 93–150ms window; this is a 10-core Mac, and
macOS reaches process creation by a different path. Carrier 4's 1/24 base rate
*was* measured here, so this venue is a legitimate world for the race — but
whether reds here share the fleet's mechanism is **unmeasured**, and assuming so
is the error that nearly closed carrier 3 early and wrong.

Stated before firing and kept: **a red against the ruling would not have bought a
larger patience.** That is the inflation the register bans.

## Base

Attests the pair `(31cf6cf7e, f576fba4b)` — `origin/main` at arming, verified.
The delivery arrived 20 commits / 3 merges behind main; main was merged in at the
desk and **both sides verified present afterwards** rather than trusting a quiet
merge, because the branch adds a *required parameter* to
`HaematiteStore::open`/`open_or_create` and main had been adding callers.

`.fleet/` is present at the gated commit and is removed in a visible commit
**after** this gate, on a census showing 0 of 24 legs reference it. See the
removal commit's own message.
