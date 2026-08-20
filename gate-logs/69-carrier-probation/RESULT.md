# Carrier 3 cured — gate result

**Commit gated:** `c8ec95bbb` (the fix) · **records:** `ee27aa6d9`
**Parent:** `0e8872423` (origin/main at arming, the #111 merge)
**Branch:** `fix/69-pin-diagnosis-completes`
**Log:** `battery-20260821.log` · **Venue:** `VENUE-NOTES.md`

## Declaration control

```
declared : 24 legs
executed : 24 legs
VERDICT: exact match — all 24 declared legs executed, and nothing else.
```

**HEAD verified unchanged at finish** (`ee27aa6d9`).

## Verdict: 23 green, 1 red — carrier 4, cleared by the within-battery control

`rust:test` exited 101 on
`api::http::worker_deployments::tests::http_put_survives_a_second_server_state_over_the_same_store`
— **carrier 4** of the four in the register (`FLAKE-VERDICT-cited.md`), 1/24 on a
base that predates every branch it has been blamed on.

**The within-battery control is decisive and cost one `grep`.** It is a lib test, so
it runs in both test legs. At this one commit, on this one box, minutes apart:

| | passes | failures |
|---|---|---|
| **carrier 4** `http_put_survives_a_second_server_state_over_the_same_store` | **1** | **1** |
| carrier 3 `production_boot_..._over_liminal` (the one this branch fixes) | **2** | 0 |
| carrier 1 `quiet_tenant_not_starved_by_bursty_tenant...` | 2 | 0 |
| carrier 2 `round_robin_gives_quiet_tenant_a_slot_every_sweep` | 2 | 0 |
| the #69 engine pin | 2 | 0 |

Passing in one leg and failing in the other at a single commit is the cheapest
evidence available that a red is not attributable to the branch.

**No mechanism, either.** This branch changes `mod lsub_prod_xnode_e2e` in
`crates/aion-server/src/run.rs` and one records file. Carrier 4 lives in
`api::http::worker_deployments` and shares nothing with it but the crate.

**I did not supply the load.** No build of mine ran during this battery — the rule
that a concurrent build in another worktree has caused exactly this kind of red is
respected by having run nothing, and said here rather than left to be assumed.

## ★ Carrier 3 held: 2/2 in this battery, after 0/20 in the confirmation run

The subject of this branch is one of the register's own carriers, so a single green
would have been weak evidence. It is not the evidence: the 20-run confirmation
recorded in `c8ec95bbb` is (13/25 before, 0/20 after). This battery's job was the
ordinary one — prove the change breaks nothing else — and 1172 other lib tests plus
23 other legs say it does not.

## Carrier 4 is now diagnosed too, and it needed no new instrument

Its failure already carries its cause, because haematite's error was written by
someone who expected to be read:

```
Backend("haematite database error: data dir is locked by another live writer
(writer lock held at .../worker-deployments.db/writer.lock); a second writer
would corrupt the shard WALs — use ReadOnlyDatabase to observe")
```

The test's whole premise is building a **second** `ServerState` over the **same**
store, and haematite refuses a second live writer while the first holds the lock.

**Leading hypothesis, not established:** the test builds the second state before the
first's writer lock has actually been released, so it races a release it assumes is
already done. That would make it the same CLASS as carrier 3 — a test asserting an
event the system has not yet guaranteed — with an entirely different mechanism.

🔴 Not fixed here, and deliberately not: it belongs in its own commit with its own
before/after, not folded into a commit about a different carrier. What is recorded
is that the diagnosis is available and cheap, which is more than was true of this
carrier an hour ago.

## On not re-running

The standing rule that a battery with a red cannot be cited as green is respected by
**not making that claim**. This is not a green battery. The claim is: 23 legs green,
one leg red on a documented carrier that passed in the sibling leg at this same
commit and has a base rate predating this branch.

A re-run would buy a cleaner-looking log rather than better evidence — and with the
diagnosis above in hand, the box time is better spent curing carrier 4 than
re-rolling for a quiet log. The choice is stated rather than buried, and it is the
landing authority's to overturn.
