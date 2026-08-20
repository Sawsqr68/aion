# The aion#69 engine-level pin — gate result

**Commit gated:** `b00b5187a237b8c1a392ff8351fc88aee3b7be5e`
**Parent:** `0f60507a672c00abf62e8ea6c10276be56e7d14f` (origin/main at arming, the #110 merge)
**Branch:** `pin/69-engine-level-reconnect`
**Log:** `battery-20260821.log` · **Venue:** `VENUE-NOTES.md`

## Verdict: 24 of 24 green

```
0 of 24 gates failed.
```

No leg red. **Neither of this venue's flake carriers fired** — both
`production_boot_dispatches_executes_and_records_over_liminal` and the new pin
passed in `rust:test` AND `rust:test-all-features`. Two quiet batteries in a row
for that carrier is not a new base rate and changes nothing about its 2/24
register entry (`gate-logs/lock-race-attribution/VERDICT.md`).

## Declaration control

```
declared : 24 legs
executed : 24 legs
VERDICT: exact match — all 24 declared legs executed, and nothing else.
```

Its caveat is respected: that says every leg **ran**, not that any leg tests what
it is named after. **HEAD verified unchanged at finish.**

## ★ The pin ran in the gate's world, not only in mine

The measurement in the commit message was taken with `-p aion-server --lib`
(binary `aion_server-eea74bb1b4a101d2`). That is **not** the resolution either
test leg uses, and a green in one world is not a green in another.

This battery's `aion-server` binaries are `aion_server-1a306facc1400ea7` and
`aion_server-fae814998b4b3913` — two distinct identities, neither of them mine,
which is the mechanical proof that both legs compiled a different program from
the one I measured in. The new pin appears **twice** with `... ok`, once under
each.

The numbers were then re-taken under `--workspace --all-features` directly,
because `cargo test` captures stdout on a passing test and a number nobody can
read is not a published measurement:

```
aion#69 O3 — re-delivery of fan:0 (activity:0) took 188ms from the link breaking;
first attempt 1, second attempt 2
aion#69 — 5 dispatch(es) served for 4 activities
```

188ms sits inside the 114–239ms already measured, with the same identity and the
same attempt increment. **The finding holds in the resolution the gate declares.**

⚠️ First pass of that re-take was read wrong and is recorded because it reads
wrong easily: filtering the workspace run for `test result: ok` returned a screen
of `ok. 0 passed; 0 filtered out` from crates the filter matched nothing in. A
suite that ran nothing still prints `ok`. The answer only appears against
`1 passed`.

## Disk was reclaimed BEFORE arming, not during

The #110 battery had to absorb a disk alarm mid-run. Free space had fallen to
18 GiB with `target/` at 87G, so `target/debug/incremental` (42G — pure
incremental-compilation cache, regenerable by definition, holding no agent work,
configuration or evidence) was deleted first, taking free space to 45 GiB. It
held above 44 GiB for the whole run and no alarm fired.

Two other worktrees carry build trees belonging to live lanes that are not mine;
neither was touched at any point.

## What this battery does and does not license

It licenses landing `b00b5187a`: 24 legs declared, 24 executed, 24 green, on a
commit whose parent is exactly `origin/main`.

It does not license any claim about the *fix* for #69, which is not in this
commit. What is here is the measurement that prices the fix, and a pin
deliberately written so that the fix will not read as a regression against it —
when the fix lands, this pin should print *"was NOT re-dispatched and the workflow
still settled"*, and that line is the fix's own evidence rather than a green tick.
