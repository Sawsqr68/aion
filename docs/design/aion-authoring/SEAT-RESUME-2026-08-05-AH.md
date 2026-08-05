# Seat resume — marker AH, 5 August 2026

Supersedes AG. AG's shipped record still stands; the defect AG opened is
CLOSED here, and its diagnosis was wrong.

## ✅ CLOSED — the "worker eligibility flaps" defect was a REPORTING defect

Marker AG recorded: *"the withdrawal is firing on ping count, not elapsed
silence"* and told Tom the agent might not answer. **Measured before touching
anything, and the conclusion was wrong in the way that mattered.**

### The measurement

- `lsof -p 45425` — the running server holds the scratchpad `server.log` open
  on fd 1 AND fd 2. The log is live, so silence in it is a fact about the
  server, not about my capture.
- Worker registered 04:32:36.689Z. At 04:38:43Z that is 367 s. Cadence is
  7500 ms (`liminal worker liveness probe started` says so), so ~49 probe
  rounds had run.
- `apply_answer` logs a WARN on EVERY failed ping, EVERY round — two distinct
  loud lines (`THE SERVER COULD NOT ASK` / `did not answer`). **Zero appeared.**

The failure path is loud by construction, so 49 rounds of silence is POSITIVE
evidence that every ping was answered — not absence of evidence. The worker had
been dispatch-eligible since ~04:32:46Z, ten seconds after it registered.

**There was no dispatch defect. The caveat I gave Tom was unnecessary.**

### What was actually wrong

`register_connection` opens an UNSERVED probation, so the first probe round
after ANY registration finds the probation unserved and announces a transition.
The line it announced was false:

- *"the server has not been able to reach its dispatch path within the window"*
  — at that instant one answer was already banked. The server HAD reached it.
- *"becomes eligible again as soon as one ping is answered"* —
  `DISPATCH_PROBATION_PINGS` is 2 consecutive, and one was already in hand.
- The restoration 7.5 s later was **silent**. Withdrawal announced, recovery not.

So every healthy worker start printed an alarm and no all-clear. An alarm that
fires on every ordinary connect carries no information, and a resolution nobody
announces cannot cancel it. It fooled me, reading the code with the history.

### The fix — `2467e6d4`, pushed

`crates/aion-server/src/worker/{heartbeat,liminal_liveness,registry}.rs`.

- `Reachability` gains `ever_proved`, set where a probation completes, cleared
  ONLY by `register_connection` (a redial is a new measurement).
- `unreachable_workers` now returns `ExcludedWorker { worker_id, exclusion }` —
  the reason travels WITH the membership, so the set that gates dispatch and the
  reason the operator is told cannot describe different states.
- Three announcements where there was one: `OpeningProbation` (INFO, says it is
  the ordinary cost of connecting), `ReachabilityLost` (WARN, says eligibility
  was actually HELD), `EligibilityRestored` (INFO, new — the all-clear).
- `transition()` is pure and total; the test enumerates **all four inputs**.
  The old inlined decision could only ever emit one of the three lines, which is
  why the missing two were invisible.
- `ConnectedWorkerRegistry::dispatch_ineligible()` added — a recovery is
  precisely the worker the NEW verdict does not mention, so per-worker queries
  cannot find one. If it cannot be read the announcements are SKIPPED, never
  guessed; the verdict itself still publishes.

**Control run at my hand:** collapsing `exclusion()` to always report
`ReachabilityLost` fails exactly 3 tests (750 passed / 3 failed) and no others.
Restored: 753 / 0. `clippy -D warnings` clean — 3 findings fixed in the CODE
(match arms merged, `const {}` assert, a 4-fold repetition extracted), no
`#[allow]`.

## ✅ The embed is committed — `b20d2da1`, pushed

`cargo xtask build-ops-console`. **Verified against the artifact:** the embedded
JS carries `return{message:e,end:!1}` and `return{message:"",end:!0}`.

A clean clone now builds a console that can drive the assistant. That retires
the `[ops_console] source = { FileSystem = … }` override — **but only for a
binary built from `b20d2da1` or later.**

## 🔴 THE GAP TOM'S ASK EXPOSED — crates.io still ships the broken console

