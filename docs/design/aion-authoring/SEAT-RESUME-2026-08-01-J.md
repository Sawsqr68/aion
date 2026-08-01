# 📍 SEAT RESUME — 2026-08-01 (J)

**Supersedes I. Read this first.** I am **Vesper Lynd**, session
`5849e0d8-4802-4869-8e0f-9f7fd187e198`. Sign `— Vesper`. **Fable is the MODEL I
run on, not my name.**

## State — nothing in flight, tree clean

```
origin/main = 76e03a14  (PUSHED, local == remote)
  76e03a14  docs(verify): the value-payload landing, and three corrections against me
  e944f25e  feat(awl): value route payloads lower on the direct path (B4)
  12f9a0ea  docs(seat): marker I
```
Working tree clean apart from the pre-existing untracked `.aion-*-20260731/`
artifact dirs. **No subagent is running.** The AWL implementer
(`a46aafc4e0cb7b421`) completed, was verified, and its work is landed.

## ✅ #7 AWL — 16 undeployable documents → 9

Value route payloads (`route out(<value>)`) lower on the direct path. Ratchet
91 → 98. Full detail in task #7; the short version of what remains:

| class | count | note |
|---|---|---|
| `parallel region` | 3 | dependency-parallel layers — NOT a keyword, arises from multi-dependency `after`, which is why flagship `dev_brief` hits it |
| `subflow` | 4 | the **last** construct at the driver's staging gate — `flow_shape_gap` is now only this check |
| `substeps` | 2 | `FnOrigin::SubStep` already exists; reference `emitter/subs.rs:20-60` |

🔴 **The standing hazard for the next unit, unchanged and load-bearing: THE
REFUSAL PINS ARE CONTRACTS, NOT STALE EXPECTATIONS.** Retiring one is legitimate
only when the BC-4 differential proves byte-identical trails against the Gleam
oracle. A fixture that cannot pass stays refusing and gets said out loud. This
is the trap that cost the #90 revert. **And check the POPULATION, not just the
ratchet** — a bar raised by authoring easy fixtures is a bar moved.

⚠️ **BC-4 has FOUR outcome buckets plus `EXPECTED_CANCELLED`:** succeeded,
failed, excluded-out-of-oracle, timer-parked. Missing one breaks both the
arithmetic and any population you derive. It cost me two wrong claims today.

## ✅ #129 CLOSED — and it is the better catch of the day

`EXCLUDED_OUT_OF_ORACLE` was an **enumeration** of the fixtures that happened to
exhibit the problem, not a derivation from the property causing it.
`send_signal` carries a bare signal wait, was left in the oracle, and flipped
Parked/Failed under load. Fixed with a test that computes the population from
the **parsed AST** and asserts set equality, watched failing in both directions.
Property: *bare signal wait AND no durable timer anywhere*. Must walk the AST —
`combined`'s `timeout 10m` is a per-attempt **activity** timeout, so a grep
clears it wrongly.

## 🔴 Method laws earned today — the expensive ones are all corrections to ME

- **A TOTAL THAT RECONCILES DOES NOT VALIDATE THE PARTITION.** My bucket split
  was wrong and still summed to 98, because every fixture was in the file. A
  cross-foot is evidence about the split only if a wrong split could break it.
  I was one step from "correcting" the implementer's right number to my wrong one.
- **I DERIVED A POPULATION FROM AN INCOMPLETE POPULATION** — swept 95 and said
  98 — *while writing up* assert-the-invariant-not-the-enumeration. Deriving
  from a property does not save you if the domain is an unchecked enumeration.
- **BEFORE ATTRIBUTING AN EVENT TO A CONDITION, MEASURE HOW OFTEN THE CONDITION
  HOLDS WITHOUT THE EVENT** (Athena). This box exceeds core count 61% of the
  time, so my "I contended it" could not have been falsified. Memory:
  `base-rate-before-attribution.md`.
- ★ **A CAREFULLY MEASURED ANSWER TO AN UNEXAMINED QUESTION IS STILL AN
  UNEXAMINED QUESTION** (Athena). 0-in-118 had **no power** against a 1/258 rate
  (excludes only faster than ~1/40; pooled MLE 1/376). **The bad number was
  mine.** There was never a quiet-vs-loaded difference to explain.
- **WHEN A PREMISE DISSOLVES, RULES BUILT ON IT MUST BE RE-DERIVED, NOT KEPT.**
  "Load is part of the oracle" survives as prudence; it is **not** citable as a
  #127 finding. The tempting move is keeping the law and dropping the footnote —
  that is how an unsupported claim acquires the standing of a settled one.
- **A demotion at the bottom of a top-down file is a demotion nobody reads.**
  Strike at the point of assertion; keep text visible-but-struck so the
  correction is *seen* rather than inferred (Athena's practice, adopted).
- **A gate that fires in only one direction matches everything** — hence the
  two-arm watch on #129.

## Board

| item | note |
|---|---|
| **#7** | 9 refusals left; next unit is `subflow` (4, and the last staging-gate construct) or `substeps` (2, cheapest) |
| **#128** | one-motion pipeline WIRED BUT UNGATED; `await_result` has NO timeout — hangs forever. Fix = honour the declared timeout, do NOT invent one |
| **#90** | reverted, stays OPEN — observation real, fix wrong; §5 of `RETRY-CLASS-HONESTY-90.md` has the corrected design |
| **#127** | routed to Apollo (his #74 + #75, land in ONE commit, neither implemented). ⚠️ my assertion at `2a33475e` names the CURRENT wrong class, so **our suite goes RED when his fix lands** — correct downstream behaviour, must be coordinated |
| **#85** | libsql 0.9.30 double-close UAF, verified in dep source; did NOT fire in ws6 |
| **#77 / #82** | Tom's standing lanes: tharsis, NIF-state P0 |

## Box rules

- ⛔ **Never bare `cargo clean`** — takes `target/release`, Tom's #108 restart
  path. Reclaim is `rm -rf target/debug` ONLY; pair absence with a survival check.
- **#108 pin:** `target/release/aion` the BINARY = 58,245,680 bytes, sha256
  `f748956c…`; `target/release` the DIRECTORY = 1,427,144 KiB. Different things.
- Box shared by ≥5 seats; free space is never "mine". Declare detached runs to
  Athena (`dm:45cf420e-621d-467b-b96c-5bffb62c04f3`) before the process exists.
- **My own cargo runs contend a subagent's.** Serialise, or expect its results
  to arrive slower and be tempted to blame load for them.
- Delegation: Opus subagents, **2–3 concurrent max** (budget). File-mutating
  agents one at a time, **NO WORKTREES** (Tom's order).
- Comms: Waffles DMs work (`dm:896955e1-…`) — the "takes no DMs" absolute was
  stale. Cally `dm:15a23351-…`. Tom by UUID `c9255b2a-…`, never bare name.
