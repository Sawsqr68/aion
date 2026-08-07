# Seat resume — marker BO, 7 August 2026

Supersedes BN. **Latest letter wins.** BN holds the F1–F14 disposition table,
the audit that found both defects, and the NAMED-NOT-FIXED list — still the
detail record for the #7 lane, still worth reading. This marker carries what
changed since.

`HEAD` = `9189ba585`. Working tree carries the whole #7 lane, **uncommitted**.
Tom's pids **45425** (server) and **48399** (worker) alive throughout, 1d 16h
uptime, `/health/live` 200 at every checkpoint. Disk recovered to **145 GiB**
free.

## 🔴 ALL THREE GATES GREEN — reconciled within one run

| Gate | Result |
|---|---|
| `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0**, zero error lines |
| `cargo test -p aion-rs --no-fail-fast` | **exit 0**, 60 verdict lines, **1115 passed / 0 failed** |
| `cargo test --workspace --no-fail-fast` | **RC_WS=0**, **4203 passed / 0 failed / 0 ignored** |

**Reconciliation, all inside the workspace run** — `Running` lines **223** =
declared binaries **223**. Verdict lines **247**, and the surplus is fully
explained rather than waved at: 223 binaries + 20 `Doc-tests` headers = 243,
plus **4** accounted for individually — three are `config::load::home_tests::
aion_home_env_probe_child` re-invoking its own binary with a filter (`796
filtered out`), one is the `compile fail` doctest running as its own
invocation. Zero `SIGSEGV` / `API misuse` / `error: test failed` strings.
**#85 did not fire this run** — it is contention-dependent, and its absence
here is not evidence it is fixed.

Logs: `scratchpad/final-workspace.log`, `final-declared.txt`,
`final-clippy.log`, `final-aion-rs.log`.

## 🔴 #146 TUTORIAL — READY, AND TOM HAS BEEN TOLD

Re-proven end to end **today** on the installed `aion 0.11.0`, isolated world
`:8097`/`:50097`, torn down and verified free **by port**. Full crib sheet with
the measured result of every step:
**`docs/design/aion-authoring/TUTORIAL-WALK-CRIB-2026-08-07.md`** — read that
before walking it with him.

**The one hazard found, and told to him rather than quietly patched:** the
deck's `aion.toml` sets no authoring root, so it defaults to `~/.aion/authoring`
— the same directory his live server uses, holding `assistant.awl`,
`assistant_spike.awl`, `git_status_sweep.awl`. Cure is one word:
`AION_HOME=~/my-first-flow/aion-home aion server --config aion.toml`. Verified
that a fresh empty `AION_HOME` resolves its own authoring root and the console
still serves 200 with no `config.toml` at all. **That is the deck's first fix
AFTER the walk** — walking it is what the fix list is for.

`:8099`/`:50099` deliberately left free for him.

## Measurement notes earned this tick

- **A god-file finding that did not survive its own second measurement.**
  `handle.rs` first measured **519** code lines — over CLAUDE.md's 500 — and I
  was one step from refactoring it. Re-measured excluding `#[cfg(test)]` items,
  which do not exist in a production build: **402**. The first pass had counted
  117 lines of test-support helpers as production code. Compliant, no change
  needed. Full sweep of all 25 lane files: **one** violation, and it is not
  this lane's — see below.
- **An empty kill target wrote a confident success line.** My first teardown
  printed `world removed: yes` while the dry-run server was still listening,
  because the `pgrep` pattern matched nothing (the processes were launched with
  a relative `--config` after a `cd`, so their command lines never contained the
  path). Then the retry failed silently again because **zsh does not word-split
  unquoted expansions** — `kill` got `"79662 "` with a trailing space. Cure both
  times: target the identity that actually distinguishes the thing (**the
  listening port**) and read the receipt back from that same identity.
  `an-actuator-must-sit-outside-what-it-fires-into`, twice in one tick.

## 🆕 Pre-existing, NOT this lane's to fix

**`crates/aion-server/src/error.rs` is 567 production code lines** — over the
500 ceiling. It was **563 at HEAD**; this lane's F4 fix added 4. Reporting it
rather than folding a split into the #7 lane, which would widen a lane already
under review. Needs its own row.

## ON RESUME — DO THIS FIRST

1. **Review 13 is re-dispatched** (the first attempt died on a weekly API
   limit, not on a finding). Read its verdict. Reviews 6–12 have **ALL
   REFUSED** — a PASS here would be the first.
