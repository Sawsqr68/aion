# What is in the working trees that git is not tracking

**2026-08-19.** Tom asked for the trees to be rounded up. Removing a tree
destroys anything in it that was never committed, so this is the list of what
would go, written before anything is removed. Companion to
`WORKTREE-ROUNDUP-2026-08-19.md`.

## The one that mattered — already saved

**`wt-fix-77` held a complete, coherent, uncommitted change and nothing else in
the estate had a copy of it.** Uncommitted work is not in git objects at all, so
it existed in exactly one place: a directory under `/private/tmp`.

It is the beamr aion#85 patch witness — the `[patch.crates-io]` pointing at
Artemis's fix rev, the lockfile, the `mod` line, and a module whose whole job is
to take the address of `Process::take_jit_transfer`, a symbol that exists in no
published beamr.

Its purpose is worth restating because it is a good piece of work: a
version-mismatched `[patch.crates-io]` **fails silent**. Cargo resolves the
registry copy instead, every crate compiles clean, and a green comes out of a
build that never contained the fix. This module makes that failure loud — if the
patch ever drops, the module stops compiling and the build stops before it can
produce a comforting green.

Committed to `preserve/patch-witness-20260819` (`61c60941a`).

🔴 **That branch is not for merging.** The `[patch]` is an absolute path into
Tom's home directory and would break any other machine. The branch exists so the
work survives; the *technique* is the part worth keeping when the patch
eventually comes out.

## Everything else, by tree

All of it is gate logs, release artifacts, and held-execution markers. No other
source file, anywhere.

| Tree | Ahead | What is in it | Size |
|---|---:|---|---:|
| `wt-fix-77` | 1 | `.release/`, `.gate/`, gate logs (the source is now saved) | **67 MB** |
| `resume-adoption` | 0 | `gate-logs/waffles-collect/`, held-execution marker | 3.7 MB |
| `libsql-retirement` | 5 | red-first mutation logs + exits, verification logs | ~0.9 MB |
| `aion-wt-59-linux-green` | 0 | three gate-log directories | ~2 MB |
| `aion-wt-r3-budget` | 0 | Waffles verify logs + release digest | ~1.3 MB |
| `aion-wt-lane1-cancel` | 5 | eleven gate logs and run scripts | ~0.6 MB |
| `241-final-message` | 0 | cold re-gate logs, takes 1–3 | ~0.3 MB |
| `release-v0.17.0` | 0 | `gate-logs/release-v0.17.0/` | 0.6 MB |
| `wt-fix-75` | 0 | `gate-logs/fix-75/` | 0.7 MB |
| `28-dig` | 3 | held-execution marker | 4 KB |

Roughly **77 MB**, and `wt-fix-77`'s `.release/` is 67 MB of it.

## What I would do, and what needs a word

**Safe to remove now** — the seven trees that are 0 commits ahead and hold only
gate logs of runs that are already recorded elsewhere: `wt-fix-75`,
`aion-wt-59-linux-green`, `aion-wt-r3-budget`, `241-final-message`,
`release-v0.17.0`, `resume-adoption`, plus the six other 0-ahead trees that hold
nothing untracked at all (`12-payload-limits`, `15-mint-credentials`,
`25-liveness`, `retire-libsql`, `57-time-glue`, `land-stack`, `release-build`,
`supersession-record`).

**Wants a word first:**

- `aion-wt-lane1-cancel` — its five commits are merged into
  `integration-20260819`, but the eleven gate logs in it are the evidence for
  that work. If they are wanted in the record they should be committed
  somewhere before the tree goes.
- `libsql-retirement` — the red-first mutation logs with their exit codes are
  the proof that the retirement's refusal path was witnessed going red. The
  branch is superseded; the *evidence* may not be.
- The three `.aion-held-execution.json` markers (`28-dig`, `libsql-retirement`,
  `resume-adoption`) — these look like held workflow state rather than logs, and
  I have not opened them to find out. They should be read before they are
  deleted.

**Nothing has been removed.** This list is so that the decision is made with the
contents known rather than after they are gone.
