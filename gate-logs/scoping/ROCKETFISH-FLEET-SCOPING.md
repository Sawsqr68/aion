# Rocketfish fleet scoping — steward's docket (2026-08-18, Vesper)

Commissioned by Tom's expansion order (item TWO), RAM health first. Access: `ssh rocketfish@100.125.55.89` (~/.ssh/config Host home-base).

## RAM — story closed, one scheduled item open
Tom's ruling: "supposed to be 32, it's the Galaxy" — and the hardware agrees: 2×16GiB DDR4 2133MT/s (ChannelA-DIMM0 + ChannelB-DIMM0, two slots empty), MemTotal 32785852kB. The dead-stick theory is closed; 32G IS the build. **Open item:** a full memtest pass has never been run at our hands. It requires the box offline, so it is a **scheduled-downtime item** — Tom names the window; a memtester run inside Linux (non-exclusive) is a weaker substitute we can run any time but it cannot clear the hardware the way the offline pass does. Until the pass, RAM is *presumed* healthy on the strength of clean gate history, not proven.

## Class of work
Rocketfish is a **gates-offload venue** — its proven history (remote clippy/test runs; its newer clippy doubles as a forward-compat canary). It is NOT yet an agent-seat venue: no claude/norn auth exists there, and granting it is a separate Tom decision with the same shape as the Dean-Mac docket.

## The toolchain hole and the hold
Rocketfish has **no gleam, no erl, no erlc anywhere** (checked command -v, ~/.local/bin, /usr/local/bin, ~/.cargo/bin, /usr/lib/erlang/bin). Every historical rocketfish green silently excluded the gleam-runtime populations (its 1157-vs-1143 count delta is that exclusion, now named). **Provisioning is HELD** (my blast-radius argument, accepted): #64 proved a toolchain mix can poison a whole suite invisibly — box gleam 1.18.1/OTP 27 emits a gleam_stdlib beam that beamr's validator refuses (validator gap, Artemis's fix lane, [m=1128]).
**Ruling folded in (Waffles, 13:12Z):** when provisioning unholds, rocketfish gets erlang/gleam only under a **KNOWN-GOOD mix named by Artemis's fix lane** — never latest-by-default — and its gates carry the #64 exclusion from minute one if her fix hasn't landed. Until then: anything wanting gleam coverage on rocketfish is **excluded-and-named in the gate script**, never silent.

## Worker seats (when opened)
1. First seat: `fleet_ops`-class script workers (bodiless actions, no agent auth needed) serving gates legs — lowest blast radius, immediate offload value.
2. Dev/counsel seats: a measurement question, not a default. Needs claude+norn auth (Tom's grant), bun for ACP, and a measured solo flight before any concurrency.
3. Capacity ramp: same law as the box — +1 concurrent per clean landing, capacity data banked per flight, step down on duration inflation.

## Order of operations proposed
(1) memtester (in-OS, any time) → (2) Tom's downtime window for the offline pass → (3) Artemis's fix lands or names the known-good mix → (4) provision pinned toolchain + install aion worker binaries → (5) first fleet_ops seat with an excluded-and-named gate → (6) measured ramp. Steps 4-6 gated on Tom's go.
