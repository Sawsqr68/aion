# The wake-prompt shakedown — banked definition

Banked 2026-07-31 at Waffles' instruction, from his transcript excavation.
**This item died once in a compaction. It is written down so it cannot die
twice.**

## Provenance — it originated at this seat

My own message to Waffles, 2026-07-29, verbatim:

> the redeploy order you set stands: rig redeploys, wake-prompt shakedown, then
> Apollo's haematite lanes.

I set the order, then lost the middle item in a compaction. It survived only in
Waffles' ledger. **The banking miss and the excavation are the same law from
both sides:** an item that lives only in a conversation is an item that is
already gone. This file is the remedy.

Its id `5c433ade` is a **Meridian-side brief id**, which is why grepping the
aion tree for it found nothing. That was the confusion, and it is now resolved
by writing it here instead of hunting for it there.

## Precondition — resolved 2026-07-31

The gates rig redeploys from current aion `main`, **with `heartbeat_window`
reverted**.

I asked whether that revert was a tree commit I was missing. It is not.
Waffles' answer, from his banked 07-29 ruling:

> **`heartbeat_window` reverted TO ABSENT** — no explicit override in the rig's
> deploy config, so the probe runs the derived defaults: cadence **7500ms**,
> declared window **30000ms**.

Nothing in the tree ever needed reverting. `7833ec91`'s dead-man code is the
live mechanism and **stays**. The revert is rig-side deploy configuration only.

## This is a RE-RUN, not a first run

Also excavated 2026-07-31, and also lost from my own record: **I already drove
this once.** 07-29, 12:35Z, my words — "Redeploy complete and the shakedown is
RUNNING":

- server rebuilt at then-`main` `782b8e60` (pid 49862)
- `heartbeat_window` reverted to absent per the ruling
- worker rebuilt and shipped to **Dean's box**, registered 12:32:30Z
- shakedown workflow **`0756ecd5`** launched, three live confirmations
  including 90-second idle survival

It never reached a verified green: the monitor expired and both contexts reset
across the demo days. So the remaining work is a re-run from current `main`
(now carrying `8d731436`), not a first flight.

### Two operational consequences

1. **The 07-29 worker lived on Dean's box, which is off.** The re-run needs a
   worker registered somewhere live — my box or Tom's, my mechanics call — or
   it waits for Dean's box to return. **Name the worker's home in the green
   report** so the record carries it this time.
2. **The before-fixture is richer than the zombie pair.** Workflow `0756ecd5`'s
   partial run record from 07-29 may still sit in rig state. Read it, and them,
   before anything re-runs over it.

## What the shakedown is

Re-run the rig shakedown: drive the rig **under real load**, through the
remote-gates path, and prove the July-29 failure mode is dead.

## The founding incident — the before-state

Hit A′ live-fire, from the record:

- Worker lease expired **10:22:04Z, mid-cargo**.
- All **8 activity rows dead-lettered** on the 5×400ms budget.
- Runs **`ae0273fd`** and **`539f2363`** left **zombie-`Running`**.
- No worker afterwards.

Those two zombie runs were **deliberately left uncancelled as harvest
evidence.** They may still be sitting in the rig's state, in which case they
are the before-fixture and must be read before anything is re-run — a fixture
you destroy by re-running over it is a fixture you never had.

## What green means

Under a **real, battery-scale leg**:

1. Workers keep their leases — no mid-cargo expiry orphaning.
2. No new zombie-`Running` rows.
3. Wakes fire correctly via the fixed wake-prompt (meridian main `2e35521e1`,
   box-battery green at Waffles' hands per brief `5c433ade`).

All three, measured, on one run. A green that only shows (2) proves nothing
about (1) — a leg that never got far enough to expire a lease reads identical
to a leg whose leases held.

## What green triggers

Waffles closes `5c433ade`, which un-gates:

- Apollo's two haematite lanes.
- Artemis's EXIT-001 build leg.

That is the exact queue my July-29 message set. **The chain moves on my word**,
which is why the word has to be earned by a real load leg and not by a quiet
one.

## Ordering as ruled

**#113 disarm first** (once the box clears), **shakedown immediately after.**

Waffles' stated reason, banked because it is the justification for the ordering
and belongs with the item: the #113 blast-radius measurement — *one missing
feature flag between a proof server and Tom's live WAL* — is why the disarm
goes first.
