# The dev cycle, decision by decision

**What this is.** Tom's ask, 2026-08-14: *"every element of determinism we can
extract out … there are hundreds of commands in there that we would run as a
matter of course given certain conditions … we want to find each and every one of
those decisions and build our workflow around that."*

So this is the inventory. One row per decision that gets made while taking a
piece of work from "someone said it" to "it is on main and proven". For each:
what triggers it, what the rule is, and — the column that matters — **where the
rule lives today**.

## How to read the "lives where" column

| marker | meaning |
|---|---|
| **WORKFLOW** | already a step in a deployed AWL document. Done. |
| **SCRIPT** | in a shell script a workflow calls. Deterministic, but not visible as a step and not resumable on its own. |
| **WRITTEN** | stated in a document (CLAUDE.md, a runbook, a memory file). A person or agent has to read it and remember to apply it. |
| **HEAD** | nowhere. It lives in whoever is doing the work. **This is the column to empty.** |

A **HEAD** row is not a small thing. It means the decision is re-derived every
time, by a different agent, with a different amount of care, and nobody can tell
afterwards whether it was made at all.

## The shape of the extraction

Most of these are not "run a command". They are **a condition, and what follows
from it**. That is precisely what a workflow expresses and what a prompt does
not. The agent should be reached for where genuine judgement is required — is
this diagnosis right, is this code good, is this the real cause — and for
nothing else.

---

## Phase 0 — intake

| # | decision | the rule | lives where |
|---|---|---|---|
| 0.1 | Has this already been fixed by something that shipped? | Re-measure the blocker before doing the work it blocks. A task filed a week ago may name a defect four releases have since closed. | **WRITTEN** (a standing law; applied by hand, forgotten often) |
| 0.2 | Does a task for this already exist? | Search the board by the claim's vocabulary, not by its shape. | **HEAD** |
| 0.3 | Which brief / cluster owns it? | The brief is authoritative; if `design.json` disagrees with a brief, the brief wins. | **WRITTEN** (CLAUDE.md) |
| 0.4 | Is it a defect or a want? | A defect names a behaviour that is wrong today. A want names one that does not exist. Different evidence bars. | **HEAD** |
| 0.5 | Does it block anything, and is anything blocking it? | Dispatch order is foundation-first. | **WRITTEN** (CLAUDE.md dispatch order) |

## Phase 1 — before a single file is touched

Every row here is a **precondition**, and every one of them has produced a wasted
run when skipped.

