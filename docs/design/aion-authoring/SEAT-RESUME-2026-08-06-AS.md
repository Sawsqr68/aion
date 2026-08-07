# SEAT RESUME — marker AS, 2026-08-06

Supersedes AR for anything they disagree on. **AR's AMENDMENT 3 is still the
authority on the #7 lane's internals; read it too.** Latest letter wins.

## 🔴 STATE AT THIS INSTANT

- **HEAD = `9189ba585` = origin/main** (fetched and verified by sha).
- **#7 IS BUILT, GREEN, AND UNCOMMITTED.** 13 tracked files modified + 1 rename
  + untracked `crates/aion/src/store_faults.rs` and three docs.
- **A second Opus review is IN FLIGHT** (agent `a60c43462fd771923`). CLAUDE.md
  mandates it. **DO NOT COMMIT UNTIL IT RETURNS AND ITS FINDINGS ARE HANDLED.**
  The first review found 3 CRITICAL + 6 MAJOR; two of its claims later failed
  measurement (see AR amendment 3).
- Tom's live pids **45425 (server) / 48399 (worker)** untouched all session.

## #7 — FINAL GATE RESULTS (this tree)

| gate | result |
|---|---|
| `cargo test -p aion-rs` | **59 verdict lines, 1087 passed, 0 failed, `TEST_EXIT=0`** |
| `cargo clippy -p aion-rs --all-targets -- -D warnings` | **`CLIPPY_EXIT=0`, 0 findings, NO `#[allow]` added** |
| 7 mutation controls | 6 DECISIVE, 1 UNDECISIVE-and-declared (C1) |

**An earlier suite run was RED** — `outbox_e2e::outbox_crash_recovery_rearms_stranded_done_row_and_finishes`,
`Backend("SQLite failure: bad parameter or other API misuse")`. Re-probed **5×
on the same tree: 5/5 green**. It is a flake, consistent with task #85 (libsql
0.9.30 double-close). ⚠️ That red run emitted only **39** verdict lines; the
green one emitted **59** — `cargo test` stops at the first failing target, so a
line count is not a suite pass. Read `TEST_EXIT=`.

Logs: `<scratchpad>/suite-7d.log`, `clippy-7e.log`, `outbox-flake.log`,
`mutctl/REPORT.txt`.

## 🔴 WAFFLES RULED (03:27Z) — completion-retry policy, BUILT

Option-2 structure **in this lane**: new `CompletionRetryConfig` (initial +
ceiling, unbounded by attempts), plumbed builder → `RuntimeConfig` →
`RuntimeHandle::completion_retry()`. Call site pins **the values the code
already ran: 1 ms / 8 ms**, verbatim, with the `Default` impl carrying a 🔴 note
that they are *inherited, not chosen*, and the 125-reads/sec figure. **Behaviour
is byte-for-byte unchanged, so no new control is required — his explicit word.**
He carries the "what should a durable-retry ceiling be" question to Tom with my
seconds-scale recommendation. **One holder, and it is him. Do not re-raise it.**

Also endorsed on the record: A5 structural-not-executed with no control claimed;
A2 pinning only its load-bearing half; filing the BC-4 nondeterminism rather
than fixing it in-lane.

---

# 🔴 THE REPO-HYGIENE SWEEP (Tom's directive via Waffles, 03:36Z)

Tom is angry that work is getting lost in worktrees and unmerged branches. Every
repo owner sweeps their own. **Tom's word, carried: the author of whatever the
sweep finds in aion is THIS SEAT. No external blame.**

## Instrument checks done FIRST (Waffles' law: known-answer before unknowns)

- `git config gc.auto` → **0** (Waffles set it estate-wide; verified at my hand).
- **Positive control on the patch-id instrument:** three commits known to be in
  main (`9189ba585`, `fdd694c4b`, `28a814b58`) each returned **exactly 1** hit in
  the main patch-id index. The instrument can see a landed commit.
- Estate diff driver strips +/- → every diff used `git -c diff.external= diff --no-ext-diff`.
- zsh does not word-split parameter expansion → every loop used a literal list
  or a piped `while read -r`.

## ✅ FINDING 1 — the dirty tree is GENUINE (frame treatment applied)

All **13** tracked dirty files blob-hashed against `HEAD`/main:
**13 GENUINELY-CHANGED, 0 identical-to-main.** This is not a stale-anchor
artifact. It is the #7 work, and it is safe to commit once the review clears.

## 🔴 FINDING 2 — SEVEN LOCAL BRANCHES ARE JUNE FOSSILS

Every local branch except one has merge-base **2026-06-04 21:11:49 +1000** (the
pre-history-scrub anchor) while its **same-named origin twin** is based Jul 11–30.
They are different branches wearing one name. Their ~870k-insertion three-dot
diffs are the merge-base artifact, **not live work — landing one would replay a
June tree over August main.**

