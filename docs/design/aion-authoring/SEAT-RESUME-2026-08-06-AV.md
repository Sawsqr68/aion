# SEAT RESUME — marker AV, 2026-08-06

Supersedes AU for STATE. AU holds the Q1/Q2/Q3 answers over the first 104;
AT holds the #7 finding text and build detail. Latest letter wins.

---

# PART 1 — THE AION SWEEP ROW IS COMPLETE: 140 OF 140, NOTHING LOST

The population was 104 and was wrong. Cally's `--no-reflogs` finding surfaced
36 more; all 36 were pinned first (`refs/rescue/20260806b-<sha>`, PINNED=36
FAILED=0, 140 rescue refs total) because reflog is the weakest retention in git
and the estate deletion hold does not protect it from expiry.

## The 36, adjudicated

| class | n | how it was settled |
|---|---|---|
| REBASED-LANDED | 17 | patch-id twin in main |
| CONTENT-ON-A-LIVE-REF | 7 | patch-id twin on a live origin branch |
| NOT-ON-ANY-LIVE-REF | 11 | present-as-orphan in the pre-scrub backup |
| MERGE | 1 | parent containment, by content |

**Verdict: aion lost nothing, established over the whole 140.**

## Artifacts

`<scratchpad>/AUDIT-19.txt` · `MERGE-CONTAINMENT.txt` · `LIVE-CONTAINMENT.txt`
· scripts `audit19.py`, `merge_containment.py`, `live_containment.py`
· `POP36-CLASSIFIED.txt`, `pop18.txt`.

## 🔴 THE DEFECT IN MY OWN FIRST PASS: THE DENOMINATOR WAS `main`

`classify.py` indexed **main alone**. Its NOT-IN-MAIN-BY-CONTENT class is a true
statement and the **wrong denominator for the question the sweep asks**, which is
*is this work lost* — not *is this work in main*. Work sitting on a live feature
branch is not in main and is not lost. Re-running containment over **every live
ref** moved 7 of the 18 out of the suspicious class:

- 5 → `origin/spike/norn-self-compaction` · 1 → `origin/docs/accessible-documentation`
- 1 → `origin/juniper/docs-package`

Two instruments agree on exactly the same 7 (backup-graph reachability, and
live-ref patch-id containment). Rescue pins were **excluded** from that index —
indexing them would make every pinned commit "contained" by the act of pinning
it, which is the same self-erasing trap that hid this population to begin with.

## 🔴 SHA ANCESTRY IS DEAD HERE, AND IT LOOKS ALIVE

main's history was rewritten (637 → 33 MiB). Every pre-scrub sha therefore fails
`merge-base --is-ancestor` against today's main **by construction**, so that
instrument answers "not in main" for work that certainly landed. It is not a
blind instrument — `HEAD~1` returns rc=0, it *can* say yes — which is exactly
what makes it dangerous: it looks like it is working. The control that exposes
it is a commit this sweep had already classified REBASED-LANDED: content in
main, and rc=1 from is-ancestor.

Containment therefore goes to patch-id, everywhere, always.

## 🔴 AND I FIRED THE PIPELINE-TAIL TRAP ON MYSELF, MID-ROW

I printed `backup main tip is-ancestor rc=0` from
`git merge-base --is-ancestor A B 2>&1 | head -2; echo $?` — that is `head`'s
exit code, not git's. The true value is 1. **Nothing caught it but arithmetic**:
the parents were ancestors of the backup tip, the backup tip read as an ancestor
of main, and the parents read as *not* ancestors of main. Transitivity cannot
hold all three, so one of them was a lie. The law already existed; the guard that
found the breach was a consistency check I happened to run, not the law.

## The merge, settled by content

`bbe89c41` "merge: L01 doc-truth sweep". `git show` emits no diff for a merge, so
its patch-id is empty — that is precisely how a merge slips past a patch-id
classifier looking like it has no content. Measured in three separate parts, and
**INPUTS-LANDED reported separately from LANDED, never folded into it**:

1. **INPUTS-LANDED 2/2** — both parents' content is in main
   (`8295d402`→`876f5d13`, `ea4fd227`→`aadcb020`).
2. **Its own resolution** touches exactly three files
   (`CLAUDE.md`, `README.md`, `COMPONENT-ARCHITECTURE.md`).
3. **The resolution landed** — two of the three are byte-identical to main
   today; `README.md` differs from later edits, and its distinctive resolution
   text ("retryable failures (provider flakes…") is present in main's README,
   while the `aion-nif` row the merge deleted is absent. Fabricated-phrase
   control returned 0.

## The 11 orphans: the amend-cycle signature, now measured

Cally flagged that three copies of "a running workflow can be asked what it is
doing" and the #125 registry-outage fix *look* landed, and that a signature is
not a measurement. Measured: **11 of 11 subjects are present in main**
(fabricated-subject control = 0). Their exact patches are not, which is the
amend-cycle signature — earlier drafts of work that landed under a rewritten
patch. Stated at its true strength: **a subject match is weak evidence**; the
decisive fact for these 11 is that they are present-as-orphan in the pre-scrub
backup and pinned in `refs/rescue`.