| # | decision | the rule | lives where |
|---|---|---|---|
| 1.1 | Is the box quiet enough to measure on? | Refuse to start a battery when load exceeds core count. A contended red is not a red. | **SCRIPT** (the battery runner refuses; nothing refuses at dispatch time) |
| 1.2 | Is there disk headroom for a cold build? | A cold workspace build needs roughly 25–37 GiB. Below that, clean `target/` first — after checking no live process holds it. | **WRITTEN** |
| 1.3 | Is another agent already writing to this tree? | One mutating lane per tree. An unmarked dirty tree means stop and find out whose it is. | **WRITTEN** |
| 1.4 | Main tree, or an isolated worktree? | Concurrent writers get their own worktree. Always, not when it seems likely to collide. | **WRITTEN** (and repeatedly not done — this is what #221 is) |
| 1.5 | Is HEAD green right now? | You cannot attribute a new red without a green baseline. If HEAD is already red, that red must be named and dispositioned first. | **HEAD** |
| 1.6 | Which server/worker processes must not be touched? | Kill only recorded pids, never a pattern match. An actuator must sit outside what it fires into. | **WRITTEN** |

## Phase 2 — deciding what the change drags with it

This is the richest seam, and almost all of it is pure formula: *this file
changed, therefore that must be regenerated and this gate must run.*

| # | trigger | what must follow | lives where |
|---|---|---|---|
| 2.1 | any Rust change | fmt, both clippy resolutions, the workspace test suite | **WORKFLOW** (`gates.awl`) |
| 2.2 | a change to a wire type | regenerate the TypeScript bindings; check all four SDKs still agree | **SCRIPT** + **HEAD** (the "check all four" half is remembered, not enforced) |
| 2.3 | a console source change | rebuild the console; re-embed it into the server crate; diff the rebuild against the committed bundle | **WORKFLOW** (the embed freshness leg) |
| 2.4 | a new declared command | it must appear in `DECLARED-COMMANDS.md` | **WORKFLOW** (a gate catches an undeclared one) |
| 2.5 | an AWL grammar or checker change | the example corpus must still check; the guide reference must still match the grammar | **WORKFLOW** (corpus + grammar-parser gates) |
| 2.6 | a change to the gate battery itself | the battery gates its own worker crate — its own instrument | **WORKFLOW** |
| 2.7 | a public API change | which SDKs, which docs, which examples reference it? | **HEAD** |
| 2.8 | a dependency bump | which crates recompile, and does any pinned behaviour depend on the old version? | **HEAD** |
| 2.9 | a change that adds to a closure | the battery widens to the closure's shape, not the diff's | **WRITTEN** (a standing law) |

## Phase 3 — proving it

| # | decision | the rule | lives where |
|---|---|---|---|
| 3.1 | Which gates does this change require? | See phase 2 — derived from what was touched. | **HEAD** (nothing computes the set from the diff) |
| 3.2 | Both clippy resolutions? | Always both. `--workspace` unifies features one way and `-p <crate>` another; they are two different programs. | **WORKFLOW** (two separate legs) |
| 3.3 | Has the new test been seen failing? | A gate that has never been red proves nothing. Land it red first, or mutate and watch it die. | **WRITTEN** |
| 3.4 | Does the mutation sit on the test's path? | An off-path mutation gives a pass that measured nothing. | **WRITTEN** |
| 3.5 | Is the mutation the actual prior text? | Use the literal bytes that were there before the fix, not a plausible reconstruction. A non-compiling mutation is a non-run. | **WRITTEN** |
| 3.6 | How is the mutation restored? | By writing bytes back, then rebuilding. A move-and-restore leaves a stale mutant binary. | **WRITTEN** |
| 3.7 | Did every leg actually run? | Reconcile the count of verdict lines against the count of binaries. A signal-killed binary emits no verdict at all. | **WORKFLOW** (per-leg exit codes + coverage floors) |
| 3.8 | If a leg is red, why? | The failing targets and the whole failure text, uncapped, separate from any tail. | **WORKFLOW** (as of #236) |
| 3.9 | Is the red mine, or pre-existing? | Compare against the baseline from 1.5. | **HEAD** |
| 3.10 | Is a known red allowed to ship? | It must be named, attributed to a filed defect, and carry a disposition. | **WRITTEN** |

## Phase 4 — landing

| # | decision | the rule | lives where |
|---|---|---|---|
| 4.1 | Format before commit | Format the whole tree and write; never format-check. | **WRITTEN** |
| 4.2 | Commit message mechanics | Via stdin, because the shell eats backticked words from an inline message. | **WRITTEN** |
| 4.3 | Branch or main? | Merges to main route through review. | **WRITTEN** |
| 4.4 | Does this need a changelog entry? | Anything an operator would notice. | **HEAD** |
| 4.5 | Does this need a version bump? | Anything published. | **HEAD** |
| 4.6 | Which task does this close, and what evidence closes it? | Every close names the commit and the measurement. | **WRITTEN** |
| 4.7 | Push, or hold? | Held branches are held for a reason and the reason is written down. | **WRITTEN** |

## Phase 5 — release

| # | decision | the rule | lives where |
|---|---|---|---|
| 5.1 | Publish order | Dependency order across the crate family. | **SCRIPT** |
| 5.2 | Did every crate actually reach the registry? | Verify against the registry, not against the publish command's exit code. | **SCRIPT** |
| 5.3 | Where does the tag go? | On the cut commit, not on branch head. | **WRITTEN** |
| 5.4 | What does the tag message have to say? | Every battery red, with its disposition. | **WRITTEN** |
| 5.5 | How is a live binary swapped? | By rename, never by copy — a copy truncates the running image. Check what holds it first. | **WRITTEN** |
| 5.6 | How is the new binary identified? | By content hash, never by the version string it prints. | **WRITTEN** |
| 5.7 | What proves the swap worked? | A ladder: versions listed, worker re-registered, a real run completed, the served bundle matching the embedded one. | **WRITTEN** (run by hand each time) |

---

## What the inventory says

Counted, not estimated — 44 rows above:

| lives where | count |
|---|---|
| **WORKFLOW** | 8 |
| **WRITTEN** | 23 |
| **HEAD** | 9 |
| **SCRIPT** | 3 |
| split (script + head) | 1 |

**Eight of forty-four decisions are in a workflow. Nine are nowhere but in the
head of whoever is working, and twenty-three rest on someone remembering to read
a document.**

The eight that are in a workflow are all in *one* place — the gate battery.
Everything before it (can I even start?) and everything after it (what does this
drag with it, what closes it, what ships) is script, prose, or memory.

That is the actual finding: **we automated the middle of the cycle and left both
ends manual.** And both ends are where the expensive mistakes happen — starting a
four-hour battery on a contended box, or shipping a binary identified by the
version string it prints rather than by its bytes.

## The four that would pay for themselves first

1. **Phase 1 as a preflight workflow (1.1–1.6).** Six conditions, all mechanical,
   all currently checked by memory. A run that cannot produce a trustworthy
   measurement should refuse to start, loudly, rather than spend four hours
   producing one nobody can use.

2. **Phase 2 derived from the diff (2.1–2.9).** "These files changed, therefore
   these gates and these regenerations" is a pure function of the diff. Today an
   agent works it out each time and sometimes works it out wrong. This is the
   single biggest block of formula in the whole cycle.

3. **The green baseline (1.5 + 3.9).** Attribution is impossible without it and
   it is entirely mechanical: what did HEAD measure last time, and was it clean.

4. **The swap ladder (5.5–5.7).** It is a fixed sequence with a fixed set of
   proofs, run by hand every release, on the one operation where a mistake takes
   the live server down.

## What must stay with the agent

Not everything here is a formula, and pretending otherwise would be its own
defect. These need judgement and should keep it:

- 0.4 — is this a defect or a want (reading intent from a report)
- 3.9 — is *this particular* red mine (when the baseline is ambiguous)
- 4.4 — would an operator notice this change
- Anything asking "is this the real cause" or "is this code good"

The goal is not to remove the agent. It is to stop spending the agent on
arithmetic, so that when it *is* reached for, it is reached for the thing only it
can do.