| local branch | local base | origin base | origin twin vs main (patch-id) |
|---|---|---|---|
| `awl-bc1-capstone` | Jun 4 | Jul 11 | 3 commits, 3 unmerged — **LIVE** (11 files, 786+) |
| `brief/cargo-gates-verdict-truth` | Jun 4 | Jul 25 | 1 commit, 1 unmerged — **LIVE** (1 file, 92+) |
| `brief/delivery-contract-fix-wave` | Jun 4 | Jul 30 | 1 commit, 1 unmerged — **LIVE** (1 file, 155+) |
| `build/dead-man-switch` | Jun 4 | Jul 29 | **0 commits — HUSK** |
| `build/remote-gates` | Jun 4 | Jul 29 | **0 commits — HUSK** |
| `defect/beamr-jit-loses-native-services` | Jun 4 | Jul 30 | 3 commits, 3 unmerged — **LIVE** (6 files, 974+) |
| `fix/delivery-contract` | Jun 4 | Jul 30 | **0 commits — HUSK** |
| `fix/worker-shell-dial-and-audibility` | **Aug 5 — SAME as origin** | Aug 5 | 1 commit, 1 unmerged — **LIVE** (2 files, 101+/8−) |

**Recovery is guaranteed:** `/Users/tom/Developer/ablative/stack/aion-pre-scrub-backup-20260804.git`
contains all seven fossil branch names. Verified by listing its refs.

## 🔴 FINDING 3 — the worktree is pure frame-trap

`.worktrees/delivery-fix` is checked out on the June fossil `fix/delivery-contract`
@ `faeeb5f08` — content **already in main**. Its 7 "dirty" files
(`.meridian/profiles/*`, `.meridian/workflows/*/profiles/norn-agent.md`) are
**byte-identical to main**. Not work. Restored content in a stale checkout.
The worktree is disposable; nothing in it needs saving.

## ✅ FINDING 4 — 104 unreachable commits, ALL PINNED

`git fsck --unreachable` → 104 commits / 30869 blobs / 558 trees / 2 tags.
**All 104 pinned to `refs/rescue/20260806-<sha>`, PINNED=104 FAILED=0**, before
any filtering. Manifest `<scratchpad>/RESCUE-MANIFEST.txt`; classification
`<scratchpad>/RESCUE-CLASSIFIED.txt`; scripts `rescue.py`, `classify.py`.

Classified by **patch-id --stable against main's 2118 indexed patches** (content,
never sha ancestry — the history was rewritten, so *nothing* unreachable can be
an ancestor and `git cherry` inflates by construction):

- **REBASED-LANDED: 19**
- **EMPTY-OR-MERGE: 12**
- **NOT-IN-MAIN-BY-CONTENT: 73** ← ⚠️ **NOT YET AUDITED. THIS IS THE OPEN ITEM.**

**Do not report 73 as "lost".** 47 of the 73 are dated Jun 4–11, i.e. the same
pre-scrub layer; the scrub rewrote every commit that touched the ~600 MiB of
committed build artifacts, so a pre-scrub commit and its post-scrub twin have
**different patches by construction** and will never match by patch-id. The
per-commit audit must therefore compare against the **pre-scrub backup repo**,
not against main, for anything dated before 2026-08-04. The ~15 dated Jul 23 –
Aug 06 are the ones that could be genuinely lost and need looking at first.

## ⛔ BINDING SWEEP CONSTRAINTS

- **NO `git gc`, NO `git prune`, NO `--prune` fetch** until estate all-clear.
  Deleting refs is what triggers auto-gc; the sweep is the likeliest executioner
  of the work it was ordered to rescue. `gc.auto=0` is set but do not test it.
- **DELETE NOTHING YET** — not the fossils, not the husks — until the 73 are
  audited. Waffles' explicit sequencing.
- Never `git add -A`; never `git stash -u`. NO-SWEEP dirs stand.

## ORDERED NEXT STEPS

1. **Second Opus review returns → handle findings → commit → push #7.** Commit
   by explicit path list; message via `git commit -F -` (backticks).
2. Audit the **73 NOT-IN-MAIN-BY-CONTENT**, newest first, against the pre-scrub
   backup for the June layer. Classify REBASED-LANDED / SUPERSEDED /
   PRESERVED-BY-INTENT / GENUINELY-LOST.
3. Land the 5 LIVE origin branches through the gates, or name each real blocker
   with receipts. (`fix/worker-shell-dial-and-audibility` is task #142, held on
   the #139 cut trigger — that is a real, named blocker.)
4. Only then: delete the 7 local fossils and the husk refs.
5. Report to Waffles: found / landed (shas) / husk (deleted) / evidence
   (declared, not detected) / genuinely blocked (receipts).

## RECEIPT FOR CALLY'S #147 QUERY

`28a814b58` is `docs(seat): marker AQ amendment 3` and **is an ancestor of main**.
#147's code landed at `3d034c686` + `fdd694c4b`, docs at `28a814b58` +
`9189ba585`. The task is marked completed at my board. **#147 is shut** — the
discrepancy is in the tracker, not the repo.
