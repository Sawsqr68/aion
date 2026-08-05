# Seat resume — marker AK, 5 August 2026

Supersedes AJ. **Latest letter wins.** AJ described #84 as in flight, unbuilt
and untested; that is no longer true — it is landed. AI's record still holds the
closed #144/#145 detail.

`origin/main` = `HEAD` = `e314caf1`. Tree clean but for the three NO-SWEEP
untracked dirs.

## ✅ #84 CLOSED — `e314caf1`, pushed

**The title was wrong and the framing was a red herring.** "Resolution scans the
raw retained set" suggests filtering to the reachable set is the fix. It is not:
a live run's own version is reachable (`AdmissionReason::LiveWorkflow`) and the
new one is route-active, so both survive any reachability filter and the
ambiguity is untouched. The raw set is in fact the RIGHT input here — unlike
worker admission. What was missing was not a filter but an argument: the RUN.

`DeclaredBodies::body_for(task_queue, action)` had no run parameter, so it could
not answer correctly. A body is a property of the run's own package version, not
of the queue. The consequence was a durability violation: a run started under
version A, still executing, failed **terminally** at its next bodied activity
the moment version B was deployed with an edited command — a version that run
would never execute. Content-hash namespacing exists precisely so versions
coexist.

**The fix.** New `crates/aion-server/src/worker/declared_body_selection.rs`:
`select_declared_body(contracts, action, run_version: Option<&ContentHash>)`.
With the version known, that version's contract is the sole authority — declares
a body, run it; declares none, delegate; never borrow another version's body.
With it unknown (registry handle gone, or registry unreadable — both logged,
never swallowed), the old queue-wide reading stands, refusal and remedy intact,
because without the run's identity choosing IS the guess the refusal prevents.

`declared_body.rs` gains `DispatchingRun<'a>` and
`EngineDeclaredBodies::version_of` (`registry().get(..)` → `loaded_version()`).
The seam is internal to aion-server — nothing outside the crate implements or
calls it (swept).

**Precedent, found by reading rather than assumed:** the ADVISORY class already
resolves by `(workflow_type, loaded_version)` —
`crates/aion/src/runtime/nif_activity_advisory.rs`, whose doc comment gives the
same rationale in its own words: *"a hot redeploy that changes an action's class
never retroactively reclasses an in-flight run's activities."* This fix is that
principle applied to the body.

### What was measured, and two traps avoided

- clippy `-p aion-server --all-targets -D warnings` **EXIT=0** (clean first
  pass — every risk marker AJ listed was unfounded).
- `cargo test -p aion-server` **951 passed / 0 failed / 0 ignored, 46 targets,
  EXIT=0**. Baseline 944 + 7 new, exact. The 46th target is `Doc-tests
  aion_server` with zero tests; 45 code targets, consistent with the baseline.
- Doc example gate **EXIT=0, 43 PASS / 1 UNVERIFIABLE / 2 ILLUSTRATIVE** —
  unchanged from #144.

**🔴 Trap 1 — marker AJ's own mutation check was the wrong mutation.** It said to
make `version_of` return `None` and watch
`an_in_flight_run_keeps_its_own_versions_body_after_a_redeploy` die. That test
calls `select_declared_body` directly and never reaches `version_of`, so the
mutation would have killed nothing and I would have recorded a pass. Written
down because the mistake is generic: **a mutation check only measures what the
test actually executes — verify the mutation is on the test's path before
trusting its verdict.**

**🔴 Trap 2 — the wiring was unproven and the unit tests could not prove it.**
They build the contract's `package_version` and the `run_version` from the same
helper, so they prove the comparison and nothing about whether production hands
over the right identity (an oracle must not read both sides from the caller).
Two things closed it: a new test `the_reader_is_asked_about_the_run_that_is_
dispatching` with a recording double, and reading the production sources to
confirm the identity link — `worker_contracts_for_queue` and
`declares_advisory_action` both read `snapshot.by_version`, keyed
`(workflow_type, ContentHash)`, and `DeployedWorkerContract::package_version`
is that same hash.

**Both mutations then run at my own hand.** Ignoring the run version in
`select_declared_body` kills exactly three tests and leaves both blind-path
controls green. Handing the dispatcher a fabricated identity kills exactly the
wiring test. Restored from file copies, not `git checkout` (which would have
wiped the uncommitted work), and the restoration verified by `diff` plus a grep
for surviving mutation text.

### Docs moved with it

`docs/workers/DECLARED-COMMANDS.md` said a redeploy wedges runs in flight. It
now says the opposite, because the opposite is now true — deploy freely, work in
flight is undisturbed, retiring the old version is housekeeping — and the
refusal is filed under `#### When the disagreement IS refused`, the one case
that still reaches it. `DECLARED-BODY-FINDINGS-2026-08-05.md` finding 2 records
what closing #145 did not close.

