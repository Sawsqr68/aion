# Seat resume — marker AP, 6 August 2026

Supersedes AO. **Latest letter wins.** AO holds Tom's tutorial order verbatim,
the console bundle-proof method, and issue #10's measured breakdown; that
record stands and is still worth reading.

`origin/main` = `HEAD` = `a32a28846`. Tree clean but for the three NO-SWEEP
untracked dirs. **Tom's pids 45425 (server) and 48399 (worker) alive and
untouched throughout — 19h32m uptime, `/health/live` 200 at teardown.**

## 🔴 #146 — TOM'S TUTORIAL IS BUILT AND SELF-RUN. IT IS READY TO WALK.

His order was: build it, **prove it myself end to end from scratch**, then walk
it with him live and fix what breaks. The self-run is done.

### What was proven, in an isolated world

`AION_HOME` of its own, ports `:8099`/`:50099`, absolute `store.data_dir`,
torn down completely at the end. **No worker was rebuilt** — the shell worker
is the shipped `aion 0.11.0` binary, so the estate-wide rebuild freeze was
never touched.

The whole arc ran: config → `aion server` → document created through the
console's own `POST /awl/documents` → real source saved via `PUT` → `aion awl
check` → `aion deploy` → `aion worker shell` → `aion run`. It returned

```
"found": "aion-data\naion.toml\nfirst_flow.awl\nprobe_scalar.awl\nworker.toml"
```

— a real `ls` on the machine, back through a durable run, `Completed` in
0.63 s, listed by the console's own endpoints. That output is the deck's result
slide.

### The deliverable

- **`docs/authoring/first-workflow.html`** — 23 slides, ~29 KB. Open it in a
  browser. Same shell as the AWL deck (the CSS is not duplicated; the builder
  substitutes the `<title>` and asserts the string it replaces still exists).
- **`docs/authoring/build-tutorial.py`** + **`tutorial-examples/`** — four
  example files, all of them the artifacts that actually ran.

**Three gates, each watched refusing on its own path:**
1. every `.awl` must pass `aion awl check` — mutated a binding, refused;
2. the manifest must wire its queue's **full** declared action set — added an
   unwired action, refused with the exact message;
3. every `aion` verb the deck teaches must resolve — this one first refused
   for the **wrong reason** (a copy in `/tmp` resolved its examples relative to
   `/tmp` and never reached the verb gate), re-run on the real path, refused
   correctly. Worth remembering: the mutation has to sit on the path.

### 🔴 The find — a real defect, out of the self-run

**`SHARED-QUEUE-SILENT-REFUSAL-2026-08-06.md`, commit `642f30579`.**

A worker is admitted only if it advertises the **full** action set every
deployed document declares for its queue. So a second document reusing a
`worker` block with a different action makes that queue unservable — and it
fails **silently on both sides**: worker process alive, 0 bytes logged, server
log 0 ERROR / 0 WARN. The only signal is
`POST /awl/workers/availability` → `connected_workers: 0`, whose hint tells you
to run the worker you are already running.

Measured with a control and a reversal — same worker, same files, serving
before / refused after / serving again once one document carried both actions.
**A live worker is not evicted**, so it breaks at the *next restart*, detached
from the deploy that caused it. That is what makes it so confusing.

Same shape as #78 and #80, both recorded closed. **Mechanism unread and
regression-status undated** — both stated as open in the doc. Do not claim it
is a regression without checking.

### Still open on the tutorial

- The **kill -9 durability step is NOT part of the proven path.** The deck says
  so explicitly and offers it as "try it yourself next". I could not stage it
  cheaply — the shared-queue defect ate that attempt. If we want it proven,
  that is a separate self-run.
- Two console gaps from AO remain unfixed (where the file lands; the name
  rule). **The deck answers both in prose** — `$AION_HOME/authoring`, printed
  at startup as `authoring_root`, and the `^[a-z_][a-z0-9_]*$` rule stated up
  front. ⛔ Still do not rebuild the console bundle without talking to Tom.
- My one unanswered question to him: is the deck for him showing someone over
  his shoulder, or someone alone with nobody to ask? I built for the first.

## 🔴 ISSUES — ALL FIVE TRIAGED

**`ISSUE-TRIAGE-2026-08-06.md`, commit `a32a28846`.** Two Opus subagents,
read-only, every citation re-read, every claim date-checked with `git log -S`
plus a sanity count proving the filter was live.

| Issue | Verdict |
|---|---|
| #10 | PARTLY STALE — claim 1 true, claim 2 half-stale, claim 3 a real trap |
| #9 | TRUE, NOT STALE — **an external duplicate of our own reverted #90** |
| #7 | **TRUE, NOT STALE, undefended — the strongest of the five** |
| #6 | PARTLY STALE — head-of-line fixed 2026-07-30 (unpinned); `max_concurrency` ignored on the liminal path is the live half |
| #5 | TRUE, NOT STALE — **and the spec overclaimed** |
| #1 | TRUE, NOT STALE — the line has never been touched |

**#7 is the one to build.** Between #7 and #1 there are five husk tests that
cannot fail if the defect is present, and two pins that encode the defective
behaviour as the contract. #7's four tests all take `InMemoryStore`, whose
`read_history` cannot fail transiently, so the `?` is never taken; the
swallowing log site at `start.rs:574` is executed by no test in the crate. The
fault-injection harness already exists one directory away
(`nif_child_watch.rs:1142`).

**Fixed in passing:** `AWL-2-SPEC.md` claimed sequential fold was expressible
as `fork … sequential` or `loop`. It is not — `fork` has no accumulator, `loop`
has no collection binding, and `Expr::Index` stores a literal `u64` whose own
doc comment reads *"Computed indices are unwritable"*. I verified both at my
own hand rather than on the subagent's report. That row would have had #5
closed as already-supported.

**Nothing has been replied to on GitHub.** Every verdict is ours, not theirs.

## Standing constraints (unchanged, all still binding)

🔴 Worker rebuilds frozen estate-wide until the #139 cut — **not touched; the
tutorial uses the shipped binary's built-in shell worker.**
🔴 Tom's `~/.aion/config.toml` `[ops_console] source = { FileSystem = … }` line
must stay until he installs a binary built from `b20d2da1` or later.
🔴 beamr JIT blocker with Artemis — keep long workflows off until fixed beamr.
⛔ Never `git add -A` here — name paths. ⛔ Never bare `cargo clean`.
⛔ NO WORKTREES in aion; one mutating lane. ⛔ Never publish artifacts for Tom —
deliverables are local files he opens. Backticked commit messages via
`git commit -F -`. Do not edit `CLAUDE.md`. Never mention prospekt.
NO-SWEEP untracked: `.aion-accepted-binary-20260731/`,
`.aion-jjuabk-snapshot-20260731/`, `.aion-data-preswap-20260804/`.
⚠️ A store cannot live under `/tmp` — the path-safety gate refuses a
world-writable ancestor. The scratchpad cannot host an Aion world.

## Board

- ✅ Closed at this seat: #84, #101, #102, #103. R2.4 discharged.
- ✅ **#146 self-run DONE** — deck built and gated, defect found and committed.
  Remaining: walk it with Tom.
- 🔬 #107 measured, NOT built — with Tom via Waffles. Confirmed live again in
  this run (`"parent": null` on every listing row).
- 🔬 #141 both measurable legs done; oracle unbuilt, queued behind the #139 cut.
- 🆕 **Shared-queue silent refusal** — new, committed, mechanism unread.
- 🆕 **#7 (GitHub)** — the build item the triage surfaced.
- Held on Tom: #139 live cut, #142, the 0.11.1 decision.

## Method memories current at this marker

`a-stated-reason-is-evidence-both-ways` (earned twice today — it convicted five
husks and exonerated the deliberate serial-executor decision in #6) ·
`mutation-must-sit-on-the-tests-path` (earned: my verb gate first refused for
the wrong reason) · `a-verdict-invariant-to-the-error-cannot-see-it` ·
`a-census-is-decisive-only-over-what-is-not-exported` (`Replay::now()` has no
caller *in this repository*, and `Replay` is exported) ·
`withdrawal-proven-against-the-artifact` · `confidence-is-what-stops-you-measuring`.
