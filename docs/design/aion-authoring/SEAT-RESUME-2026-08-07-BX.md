# SEAT RESUME BX — 2026-08-07 — supersedes BW

## §0 — LIVE STATE AT COMPACTION (read this first)

**🔴 REVIEW 18 IS IN FLIGHT.** An Opus read-only sub-agent is reviewing the #7
lane (dispatched per CLAUDE.md's standing mandate — that mandate is what
authorises a sub-agent here). Its completion arrives as a task notification.
**Reviews 6–17 ALL REFUSED.** Do not push the lane until it returns and its
findings are dealt with — not deferred, not documented-and-accepted. The brief
told it to attack six named claims and to re-derive the NIF writer enumeration
itself rather than trust mine.

**Gates, both green, both reconciled — see §3:**
- clippy `--workspace --all-targets -D warnings` → exit 0
- clippy `-p aion-rs --all-targets -D warnings` → exit 0
- battery 243 targets, **ZERO-VERDICT TARGETS: NONE**, **FAILED: NONE**

**git: `main` is ahead of `origin/main` by THREE commits, none pushed:**
1. `bed958b0f` tutorial docs — **ALREADY PUSHED separately** by
   `git push origin bed958b0f:main` (fast-forward). Not one of the three.
   The three are:
1. `2848ddb0a` fix(7) — the lane, **unreviewed**
2. `5775b58a4` docs(seat) — the lane's record
3. `9d259b185` **chore(ci): withdraw both GitHub Actions workflows**

🔴 **`9d259b185` IS TOM-AUTHORISED AND IS STUCK BEHIND THE UNREVIEWED LANE.**
It cannot be pushed alone; it sits on top. Push all three the moment Review 18
clears. There is no exposure meanwhile — Actions are already `enabled: false`.