2. On a PASS only: **re-derive** `scratchpad/commit-manifest-7-lane.txt` from
   `git status` (derived, never recalled), keep
   `docs/design/aion-authoring/SPEC-5-AWL-SUM-COMBINATOR.md` **excluded** (1.1 MB
   of raw agent JSONL under a `.md` path; issue #5's artefact; NOT deleted), and
   commit with paths named **explicitly**. ⛔ Never `git add -A` in this tree.
3. Any code change means a **new tree** means a **new battery** — the green
   above does not survive an edit.
4. Then: Tom's tutorial walk (live, he is ready), then #5, #1, #6, #9.

## Owed to Tom, in plain words

Reviews 6 through 12 all refused. That is the reason the lane is not committed
yet — not a failure to report, the reason. Say it plainly when he asks.

## Standing constraints (unchanged)

⛔ Never `git add -A`; name paths. ⛔ Never bare `cargo clean`. ⛔ No worktrees
in aion; one mutating lane. ⛔ Never publish artifacts for Tom — deliverables
are local files he opens. ⛔ Do not edit `CLAUDE.md`. Never mention prospekt.
🔴 Worker rebuilds frozen estate-wide until the #139 cut. 🔴 beamr JIT blocker
with Artemis — keep long workflows off. 🔴 Tom's `~/.aion/config.toml`
`[ops_console] FileSystem` line stays until he installs a binary from
`b20d2da1` or later. Backticked commit messages via `git commit -F -`.
NO-SWEEP untracked: `.aion-accepted-binary-20260731/`,
`.aion-jjuabk-snapshot-20260731/`, `.aion-data-preswap-20260804/`.
⚠️ A store cannot live under `/tmp`. ⚠️ zsh does not word-split unquoted
expansions, and `status` is read-only — name the variable `rc`.

---

# AMENDMENT — tutorial accuracy pass, 2026-08-07 (Tom's direct order)

**His words:** *"Just make sure that the tutorial is accurate. I don't want to
have to tell anybody to change anything from the tutorial."* That overrode my
earlier plan of leaving D1 for him to discover. Full detail:
`docs/design/aion-authoring/TUTORIAL-WALK-CRIB-2026-08-07.md`.

## Method that made the difference

Commands were **extracted from the built HTML and executed verbatim** by
`/bin/bash` — nothing retyped. The previous self-run retyped them, which is
exactly why it declared the deck proven while a paste-breaking defect sat in
it. **A proof that paraphrases its subject is not a proof of the subject.**

## Three defects found and FIXED in the deck

- **D1** shared authoring root with his live server (`~/.aion/authoring`, which
  holds `assistant.awl`, `assistant_spike.awl`, `git_status_sweep.awl`) →
  start command now carries `AION_HOME=~/my-first-flow/aion-home`.
- **D2** 🔴 the availability `-d` payload was wrapped across two lines; a `\`
  continuation ENDS the quoted string, so leading spaces split the argument and
  curl got 42 of the characters — `EOF … column 43`. Valid shell, correct-
  looking slide, `bash -n` clean. Now one unbroken line.
- **D3** config needed hand-editing (`/Users/you/…`). A `~` in `data_dir` is
  taken LITERALLY and creates a directory named `~` — **its own defect, needs a
  row**: silently accepted, `data_root` logged with the tilde intact. Config
  slide is now a `cat > aion.toml <<EOF` block whose `$HOME` the shell expands.
  Same for the run step's placeholder.

## New gate, mutation-proven

`gate_command_payloads()` in `build-tutorial.py` — joins/splits each command as
a shell would and asserts the `-d`/`--data`/`--input` payload parses as JSON.
Not a syntax check; the broken form WAS valid shell. **Mutation-killed with the
literal prior two-line wrap: RC=1 at column 43**, restore byte-identical by
shasum, rebuild green. `config_heredoc()`'s `$HOME` guard killed the same way.
Build prints `ok 2 pasted command payload(s) parse as JSON`; census now reads
`3 excerpts from 4 of 4` (it had begun reporting `aion.toml` as unquoted once
the config moved to a heredoc — a census blind to one of its own doors).

## ⚠️ NOT PROVEN — do not round this up

**That the completed run APPEARS in the console listing.** Console serves 200
and `/awl/documents` answers, but my run-list check failed to parse (wrong
endpoint path guessed) and I tore the world down before re-checking. The claim
is "the console serves", NOT "the run is listed". Confirm by eye during the
walk. The `kill -9` durability step is also still unproven and the deck says so.

## Files touched by this pass (uncommitted, ON TOP of the #7 lane)

- `docs/authoring/build-tutorial.py`
- `docs/authoring/tutorial-examples/aion.toml`
- `docs/authoring/first-workflow.html` (rebuilt)
- `docs/design/aion-authoring/TUTORIAL-WALK-CRIB-2026-08-07.md` (new)

⚠️ These are a SEPARATE concern from the #7 lane. Commit them separately, with
paths named explicitly. Never `git add -A`.

## State at amendment time

`~/my-first-flow` removed; `:8099`/`:50099` verified **free by port** for Tom.
Tom's pids 45425 / 48399 alive, 1d 16h, `:8080` health 200. Review 13 running
in the background on the #7 lane — its verdict is still owed.