## PART 1c — RESCUE PINS PUSHED: 139 of 140, one hard-blocked by the remote

Ruled jointly by Cally and Waffles (the pins were local-only, single-copy).
Pushed with the safe form only — no `--force`, `--mirror`, `--prune`, `--delete`.

    local  refs/rescue/: 140      remote refs/rescue/: 139
    CONTROL remote refs/heads/: 14 (nonzero)
    object-for-object, sha+refname sorted: 139 MATCHED, 0 REMOTE-ONLY,
    1 LOCAL-ONLY — and it is exactly the pin predicted to fail.

🔴 **THE BLOCKER IS THE SCRUB ITSELF.** The first attempt was rejected wholesale,
`pre-receive hook declined` on all 140:

    remote: error: File docs/design/aion-authoring/ASSISTANT-AWL-PORT.md is
            105.52 MB; this exceeds GitHub's file size limit of 100.00 MB

aion's history was rewritten 637 → 33 MiB precisely to shed this class of object.
**A rescue pin points at PRE-SCRUB history, so pushing pins pushes back what the
scrub removed — and the remote is the only party that refuses.** It fails
ATOMICALLY: one oversize blob rejects all 140 refs, so the naive reading is
"pins cannot be pushed" when 139 were always fine.

Measured, not assumed: exactly **one** blob in the object store exceeds 100 MB
(`ee82d2f7`, 105.5 MB) and exactly **one** pin reaches it in its object closure —
`refs/rescue/20260806b-3f8d28d5…` *"docs(assistant): deploy is a one-way door"*.
The predicate was object-closure containment (`rev-list --objects`), the same
predicate the push itself uses, not a filename guess.

⚠️ That pin is **not** single-copy: it is one of the 11 and the audit has it
PRESENT-AND-UNREACHABLE in the pre-scrub backup.

**RULED (Cally, 2026-08-06): leave the 140th exactly where it is.** Deleting it
or moving it to LFS is method work and it changes what the remote holds. The
residual is named honestly rather than dismissed: **two copies on one machine is
a weaker fact than it sounds, because the failure mode that takes one takes
both** — the correct remedy is an OFF-BOX copy, so it folds into Tom's existing
off-box-mirror docket item instead of becoming a new decision.

Cally verified the push independently (140 local / 139 remote / control 14) and
measured the warning's real population: **beamr had already pushed clean 79/79
and haematite has 0 blobs over 100 MiB (largest 9.94 MiB)** — so the atomic-push
hazard applies to aion alone, not to all three history-rewritten repos.

🔴 **THE GENERAL LAW, banked estate-wide: AN ATOMIC FAILURE MISATTRIBUTES ITS
CAUSE TO THE WHOLE POPULATION.** One oversize blob rejected all 140 refs, and the
honest-looking reading of that rejection — *"rescue pins cannot be pushed"* — is
wrong about 139 of them. A failure whose blast radius is the batch tells you
nothing about how many members caused it. The cure is a predicate that names the
specific member BEFORE the measurement; "something will probably fail" is not a
test.

## Still owed (small, from AU)

Narrow or blank `RESCUE-CLASSIFIED.txt`'s `main-twin` column for EMPTY-OR-MERGE
rows — it holds the commit's own TREE sha there, a value shaped exactly like a
commit id. Its class name should also read NOT-IN-MAIN-BY-CONTENT-**OR-BRANCH**
or the denominator defect above will be re-read by whoever opens the file next.

---

# PART 2 — #7 STATE (unchanged from AU)

## GATES ON THE CURRENT TREE — ALL GREEN

- `cargo test -p aion-rs --lib` → **TEST_EXIT=0, 642 passed / 0 failed**
- `cargo test -p aion-rs` (all targets) → **TEST_AION_EXIT=0, 1099 passed /
  0 failed** across 59 result lines
- `cargo clippy --workspace --all-targets -- -D warnings` → **EXIT=0**
- Mutation board re-run on the final tree → **15 legs, FAILURES: 0,
  BOARD_EXIT=0**; 13 real mutations, 13 KILLED; both controls fired; four
  mutated files verified byte-identical afterwards by SHA-256.

## 🔴 THE ONE THING LEFT BEFORE COMMIT

**THIRD Opus review.** Give the reviewer marker AT (findings + build detail),
marker AU, this marker, the diff, and `<scratchpad>/mut7/REPORT.txt`.
Then commit + push.

## Standing constraints unchanged

NO WORKTREES · never `git add -A` · never `git stash -u` · NO `git gc`/`prune`
until the estate deletion hold lifts · DELETE NOTHING · Tom's pids 45425 / 48399
untouched · worker rebuilds frozen until the #139 live cut ·
`<<autonomous-loop-dynamic>>` wakeup must be re-armed every tick.

## Then, in order

1. Land the 5 live origin branches through the gates (or name each blocker with
   receipts — `fix/worker-shell-dial-and-audibility` = #142, held on #139).
2. After the deletion hold lifts: 7 local fossils, 3 husk refs, frame-trap
   worktree.
3. Tom's remaining queue: #9, #6, #5 (bump `Keyword::ALL`'s `[Self; 57]` in the
   same change), #1.