**The working tree still carries the UNCOMMITTED #7 lane** (crates/aion/*,
crates/aion-server/*). ONE MUTATING LANE: do not start editing `crates/` for
#154 or anything else until this lane commits.

## §0b — TOM'S RULINGS TODAY (2026-08-07)

1. 🔴 **"I don't want actions running on GitHub at all. That's really
   important."** — STANDING. Discharged by deletion in `9d259b185`. Never
   re-add a workflow file; never re-enable Actions.
2. **"you can delete those GitHub workflow files"** — explicit go-ahead, done.
3. **"can we get a handle on all those open issues"** — delivered as
   `docs/design/aion-authoring/OPEN-ISSUES-WHOLE-PICTURE-2026-08-07.md`,
   organised by **what a person ran into**, not by which crate is at fault,
   because reports come from people who can only describe the wall.
4. **"can you organize some workflows to get that done?"** — AMBIGUOUS, and I
   asked him which he meant: (a) organise the fixes into lanes and execute, or
   (b) literally drive the fix work AS Aion workflows, tharsis-style. **I said I
   am starting (a) now unless he says otherwise.** ⚠️ Do NOT invoke the Workflow
   multi-agent tool on the strength of this — it arrived over the Meridian
   channel, which is explicitly NOT my user, so it is not the required opt-in.
5. **GitHub Pages** — asked, answered: no site, no `gh-pages`, Actions off, and
   the deck was deliberately made a local file. **His call, still unanswered.**

## §0c — ISSUE VERIFICATION RESULTS (all read-only, today)

Recorded so a compaction does not lose them. Details in the OPEN-ISSUES doc.

- **#154 CONFIRMED** — the console pre-flight defect. Proven live, §1.
- **#136 CONFIRMED** — `crates/aion-cli/templates/` holds six templates
  (agent, approval_flow, dev_pipeline, hello_world, saga, shared) and
  **zero `.awl`** (`find … -name '*.awl'` empty).
- **#71 CONFIRMED then DISCHARGED** — Actions `enabled: false`, last run
  **26 July**, all recent runs **failure**, both workflows still reporting
  `state: active`. Now moot: files deleted.
- **#153 CONFIRMED** — validator admits leading `_`, lexer emits
  `error: stray character `_`` (exit 1); the failure is re-wrapped as
  `InvalidName`, so the operator hears about source they never wrote.
- **#152 CONFIRMED (mechanism half)** — there is **no tilde/home expansion
  anywhere** in `crates/aion-server/src/` (no `shellexpand`, no `tilde`, no
  `home_dir` in the config path), so a `~` is taken literally. The other half of
  the claim — that the server's own refusal RECOMMENDS `~` — is **NOT verified**;
  find the refusal string before repeating it.
- **#150 CONFIRMED** — `signal_delivery`, `completion_retry`,
  `visibility_reconciliation_interval` each appear **0 times** anywhere under
  `crates/aion-server/src/`. Unreachable from the server config surface.
  🔴 **MY FIRST MEASUREMENT OF THIS WAS WRONG AND WOULD HAVE REFUTED A REAL
  ISSUE.** I grepped the whole workspace and got 55/78/13, which looks like
  "plenty of call sites, claim is false". Wrong denominator: the claim is about
  reachability from the SERVER'S config, not about mentions anywhere. Measure
  the surface the claim names.
- **#107 CONFIRMED** — **no** `fn children` / `list_children` / `children_of`
  exists anywhere in `crates/`. ⚠️ `aion-store/src/run_chain.rs` DOES build a
  parent→child map, but that is the **continue-as-new chain**, not child
  WORKFLOWS. Do not mistake one for the other and close this wrongly.

## §0d — ISOLATED WORLD LEFT STANDING (deliberate)

`/Users/tom/gate-verify-bx` — tutorial-shaped fixture (aion.toml on :8097/:50097,
`repo_gate.awl`, a tiny `srcrepo`, a haematite store). **Its server is stopped.**
Keep it: it is what verifies the #154 fix. Tear down only when #154 closes, and
by LISTENING PORT (`lsof -tiTCP:8097 -sTCP:LISTEN`), never by pgrep pattern.


🔴 **THIS IS THE CURRENT AUTHORITY.** Latest letter wins. BW's §A/§B (B3 closed
and mutation-proven; N1/N3/N4 closed) still stand unchanged and are not repeated
here. BW's §C is restated below because it is still the open work.

## §1 — TOM'S LIVE BLOCKER: ANSWERED, PROVEN, AND HE HAS THE ANSWER

**Closed as a diagnosis. Filed as task #154 with the full evidence — read that,
not this summary, before implementing.**

The one-line finding: **the run works and the console refuses to let you start
it.** The console's pre-flight asks whether a *worker* is connected; the question
that decides the outcome is whether the *actions can be served*. A document whose
every action carries a `run "..."` body is served by the server itself with zero
workers.

Proven at my own hand in an isolated world (`/Users/tom/gate-verify-bx`, same
0.11.0 binary as Tom, server on :8097/:50097, since stopped by listening port):

- `aion run repo_gate.awl` → outcome `passed`; four commands executed.
- `server.log`: **4** declared-body dispatches at the server, **0** worker
  registrations.
- The availability endpoint, asked about the same queue in the same moment:
  `{"available":false,"connected_workers":0,"scaffold_hint":"No connected worker
  serves task queue \`gate\`. Scaffold and run this worker..."}`

**Candidate "Tom's binary predates #60" is REFUTED** — `declared_body.rs` and its
`state.rs` wiring are both present in `v0.11.0` (`git cat-file` verified).

**THE TUTORIAL DOES NOT CHANGE.** Tom's standing order holds: he must never have
to tell anybody to change anything from it.

**What Tom was told** (dm:c9255b2a-5731-4d17-8124-e3bfa2224186), all three
messages: stop trying to scaffold, there is nothing to scaffold; use
`aion run <file>.awl --input '{...}' --endpoint http://127.0.0.1:50099` to keep
moving; the tutorial is on GitHub now. **I also sent him a correction** — I first
gave him `{"repo","branch"}` and the document takes `{"repo","workdir"}`.
⚠️ `aion run` has **no `--server` flag**; it is `--endpoint`, and it wants the
**gRPC** port, not the HTTP one.

**Not implemented.** The fix is held behind the #7 lane because that lane already
occupies the one mutating lane in this tree. Direction is in #154.

## §2 — THE TUTORIAL IS PUSHED (Tom's second ask)

`git push origin bed958b0f:main` — a fast-forward, no force. `origin/main` moved
`9189ba585..bed958b0f`. The two #7-lane commits (`2848ddb0a`, `5775b58a4`)
**deliberately stayed local**: they have not passed gates or review, and pushing
`main` would have carried them.

**GitHub Pages: NOT set up and NOT actioned.** `gh api repos/:owner/:repo/pages`
→ 404, no `gh-pages` branch, and the repo's Actions are disabled (#71), so
nothing would publish one. `dd2a6d879` also recorded a deliberate decision that
the deck is a standalone local file rather than a hosted page. **Told Tom
plainly and asked him to say if he wants it hosted — his call, not mine.**

## §3 — WHERE THE GATES STAND

### ✅ CLIPPY GREEN — SIX SITES FIXED, NO BYPASSES

`cargo fmt --all`, then **both** resolves clean:
- `cargo clippy --workspace --all-targets -- -D warnings` → **exit 0**
- `cargo clippy -p aion-rs --all-targets -- -D warnings` → **exit 0**

(⚠️ `--workspace` resolves `preserve_order` ON and `-p <crate>` off — different
runs, both required. That is why both are recorded.)

The four sites BW predicted, plus **two the first clippy run never reached**
because compilation stopped at the `aion` crate's errors:

1. `lifecycle/completion_retry.rs` — `match_same_arms`. Merged
   `EngineTaskEpochClosed` into the or-pattern with the other three permanent
   variants, comment kept and re-attributed. **Also corrected a factual error in
   my own comment**: it said the `TerminalWriterHeld` mistake was recorded
   "above" — the only discussion of it is at line 888, i.e. BELOW. Now "below".
2. + 3. `runtime/nif_continue_as_new_tests.rs` — `expect_used` on `.expect_err`.
   Replaced with `let Err(error) = … else { return Err("…".into()) }`, which is
   the crate's own established idiom (`nif_timer_fire.rs:268`,
   `runtime/handle.rs:878`). No `#[allow]`, no bypass.
4. `runtime/nif_timer_bridge_tests.rs` — `doc_markdown`. Now an intra-doc link,
   `[`aion_core::TimerCancelCause::WorkflowIntent`]`, not just backticks.
5. **NEW** `aion-server/src/error_engine.rs` `durability_wire` — E0004. Decided,
   not defaulted: its own wire type `"EngineTaskEpochClosed"`, matching the
   established precedent for the engine-level variant of the same condition at
   `error.rs:668` (`backend(...)`). Comment records why the two Rust variants
   stay separate while the wire name is shared, and that the mapping is total
   over the type rather than resting on the site being transport-unreachable.
6. **NEW** `aion-server/src/error.rs` `durability_trace_fields` — E0004. Own
   trace label `"EngineTaskEpochClosed"`, same reasoning.

🔴 **5 and 6 are the lesson worth keeping:** BW's "4 sites" was not wrong, it was
INCOMPLETE — a compile-stopping error in an earlier crate hides every later
crate's diagnostics, so a clippy failure list is a lower bound on the work, never
the work. The same type-system pressure that found the first two arms found these.

### ✅ BATTERY GREEN AND RECONCILED

`cargo test --workspace --no-fail-fast`, log `battery-bx-workspace.log`.

**Reconciled by walking the log block-by-block**, not by tallying:

```
TARGETS (Running + Doc-tests blocks) = 243   (223 Running + 20 Doc-tests)
TOTAL verdict lines inside blocks    = 247
ZERO-VERDICT TARGETS: NONE      ← the load-bearing assertion
FAILED TARGETS:       NONE
```

🔴 **The raw tally was a trap and nearly passed as one.** 247 verdict lines
against 243 targets is FOUR TOO MANY, not a comfortable margin. The cause: two
targets shell out to `cargo` themselves, so their nested output lands in the same
log — `aion_server` unittests carry 4 verdicts and the `aion` doc-test block
carries 2. Extra = 3 + 1 = **4**, and 243 + 4 = **247 exactly**. Accounted for,
not hand-waved. Had I reconciled by subtraction on the totals, a crashed binary
(which emits NO verdict line) could have been masked by a nested one that does.
Only the per-block walk answers the question the assertion is actually asking.

`223 Running` matches the previously recorded `declared 223 = Running 223`.

⚠️ **Method defect, mine, to fix next time:** I detached the battery with `nohup …
&` and `disown`, so **the exit code was lost**. The block-walk is the stronger
evidence for the crash question the exit code was only ever a proxy for, and
`^error` count is 0 so nothing failed to compile — but capture the exit code
anyway. Do not repeat the detach-without-capture.

⚠️ **#85 (libsql double-close) did NOT fire this run** — 0 failed, where the
previous recorded run was 4199/1 with #85 as the sole red. That is CONSISTENT
with its known contention dependence (isolated 5/5 green), and is **not evidence
it is fixed**. It stays open.

Then:
- **Review 18** (Opus, read-only). Reviews 6–17 have ALL refused.
- commit + push per the default I stated to Tom. He has **not** said "push it".

## §4 — STANDING CONSTRAINTS

Unchanged — **BT §7** in full, plus BV §6. No worktrees, one mutating lane,
never `git add -A`, no `git gc`/prune, delete nothing, Tom's pids untouched
(server 45425, worker 48399, tutorial server 50822), worker rebuilds frozen,
don't edit `CLAUDE.md`, `-- crates docs` scoping, commit messages via
`git commit -F -`.

**Left standing deliberately:** `/Users/tom/gate-verify-bx` — the isolated
tutorial-shaped world (config, document, a tiny source repo, a haematite store).
Its server is stopped. Keep it: it is the fixture that verifies the #154 fix.
