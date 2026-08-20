# Venue — the aion#69 engine-level pin

- Worktree: `.worktrees/100-budget`, branch `pin/69-engine-level-reconnect`
- **FROZEN HEAD**: b00b5187a237b8c1a392ff8351fc88aee3b7be5e
- **Parent**: 0f60507a672c00abf62e8ea6c10276be56e7d14f (= origin/main at arming, the #110 merge)
- Box: 10 cores, load at arming: 6.43 5.96 5.93
- Free disk at arming: 45 GiB on /System/Volumes/Data
- Tom's live server (pid 71475) is up and holds 8080/50051/50061 — the same
  venue the #100 and #110 batteries ran in, deliberately.

## Disk was reclaimed BEFORE arming, not during

Free space had fallen to 18 GiB with `target/` at 87G. The #110 battery had to
handle a disk alarm mid-run; this one reclaims first. `target/debug/incremental`
(42G) was deleted — pure incremental-compilation cache, regenerable by
definition, and it holds no agent work, configuration or evidence. Nothing else
was touched: `port-247` (9.2G) and `247-leg3` (4.6G) are live lanes belonging to
other work and are off limits at any pressure.

Cost: the first compile of each leg is cold rather than incremental. The gate
compiles from source either way, so the attestation is unaffected.

## Why the full battery

The change is one file, `crates/aion-server/src/run.rs`, but it is NOT a
build-graph leaf — `run.rs` is a module of the `aion-server` library, so the
whole crate and everything downstream of it recompiles. A scoped gate would not
have been defensible.

The measurement it carries was taken in the `-p aion-server --lib` world
(binary `aion_server-eea74bb1b4a101d2`), which is NOT the gate's
`--workspace --all-features` resolution. This battery is also how that
measurement gets re-taken in the world the gate actually declares.

## Expected carrier

`production_boot_dispatches_executes_and_records_over_liminal` shares this
module and is one of four documented carriers of a load-sensitive flake, 2/24 on
a base that predates this branch (`gate-logs/lock-race-attribution/VERDICT.md`).
The NEW pin sits in the same venue and inherits that sensitivity. If either
fires, cite the register and check the within-battery control first: both are lib
tests, so they run in `rust:test` AND `rust:test-all-features`, and passing in one
leg while failing in the other at one commit is the cheapest evidence available.
