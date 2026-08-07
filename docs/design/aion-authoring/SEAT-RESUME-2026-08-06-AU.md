# SEAT RESUME — marker AU, 2026-08-06

Supersedes AT for STATE. AT still holds the #7 finding text and build detail.
Latest letter wins.

---

# PART 1 — CALLY'S SWEEP RECALL: ANSWERED, ROW HOLDS (re-measured, not re-asserted)

Owed: two coverage numbers · Q2 (bare `git diff`) · Q3 (lenient merge verdicts).

## Q1 — the two numbers

**Verdicts emitted: 104. Population stated over: 104. They match**, and that is a
counted result — `RESCUE-CLASSIFIED.txt` has one row per commit, tallying
12 EMPTY-OR-MERGE + 73 NOT-IN-MAIN-BY-CONTENT + 19 REBASED-LANDED = 104. No
commit fell through without a row.

## Q2 — bare `git diff`: EXEMPT BY CONSTRUCTION, and checked

`classify.py` and `rescue.py` use `git show | git patch-id --stable`, `git log`,
`git rev-parse`, `git reflog`, `git cat-file` — **there is no `git diff` in the
classifier at all**, bare or otherwise. So no zero line-count from a
difft-rendered bare diff exists anywhere in this row.

The two blob-identity claims in the sweep are used in their SOUND directions
only, which matters given the wrong-noun trap:
- finding 1 "13/13 blob-differ from main" claims the dirty tree CONTAINS real
  work. Differing blobs prove difference exists. Sound.
- finding 3 "7/7 byte-identical to main" claims the worktree contains NOTHING
  new. Identical blobs prove sameness. Sound.
Neither uses blob identity to claim CONTAINMENT of one change in another, which
is the direction that fails both ways.

## Q3 — lenient merge verdicts: THE LENIENT PATH HAS ZERO OCCUPANTS, MEASURED

Two facts, both counted rather than reasoned:

1. **Capture is first-line-only.** `fields = stdout.split(); pid = fields[0]` —
   one patch-id, then a dict lookup `pid in main_index`. Not `grep -qx` over a
   multi-line capture, so the any-parent-side match cannot occur.
2. **No merge was ever called LANDED.** Cross-tabulating class against parent
   count over all 104: all 19 REBASED-LANDED have **exactly one parent**. The
   merge population is 15 (13 two-parent + 1 three-parent octopus in
   NOT-IN-MAIN-BY-CONTENT, 1 two-parent in EMPTY-OR-MERGE). The lenient verdict
   is empty by measurement, not by argument.

Consistent with the index: `git log -p main` emits no diff for merges, so main's
merges are not in `main_index` and no merge could have matched.

## The gap Q3's spirit DID find, and it is now closed

The 12 EMPTY-OR-MERGE were the only rows that never went through the decisive
backup audit — the 73 did. So I ran that audit over **all 104**, not 73.

| class | present in backup | reachable from any backup ref |
|---|---|---|
| EMPTY-OR-MERGE (12) | 12 present | 0 reachable |
| NOT-IN-MAIN-BY-CONTENT (73) | 71 present, 2 ABSENT | 0 reachable |
| REBASED-LANDED (19) | 19 present | 0 reachable |

**Both ruler controls fired on this run**: a fabricated sha is absent from the
backup (must be False → False), and the backup's own `main` tip is in the
reachable set (must be True → True). So "present" and "reachable" are both
capable of coming back the other way.

**102 of 104 present-as-orphan and unreachable pre-scrub. The 2 ABSENT are dated
2026-08-06, AFTER the 2026-08-04 backup — this seat's own `wip-7` stash pair,
whose content is in the working tree now.** Verdict unchanged: **aion lost
nothing.** It is now established over the whole population instead of 73 of it.

## One artifact defect found while answering, recorded

`RESCUE-CLASSIFIED.txt`'s third column is headed `main-twin`, but for
EMPTY-OR-MERGE rows it holds the commit's OWN TREE sha, not a twin in main —
e.g. `2f12d661` (Merge NB-007) shows `7641f18b2c2a`, which is a tree object and
resolves to no commit. Anyone reading that column as "landed as" would be
misled. `classify.py` also computes a `found` variable in that branch and never
uses it. The twin column is only meaningful for REBASED-LANDED rows.

---

# PART 1b — 🔴 THE POPULATION WAS UNDER-SCOPED. 104 → 140. NOT YET CLOSED.

Cally's `--no-reflogs` warning lands on this row. Measured at my hand:

    git fsck --unreachable              → 0 commits
    git fsck --unreachable --no-reflogs → 36 commits

**36 of 36 are NOT in the pinned 104.** (The default now reports 0 because my own
`refs/rescue/*` pins made the original 104 reachable — so the default flag was
doubly blind here: reflog-as-root AND my own pins.)

✅ **All 36 PINNED: `refs/rescue/20260806b-<sha>`, PINNED=36 FAILED=0.** Total
rescue refs 140. Done FIRST because reflog is the weakest retention in git.