## ✅ R2.4 discharged — ruled, then amended into the brief

`BRIEF-139-LEG-A-REWORK.md` **R2 item 4** (cited throughout as R2.4 — the brief
numbers items under `## R2`, there is no literal string "R2.4" in it) ordered
`assistant.awl` and `assistant_spike.awl` to carry a byte-identical
`assistant_provision` body forever, because they share queue `default`.
Waffles ruled it discharged on evidence at 07:40Z, and ruled the FORM too:
**not a quiet edit — an amendment recorded in the brief itself**, naming
`e314caf1`, with the reason the requirement existed, so a reader in a month
sees the rule was real and was retired on evidence rather than evaporating.
No re-ratification; a requirement discharged by its own terms.

Landed as **Amendment 1** at the foot of the brief, plus an inline ⚠️ pointer
on item 4 itself so a reader who stops at the requirement still learns it
moved. It states both permissions separately, because they are different and
only the second matters: *you may stop copying the body*, and **the two
documents are now allowed to carry genuinely different bodies.** It also
states the residual — divergence reintroduces the terminal refusal on the one
path where a run's version cannot be named — and records the state at the time
of amendment (both bodies byte-identical, sha256 `e377d3e8…`), so the starting
position is on the record.

**The rule underneath, worth more than the amendment:** one truth kept in two
places, equal only because a person keeps it equal, has drifted or it will.

## #101 re-measured — one of its claims was false, one is closed, one is real

Taken up because #145 plausibly discharged part of it. Measured rather than
reasoned; the task splits four ways.

1. **"Versions CAN be unloaded"** — correct, and already recorded. Nothing to
   build.
2. **"The real gap is ergonomics: nothing tells an operator the verb exists"**
   — **FALSE, and it was false when written (2026-07-31).**
   `docs/GETTING-STARTED.md` has carried a whole section — *"## 8. Versions,
   routing, rollback"*, with `aion versions` / `route` / `unload` — since
   **2026-06-12** (`359c95631`), six weeks earlier and before the #80 incident
   the task cites. `git log -S "aion unload"` on that file returns exactly one
   commit, so the string was added once and never removed: continuously
   present. `docs/operations/operations.md` and `operations/API.md` carry it
   too.
3. **The narrower true claim — the failure did not point at the remedy** —
   was real, and is **closed by #145** (`fc1c10cc`): the ambiguity refusal now
   prints runnable `aion unload <type> <hash>` commands with full hashes.
   That is what the #80 operator actually lacked; not a doc, a signpost at the
   moment of failure.
4. **The residual capability gap — real, verified today.** A type's
   route-active version cannot be unloaded and **there is no route-retire
   path**: `grep` for `retire_route` / `remove_route` / `fn undeploy` across
   `crates/` returns nothing relevant (the `retire` hits are timers, deadlines
   and process exits). So version accumulation is bounded by what the operator
   unloads; **type accumulation is not bounded at all.**

**Item 4 is the only live one, and it is a decision, not a fix.** Decision
record #62 D2 ruled manual unload with engine-enforced safety, the embedding
platform owning *when*, and deliberately no auto-GC inside aion. Whether a
type should be retirable at all is a new question against that ruling, not an
oversight to patch — **do not build a route-retire path without Waffles.** It
also feeds #105 (Manifold condition 2), whose evidence #101 says it is; the
correction to item 2 should travel with it.

**Method note:** the claim that failed was the one stated most confidently and
never measured. Dating a doc line is two commands (`git log -S` plus a read),
and it overturned a six-week-old belief that had already been cited as
evidence to Waffles.

## Board

- ✅ #84 (`e314caf1`), ✅ #144 (`483c933a` + `4bed6a81`), ✅ #145 (`fc1c10cc`).
- Held on Tom: the #139 live cut, #142, the 0.11.1 decision. Queued: #141.
- Nothing else on the board is startable without Tom or Waffles.

## Unchanged standing constraints

🔴 Worker rebuilds frozen until the #139 cut — the console is not a worker, and
restarting an already-built worker is not a rebuild. **Editing aion-server
source is not a worker rebuild, but do not restart Tom's live processes.**
🔴 Tom's `~/.aion/config.toml` `[ops_console] FileSystem` line must stay until he
installs a binary built from `b20d2da1` or later. 🔴 beamr JIT blocker with
Artemis. ⛔ Never `git add -A` here — name paths. ⛔ Never bare `cargo clean`.
Backticked commit messages via `git commit -F -`. Do not edit `CLAUDE.md`.

Live pids, untouched all session: server **45425**, worker **48399**, both from
the repo root. Server log: scratchpad `server.log`.
