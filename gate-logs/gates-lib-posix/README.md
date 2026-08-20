# Gate record — `scripts/gates/lib.sh` sources under sh, and restores glob state

## Verdict — 24 declared, 24 executed, 24 PASS

Battery run 2026-08-20 at frozen HEAD **`2a7ed1eb2688ea112aa24e4b730afb6afe1f2f3f`**,
parent **`43ef592af`**, in `.worktrees/gates-lib-posix`. Runner:
`bash scripts/gates/battery.sh`, the repository's own runner invoked its own way
— not a hand-rolled reproduction. Elapsed 51m25s. Full log: `battery.log`.

```
0 of 24 gates failed.
```

**Declaration verified**, which the runner correctly refuses to claim for itself
(*"NOT VERIFIED YET … that they are the legs gates.json declares is a separate
claim"*). Ran `scripts/gates/verify-declaration.sh` against this log:

```
declared    : 24 legs
executed    : 24 legs
VERDICT: exact match — all 24 declared legs executed, and nothing else.
```

`rc 0`. The verifier states its own limit and it is repeated here rather than
dropped: *"names only: this says every leg RAN, not that any leg TESTS what it
is named after."*

## Test totals

628 `test result:` lines, **11,325 passed, 0 failed**.

**Zero ignored, everywhere** — no `test result:` line carries a non-zero
`ignored` count. **No runtime-gated skip emissions**: no `SKIP:`, `skipping`,
`skipped because`, or `gate skipped` line anywhere in the log.

⚠️ A naive `grep -ci skip` on this log returns **75**, and every one of those is
a **test name containing the word "skip"**, all reporting `... ok` — e.g.
`schedule::policy::tests::skip_policy_skips_when_execution_is_running`. The
count is recorded here with its explanation so nobody re-derives it and reads 75
skipped tests. Counting a word is not counting the thing the word names.

## ⚠️ Header reconciliation — 25 banners, 24 legs

The log carries **25** lines matching `^=== `. The 25th, at line 17669, is
`=== must-be-claimed gate ===`, printed by `check_must_be_claimed.sh` **inside**
the `estate-guard` leg — a child banner, not a leg. Same class as the 630-vs-628
reconciliation recorded under `gate-logs/94-reconcile/`. The manifest and
`EXECUTED_LEGS` both list exactly 24, and the declaration verifier agrees.

## What was gated

One file, 58 insertions / 4 deletions:

```
scripts/gates/lib.sh | 62 ++++++++++++++++++++++++++++++++++++++++++++++++----
```

Two commits:

- `43ef592af` — `lib.sh` sources cleanly under `sh`: process substitution removed.
- `2a7ed1eb2` — `select_python3` **restores the caller's glob state instead of
  forcing it on**. The prior code did `set -f` to split `$PATH` safely and then
  unconditionally `set +f`, so a caller that had globbing OFF got it switched ON
  underneath. That is a permissive-direction defect in a library every gate
  sources: it fails open, silently, in the caller's shell rather than in this
  function.

`select_python3` chose `/opt/homebrew/bin/python3` in this run — recorded because
**presence on PATH is not executability** on this box, and the choice is part of
what the gate ran, not an incidental.

## ⚠️ The base moved under this branch — stated, not papered over

Merge base with `origin/main` is `dfea2ec83`. `origin/main` has since advanced to
`cdde313b3` (PR #106, aion#94). **This gate therefore attests a tree that does
not contain #106**, and #106's tests have never run under the changed `lib.sh`.

Not re-gated, for reasons given rather than assumed:

- The branch's whole diff is **one shell file**. It adds no Rust, so it cannot
  break #106's compile.
- #106's diff is `crates/` and `gate-logs/`. It contains no shell, so it cannot
  affect what this change does.
- The changed function selects a Python interpreter and restores a shell flag.
  It has no dependence on the contents of the Rust tree.
- `origin/main` carried its own green battery at #106's landing.

**Landing by MERGE, never rebase.** A rebase mints new shas and would void this
attestation, which names `(2a7ed1eb2, parent 43ef592af)`. The merge commit
preserves the gated commit in history with its parent intact.

## The record commit sits ON TOP of the gated commit

This file is added after the gate ran, so the PR head is not the gated commit.
That is stated rather than hidden. **"Docs-only" is a build-graph claim and it
was settled, not asserted**: no leg in `gates.json` mentions `gate-logs` (checked
by parsing the declaration; positive control — the same search finds `crates` in
`rust:preserve-order-pins`, so the search works and the zero is a true negative).

## What this gate does not say

It does not say the estate is green. `estate-guard` passed and its own output
says why that is narrower than it looks: *"⛔ AND IT SAYS NOTHING ABOUT THE
REPOSITORY YOU ARE LANDING IN unless that repository is on the list. The
population is the list; this battery was only the clock."*

Separately measured the same night and recorded outside this branch: `liminal`
and `haematite` have no runner of their own, so their `estate-guard` leg refuses
`rc 3` on every run — the fix that landed here, in `frame`, and in `fulcrum`
landed in bash runners those two repositories do not have. `apps/meridian` is
still on the pre-2026-08-20 hardcoded-path shape. All three fail closed and
loud; none is a false green. Ruled by Waffles the Terrible: `gates.py` should
resolve and export the root itself, and an unset declared `environment` var
should become a runner refusal — one brief, deliberate, not tonight.