## First pass over the 36 (patch-id, with the instrument checks Cally asked for)

- **17 REBASED-LANDED** — content is in main under a new sha.
- **18 NOT-IN-MAIN-BY-CONTENT** — need the backup audit, exactly as the 73 did.
- **1 MERGE** (`bbe89c41`, "merge: L01 doc-truth sweep") — routed to PARENT
  CONTAINMENT, not patch-id, and remember INPUTS-LANDED ≠ LANDED.
- **Distinct-id check: 35 ids emitted, 35 DISTINCT.** No shared degenerate value,
  so the third instrument fault is absent here. No empty id outside the merge.

⛔ **DO NOT REPORT A VERDICT OVER THE 36 YET.** The 18 carry subjects that LOOK
landed (three copies of "a running workflow can be asked what it is doing"; the
#125 registry-outage fix) — that is the amend-cycle signature, but a signature
is not a measurement. **NEXT STEP: run the backup audit (present-as-orphan +
unreachable-from-any-ref, with both ruler controls) over the 18, and parent
containment over the merge.** Until then the honest statement is: *the aion
verdict is established over 104 of 140; 36 are newly surfaced, pinned, and
19 of them (18 + 1 merge) are unadjudicated.*

Artifact: `<scratchpad>/POP36-CLASSIFIED.txt`, `pop-new.txt`, `pop-noreflogs.txt`.

## Also owed back to Cally (small)

Narrow or blank `RESCUE-CLASSIFIED.txt`'s `main-twin` column for EMPTY-OR-MERGE
rows — it holds the commit's own TREE sha there, a value shaped exactly like a
commit id.

---

# PART 2 — #7 STATE

## GATES ON THE CURRENT TREE — ALL GREEN

- `cargo test -p aion-rs --lib` → **TEST_EXIT=0, 642 passed / 0 failed**
- `cargo test -p aion-rs` (all targets) → **TEST_AION_EXIT=0, 1099 passed /
  0 failed** across 59 result lines (`<scratchpad>/test-aion.log`)
- `cargo clippy --workspace --all-targets -- -D warnings` → **EXIT=0**
  (`<scratchpad>/clippy-ws2.log`)
- Mutation board **RE-RUN ON THE FINAL TREE** → **15 legs, FAILURES: 0,
  BOARD_EXIT=0**, both controls fired (noop SURVIVED, positive KILLED), and the
  four mutated files verified byte-identical afterwards by SHA-256 against a
  freshly regenerated snapshot. 13 real mutations, 13 KILLED.

## 🔴 THE ONE THING LEFT BEFORE COMMIT

**THIRD Opus review** — every gate is green on the final tree. Then commit + push.
Give the reviewer marker AT (findings + build detail), this marker, the diff, and
`<scratchpad>/mut7/REPORT.txt`.

## Everything built for #7 (all of it compiles, all of it is pinned)

C-1 · M-1 (three parts) · M-2 · M-3 (typed key + escalation ladder) · M-4 ·
M-5 (two pins, two links) · m-1 · m-2 · m-3 · m-4 · m-5 · m-6 (a REAL defect,
not a doc fix) · m-7 · m-8 · m-9. Nothing deferred. Detail in marker AT.

Test seams that had to be BUILT because the properties were unmeasurable:
`ProcessExitRegistry::force_shutdown_failure()` (`#[cfg(test)]`, in-pattern with
ten existing flags) · `FlakyStore::fail_reads_after(skip, count)` with
`fail_next_reads` becoming `skip = 0` (ONE budget) ·
`FlakyStore::fail_next_reads_with_lost_ownership` · `FaultReporter` extracted
from the retry loop so the re-log cadence has a witness other than a log line.

Clippy caught three real defects in my own work — a 108-line `build`, a doc
comment silently reassigned to the wrong item, and **`aion-server` failing to
COMPILE (E0004) on two non-exhaustive `EngineError` matches**. That last one is
the m-3 mechanism firing for real: the enums are not `#[non_exhaustive]`, so a
new variant is a compile failure at every consumer. `clippy -p aion-rs` was
green while it was broken — **the workspace run is not optional.**

## Standing constraints unchanged

NO WORKTREES · never `git add -A` · never `git stash -u` · NO `git gc`/`prune`
until the estate deletion hold lifts · DELETE NOTHING · Tom's pids 45425 / 48399
untouched all session · worker rebuilds frozen until the #139 live cut ·
`<<autonomous-loop-dynamic>>` wakeup must be re-armed.

## Then, in order

1. Land the 5 live origin branches through the gates (or name each blocker with
   receipts — `fix/worker-shell-dial-and-audibility` = #142, held on #139).
2. After the deletion hold lifts: 7 local fossils, 3 husk refs, frame-trap
   worktree.
3. Tom's remaining queue: #9, #6, #5 (bump `Keyword::ALL`'s `[Self; 57]` in the
   same change), #1.
