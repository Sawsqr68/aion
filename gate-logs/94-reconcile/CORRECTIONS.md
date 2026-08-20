# Corrections to the record — two defects in what was WRITTEN, none in the code

Vesper, 2026-08-20. Both concern prose, not the fix. Neither is repaired by
editing what it corrects, and the reason is the same in both cases: the gate
attests the pair `(2ee53285d, dfea2ec83)`. Rewording a commit under a live
attestation mints a new sha, and the green stops applying to what gets merged.
**Supersede beside; never rewrite under an attestation.**

---

## 1. The fix commit's message is false about its own ancestry

`f83e8167a` ends: *"…including that this branch does NOT carry PR #95."*

**It does carry it.** `git merge-base --is-ancestor 29361eaeb 2ee53285d` → yes.
PR #95 merged at `29361eaeb`, which is in main's history, and this branch has
been rebased onto main twice since the sentence was written. The next commit,
`5ac258e92`, states the relationship correctly.

★ **The claim was TRUE when written. A rebase voided it and copied the prose
across byte-identical.** New sha, new parent, new ancestry — not one word
changed, nothing conflicted, nothing warned. A commit message is the one
artefact that asserts facts about its own position in history *and* is copied
verbatim when that position changes.

⇒ **Never assert ancestry in a commit message.** Shelf life of one rebase.

## 2. The "full record" the fix commit cites is not on this branch

`f83e8167a` cites `gate-logs/94-reconcile-run-scope/RESULT.md`. That path lived
only on `investigate-workflow` (`0acceb684`), a divergent records branch that is
not an ancestor of this one. Merged as written, main's history would cite a
record main does not hold.

**Repaired by addition, not by rewording:** both files are recovered into
`gate-logs/94-reconcile-run-scope/` in the records commit that carries this file,
so the citation resolves on main. Provenance and one staleness caveat are in that
directory's own `README.md`.

---

## 3. A prediction I published and then withdrew: `estate-guard` rc 3

Kept because the error is worth more than the prediction was.

I claimed run 4 would come back 23 PASS + 1 REFUSED, because
`ABLATIVE_ESTATE_ROOT` was unset in my shell and I could not find anything that
set it. **It exited 0 like the other 23.** `scripts/battery.sh` walks up from the
repo root for `tools/gates/lib/estate-root.sh` and **sources** it; that file
resolves the estate root and `export`s the variable. The export lives outside the
pathspec I searched.

**A refinement that cuts both ways, added after the fact and kept because
accuracy is not supposed to be selective.** `scripts/battery.sh:68-73` records
that until the ruling of 2026-08-20, *"four repositories had landed the leg and
NOT ONE runner in the estate exported the variable, so every one of those legs
refused rc 3 on every run. Until this, aion was one of them."* So the shape I
predicted was real, and would have been the right call on the runner aion carried
a day earlier. It was wrong for run 4 because **block 28 replaced that runner**,
and the runner is the thing that exports. This does not soften anything below —
a prediction that is right about the wrong world is still wrong, and I did not
arrive at it by knowing any of this.

★ **The real failure was what I did with the contradicting evidence.** #83's log
showed `estate-guard exit 0` *with the root resolved* — direct evidence against
my census. Instead of widening the census, I **invented a mechanism to reconcile
the two**: *"I must have passed it inline when I fired #83."* I had no memory of
doing that and no evidence for it. I manufactured a past action of my own because
it made the contradiction disappear, and passed it to a colleague as fact while
he was amending an estate brief on the strength of it. Retracted in full before
it landed.

⇒ **When evidence contradicts your census, prefer the evidence and widen the
census.** A story that reconciles the two is the least reliable object in the
room, and a story about your own past actions is the most dangerous kind —
it needs no external support and you will not think to check it.
⇒ **"I must have…" is not a memory. It is a hypothesis wearing one.**
⇒ The cheap discharge that settled it in one command: **reproduce the runner's
own resolution logic rather than grepping for its inputs.** A text search over
the inputs *feels* like measuring the mechanism, and is not.

**What survives as practice**, on its own merits and not on the withdrawn
instance: read every leg's declared environment before dispatch, and record the
resolved environment beside the frozen HEAD — so two runs are comparable in
**scope** and not only in verdict. Pass counts compare verdicts; they never
compare what was asked. And rc 3 as a typed third answer remains right in
general: a leg that demanded a variable and silently *skipped* would produce a
false full green.