`README.md` says `cargo install aion-cli --locked`. crates.io max version is
**0.11.0** (verified via the registry API, versions list `['0.11.0','0.10.0',…]`).
0.11.0 predates the assistant fix (`9b30ca5e`, today). So **anyone following the
published instructions today installs a console whose chat cannot work.**

The instructions are not missing. The ARTIFACT they point at is stale.

**This is Tom's call: cut 0.11.1, or leave it.** Raised with him 2026-08-05.
Nothing published without his word.

## 🔴 TOM'S MACHINE — do not remove the config line yet

`~/.aion/config.toml` carries the `[ops_console] FileSystem` override pointing
at `apps/aion-ops-console/dist`. His server is the **installed 0.11.0**, whose
embedded bundle is still the broken one. Removing that line before he installs a
newer `aion` puts the broken console back. Backup: scratchpad `config.toml.bak`.

Live pids: server **45425**, worker **48399**, both from repo root.

## ✅ CLOSED after the marker was written — `d8e6d568`, pushed

`AWL-REFERENCE.md:92` is no longer UNVERIFIABLE. The recorded reason ("the
repository does not contain the schema") described a missing capability in the
GATE, not a defect in the documentation — the gate had no way to put a file
where an import looks. `Spec.fixtures` now writes files beside the composed
document (where AWL resolves imports from, NOT the process cwd), and
`describe()` names each supplied fixture in the row so the PASS cannot be
misread as "copy this and it runs" when the reader brings their own schema.

Reaching the checker exposed a latent scaffold defect: the default tail routed
`done(summary, message)` while `GATE_TYPES` declares `GateResult { summary }`
only, so ANY block using the pure default scaffold would have failed on the
scaffold rather than its own content. This block was simply the first to use it
unmodified. Corrected.

**Measured: 41 PASS / 1 UNVERIFIABLE / 2 ILLUSTRATIVE / 0 FAIL, exit 0**
(was 40 / 2 / 2 / 1). Every PASS still survives the corruption control. The one
remaining UNVERIFIABLE is `AWL-REFERENCE.md:85` — two expression forms shown
out of statement position; checking them needs a binding that would change the
fragment. That reason is about the block, so it stands.

## ✅ The battery re-earned on the FINAL tree

`battery-re-runs-on-new-tree` applied honestly, and it caught me mid-sentence.
I was about to record "docs-only since the battery" — **that was false.**
`b20d2da1` rewrote bytes under `crates/aion-server/ops-console-embed/`, and
`rust_embed` makes those a COMPILE INPUT of the crate. A green earned before
them is green on a different tree.

The two verdicts have different inputs, so they were settled differently:

- **clippy** reads Rust source and manifests. `git diff 2467e6d4..HEAD -- '*.rs'`
  and the same for `Cargo.toml`/`Cargo.lock` are both **EMPTY** — byte-identical,
  so the clean verdict carries and re-running it would prove nothing new.
- **the compiler and the tests** read the assets too, so that verdict was
  re-earned at HEAD: `cargo test -p aion-server`, **EXIT=0**, 45 verdict lines
  counted (not prose) totalling **937 passed / 0 failed / 0 ignored**.
  Log: scratchpad `aion-server-head.log`.

The rule this makes concrete: *"nothing but docs changed"* is a claim about the
BUILD GRAPH, not about file extensions. An asset directory inside a crate is
source as far as the compiler is concerned.

## Still owed
- 🔴 #142 — merge `fix/worker-shell-dial-and-audibility` when the #139 cut
  clears, then run the operator's real command.
- #141 — wire-with-oracle sweep. *The oracle must not read both sides from the
  caller.*
- Docs site — silence-default: nothing gets built.

## Standing constraints (unchanged)

🔴 Worker rebuilds frozen until the #139 cut — the console is not a worker.
🔴 beamr JIT blocker with Artemis. ⛔ Never `git add -A` here (the three
`.aion-*` untracked snapshot dirs must never be swept). ⛔ Never bare
`cargo clean`. Backticked commit messages via `git commit -F -`. Do not edit
`CLAUDE.md`. Tom DM: `dm:c9255b2a-5731-4d17-8124-e3bfa2224186`.

## Commits today, all pushed

`adecb099` · `de4cc3ca` · `309b7f18` · `9b30ca5e` · `947aae32` ·
`2467e6d4` (eligibility reporting) · `b20d2da1` (embed).
`git status` clean but for the three NO-SWEEP dirs.
