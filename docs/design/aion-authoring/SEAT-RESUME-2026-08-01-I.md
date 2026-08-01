# 📍 SEAT RESUME — 2026-08-01 (I)

**Supersedes H. Read this first.** I am **Vesper Lynd**, session
`5849e0d8-4802-4869-8e0f-9f7fd187e198`. Sign `— Vesper`. **Fable is the MODEL I
run on, not my name.**

## 🔴 IN FLIGHT RIGHT NOW — CHECK THIS BEFORE ANYTHING ELSE

**An Opus implementer is building AWL value-route-payload lowering in the MAIN
TREE (no worktree, per Tom's order). It may have left uncommitted changes.**

- agent id `a46aafc4e0cb7b421`, dispatched ~00:34Z. Resume it with SendMessage,
  or if the session is gone, **`git status` first** — its work is unlanded and
  uncommitted by design (it was told not to commit).
- Task: retire the `"value route payloads (\`route out(<value>)\`) — the rev-3
  flow shape is not yet lowered (B4)"` refusal pinned at
  `crates/aion-awl/src/mir/deferred_tests.rs:48-84`. ~11 of 16 uncovered
  fixtures.
- **THE GATE:** `crates/aion/tests/awl_bc4_differential/` — compiles each fixture
  BOTH ways and asserts byte-identical normalized execution trails through a
  real engine against the Gleam oracle. `COVERED_COUNT = 91` at `covered.rs:36`
  is a ratchet and must rise by exactly the number added.
- 🔴 It was told, and the reviewer must hold it to this: **the refusal pins are
  CONTRACTS, not stale expectations.** Retiring one is legitimate only when the
  differential passes. A fixture that cannot pass stays refusing and gets said
  out loud. This is the exact trap that cost the #90 revert.

## State

```
origin/main = 03bc9e86 (PUSHED, local == remote)
  03bc9e86  docs(seat): marker H — #127 diagnosed, framing corrected
  2a33475e  test(conformance): concurrent-append assertion names the actual error
  8741e7d4  docs(seat): marker H
  f7a08aaf  revert(retry): #90's fix was the defect
crates/aion/ is byte-identical to the pre-#90 state 3bee2840
```

## 🔴 #90 REVERTED — the fix WAS the defect

Full record: `RETRY-CLASS-HONESTY-90.md` (rewritten to say the opposite of its
first version). Task #90 has the detail and **stays open** — the operator-facing
observation is real, only the fix was wrong.

One line: `ActivityErrorKind` and the `retryable:` prefix are not duplicates.
`kind` is the replay disposition and on that path is a **hardcoded constant**
(`nif_activity.rs:262`); the prefix is the **Gleam SDK's typed-error wire
format**, parsed by `parse_error` and pattern-matched by workflow code. The fix
rewrote the informative field to agree with the constant one, making
`error.Retryable` unreachable from any remote activity. Cally's name for it, and
it is the best one: **proof against the wrong population** — the mutation suite
proved the fix changed behaviour; nothing proved the OLD behaviour wasn't a
contract.

## ✅ #127 REPRODUCED ON DEMAND — routed to Apollo, do not carry it

Hit at iteration 258, `00:38:13Z`, verified in my own log. `first=Ok(())` on the
wire ⇒ history WAS protected; it is a **misclassification, not a single-writer
breach**. `2a33475e` earned itself: the hit is self-describing.

🔴 **Statistical correction that is MINE, and the transferable part:** I framed
"0 in 118 quiet" vs "1 in 258 loaded" as a LOAD effect. But
P(0 in 118 | rate 1/258) ≈ **0.63** — the two are compatible with ONE rate and
pure chance, and the hit landed with **builders = 0** under moderate load,
outside any marked window. ⇒ **A RARE-EVENT MISS IS NOT A SIGNAL UNTIL YOU
COMPUTE WHAT CHANCE PREDICTS.** Reaching for a mechanism to explain a difference
chance already explains manufactures a causal story out of noise.

Athena's honest limit, recorded because it bounds the claim: her sampler cadence
is 10s, the window is microseconds — four orders of magnitude too coarse to
attribute the hit to any sampled condition. The instrumentation bought a
timestamped oracle-carrying repro, **not** a dose-response claim.

### 🔴 APOLLO'S RULINGS + AN UNBLOCK I HAD WRONG (via Cally, 00:41Z)

**I said aion CANNOT avoid the full scan today. That is WRONG — the capability
exists public, one level down:**
- `Database::read_stream_next_seq` is **pub** at `db.rs:224`
- `EventStore::database()` at `:74` hands out the `&Database`
- ⇒ `store.database().read_stream_next_seq(key)` works **today**, no haematite
  release needed. My "no head read exists" claim surveyed only `EventStore`'s
  own methods. ⇒ **AN API-SURFACE ABSENCE CLAIM MUST NAME THE LAYER IT
  SEARCHED** — one type's method list is not the crate's capability.

⚠️ **THE TRAP, Apollo's words, read before using it:** *"the engine is 1-BASED,
the public API is 0-BASED, and `read_from` does that translation internally at
`:179-183`. A caller reaching through `database()` gets the ENGINE-side number
and will be off by one unless it repeats the translation."* If we take the
shortcut, **own the conversion at the call site and comment it** — owning that
conversion is the missing `EventStore` method's real job.

**His rulings:** the misclassification is **#74** on his tracker (0.8.0-class);
the head read is **#75**, and they land in the **SAME commit** — because the
caller only reads from 0 for want of a cheaper way to learn the head, so fixing
one alone leaves the pressure that produced the bug. Both ruled, **neither
implemented**; his seat has no executor floor, so no date (he was straight that
a date would be fiction).

🔴 **SEQUENCING — DO NOT BE SURPRISED BY THIS:** my assertion at `2a33475e`
names the CURRENT (wrong) class, so **our suite is green against the defect.
When Apollo's fix lands, that test goes RED.** That is correct downstream-
consumer behaviour, not a regression — but his fix and our assertion update must
coordinate. He flagged it himself.

He confirmed the mechanism at the bytes independently (`:184`/`:185` two
round-trips, predicate at `:192` satisfied exactly by a lost race) while
**declining to carry my reproduction numbers as fact** — *"a reproduction profile
consistent with a mechanism is not independent evidence of it."* That is the
right discipline and worth keeping as his phrasing.

## 🔴 #7 (AWL) — MEASURED, and the target is now precise

**"Direct bytecode" is DONE and shipped.** `aion deploy`/`aion run` go straight
to BEAM: `compile.rs:214` `select(&module)`, via `prepare.rs:44`, Gleam runtime
closure vendored as committed `.beam` files (`bundle_data.rs:111-192`). Gleam
survives only on preview paths (`emit --target gleam`, still the DEFAULT at
`awl.rs:85`) and as BC-4's reference oracle.

**THE GAP:** there is **NO Gleam fallback on the deploy path** (verified: zero
`Unsupported`/`Planning` handling in `prepare.rs` / `one_motion.rs`, control:
`compile` appears 14 and 9 times). ⇒ **16 valid AWL documents are UNDEPLOYABLE.**
Counted at my hand: `COVERED_COUNT = 91`, 107 valid fixtures, control 160
invalid. That is what "there's still no working language" means.

**Committed finish line** — `docs/design/WORKER-AUTHORING-STORY.md:156-158`:
subflow, on failure, substeps, value route payloads, dependency-parallel layers.

Remaining after the in-flight lane: substeps; dependency-parallel layers (NOT a
keyword — arises from multi-dependency `after`, so flagship `dev_brief` hits it);
subflow (measure first, no distinct pin found); on-failure substep arm (cheapest).

Everything else in the lane is BUILT AND GATED. Task #7 has the full table.

## Board

| item | note |
|---|---|
| **#7** | in flight — see top of this file |
| **#128** | one-motion pipeline WIRED BUT UNGATED (its only 2 tests are negative); `await_result` has NO timeout — hangs forever. Fix is to honour the declared timeout, NOT to invent one |
| **#90** | reverted, stays open; §5 of the record has the corrected design |
| **#127** | reproduced, routed to Apollo. Aion residue: `stream_head` is O(history) per append and cannot simply be deleted (`store.rs:1350-1353` explains why) |
| **#85** | libsql 0.9.30 double-close UAF, verified in dep source; did NOT fire in ws6 |
| **#77 / #82** | Tom's standing lanes: tharsis, NIF-state P0 |

## Comms — corrected tonight

- **"Waffles takes no DMs" is STALE.** He sends and receives; `dm:896955e1-…`
  works. Rule is now "DM works; board copy through Cally when stakes warrant".
  ⇒ **a comms rule stated as "never" decays silently** — nobody re-checks a
  prohibition, so it outlives its justification.
- Waffles' two orphan threads (gates-chain/wake-prompt shakedown, count-skew) are
  **RETIRED at his board, nothing owed.** `5c433ade` was never a git SHA — it is
  an acceptance/brief identifier. ⇒ **a missing identifier may be the wrong
  ARTEFACT CLASS, not a missing object** — a third category beyond "exists" and
  "unpushed".
- 🔴 **Author name cannot discriminate seats on this box** — every agent commits
  as `tomWhiting`. Rest attribution on the repo, never the name.

## Method laws earned today (the expensive ones)

- 🔴 **When two fields disagree, establish which carries information before
  correcting either.** One may be a constant.
- 🔴 **A string that crosses a language boundary is a wire format.** The search
  space follows the value across every boundary, not to the edge of your language.
- 🔴 **A contract can look like a stale pin** — mirror of "a defect can have a
  test defending it". Bulk-updating would have shipped the regression with its
  own gate rewritten to endorse it.
- 🔴 **A mutation proof shows a change is wired in, not that it is correct.**
- 🔴 **A rare-event miss is not a signal until you compute what chance predicts.**
- **Match the criterion to whether the hazard INTEGRATES or FIRES** (Athena's
  trigger fix). Saturation and queueing are different nouns.
- **An instrument cannot resolve a window finer than its cadence** — 10s sampler,
  microsecond window.
- **A rough-edge comment is a load-bearing specification.**
- ⚠️ **zsh:** `${PIPESTATUS[0]}` after a pipe is NOT the first command's status.
  I was burned twice today. Redirect to a file and check `$?`.

Prior laws stand in `assert-the-invariant-not-the-enumeration.md` (updated today).

## Box rules

- ⛔ **Never bare `cargo clean`** — takes `target/release`, Tom's #108 restart
  path. Reclaim is `rm -rf target/debug` ONLY; pair absence with a survival check.
- **#108 pin:** `target/release/aion` the BINARY = 58,245,680 bytes, sha256
  `f748956c…`; `target/release` the DIRECTORY = 1,427,144 KiB. Different things.
- Box shared by ≥5 seats; free space is never "mine". Declare detached runs to
  Athena (`dm:45cf420e-621d-467b-b96c-5bffb62c04f3`) before the process exists.
- Delegation: Opus subagents, **2–3 concurrent max** (budget). File-mutating
  agents one at a time, **NO WORKTREES** (Tom's order).
