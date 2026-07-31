# 📍 SEAT RESUME — 2026-08-01 (C)

**Supersedes `SEAT-RESUME-2026-08-01-B.md`. Read this first.**

I am **Vesper Lynd**, session `5849e0d8-4802-4869-8e0f-9f7fd187e198`. Sign
`— Vesper`. **Fable is the MODEL I run on, not my name** — I signed two Meridian
messages with the model tier after a compaction and Athena caught it. The thing a
process runs ON is not the thing it IS.

## State

```
origin/main  0e297071   (#86 closed and pushed, 3 commits)
local  HEAD  8c758d60   (#125, COMMITTED NOT PUSHED)
```

**A cold workspace battery is running detached**, launched 21:47Z:
```
pid 27581   sh -c 'cargo test --workspace --no-fail-fast; echo WORKSPACE_EXIT=$?'
            PPID 1, cwd stack/aion, started 21:46:59Z, cargo child 27583
            (I first reported 10852 — that was the CARGO CHILD my grep matched,
             not the wrapper. Athena verified the wrapper. Reap the wrapper.)
⚠️ SIGNATURE COLLISION: Waffles runs the IDENTICAL command+toolchain in
   apps/manifold/.worktrees/tree-registry. ONLY cwd separates it from mine.
log         <session scratchpad>/ws4.log            free at launch: 77 GiB
```
🔴 **COLD** — `target/debug` was reclaimed at 21:44Z, so this rebuilds
everything. Expect it long. Athena Zooper Dooper (`dm:45cf420e-621d-467b-b96c-5bffb62c04f3`)
holds a standing reap authorisation and snapshots the log; my scratchpad dies
with my session, hers does not.

### 🔴 ws4 PROGRESS AT COMPACTION (21:54Z, ~7 min in)

```
76+ verdicts, 0 non-clean, 0 FAILED, 0 compile errors
COSTLY_PROOF_SKIPPED announcements: 5   <- #86 machinery intact through #125
ws4.log:1664  input_skeleton_round_trips_… ... ok   <- THE #125 TRIGGER, GREEN
free 54 and FLAT — the cold compile phase is OVER, debug 22.8 GiB
```
Athena no longer expects a Tom escalation on this run (trigger 35 is +19).

**⚠️ ONE UNCOMMITTED EDIT:** `GLEAM-FAILURE-HONESTY-125.md` was edited AFTER
commit `8c758d60` to record the isolation confound below. `git add` + amend or a
follow-up commit before pushing.

### ★ The #125 trigger is established FOUR ways, and the fourth is not redundant

```
run 2  line 1157  ok        complete run, WORKSPACE_EXIT=0
run 3  line 1157  FAILED    error: HTTP error, hex.pm
       single-test re-run:  ok, bracketed by 4x HTTP 200 on the failing URL
       independent probes:  200 x5 across 21:20-21:23Z (Athena)
run 4  line 1664  ok        COLD full-workspace, no special-casing
```

🔴 **AN ISOLATED RE-RUN CAN PASS *BECAUSE* IT IS ISOLATED.** My probe-run-probe
re-ran the test ALONE — warm cache, no contention. That pass is consistent with
"the network recovered" AND with "this only fails under load or from cold", and
I had chosen between them by assumption. Only the cold full-workspace green rules
it out. ⇒ **When you re-run a failure in a NARROWER context than it failed in, a
pass has two explanations. The confirming run must be at least as wide as the
failing one.** (Athena.)

### To finish #125

1. `grep 'WORKSPACE_EXIT=' ws4.log`. **No marker = TRUNCATED**, re-run rather
   than salvage.
2. 🔴 **A TAIL IS A WINDOW, NOT A SCAN.** Colour comes from a whole-file scan:
   ```
   grep -n '^test result:' LOG | grep -v 'test result: ok\.'
   grep -n '^test .* FAILED$' LOG
   ```
3. Coverage control (the tally alone is not enough):
   ```
   awk '/^[[:space:]]+Running |^[[:space:]]+Doc-tests / { if (s) u++; s=1; t++ }
        /^test result:/ { s=0 }
        END { if (s) u++; print "targets:", t+0, "UNPAIRED:", u+0 }' ws4.log
   ```
4. Watch specifically: `input_skeleton_round_trips_…` should be GREEN (it was
   the #125 trigger and hex.pm was up at 21:23), and the **aion-rs** suite —
   `engine/mod.rs` is my change, so a surprise there is mine, not the network.
5. Push `8c758d60`, mark #125 completed.

## ✅ Closed today

- **#76** (`f7af328b`) — a transitive `serde_json/preserve_order` rewrote emitted
  bytes with nothing detecting it. With the ordering defence DELETED the
  per-crate run was 0 failed / exit 0 and the `preserve_order` run was 11 failed.
- **#86** (`11e541ae` + `0e297071`) — five `#[ignore]`s removed. **`IGNORED: 0`,
  measured** (was 5), 240 targets / 0 UNPAIRED. See `COSTLY-PROOF-GATES-86.md`.
- **#125** (`8c758d60`, unpushed) — see `GLEAM-FAILURE-HONESTY-125.md`.

## 🔬 Method laws earned today

Full catalogue in memory: `assert-the-invariant-not-the-enumeration.md` (21
rules). Minted or sharpened today, beyond what is already there:

**🔴 A BASELINE IS ONLY A CONTROL IF IT IS INDEPENDENT OF WHAT IT MEASURES.**
My #86 gate derived its expectation from the call sites it audited: `4 vs 4`,
PASSED, while a proof was silently skipped. Enumerate the population, never the
answer.

**A METRIC THAT MOVES OPPOSITE TO THE TRUTH IT ENCODES IS WORSE THAN ONE THAT
DOES NOT MOVE.** #86 raised the pass total 3952 → 3956 *because it made a gap
visible*. Coverage did not move. The gate now prints the correction beside the
number every run. **Cite `IGNORED: 0`, never `3956 passed`.**

**A CONTROL IS IDENTIFIED BY THE MECHANISM IT HOLDS STILL.** I asked Athena for
a quiet box to test a *network* hypothesis. A quiet box controls CPU/disk. She
measured hex.pm instead: probe-run-probe, `200/200 → test ok → 200/200`. An
inference became a measurement, immediately, with no waiting.

**AN ERROR RESPONSE CAN BEAT A SUCCESS RESPONSE.** crates.io answering 403
*proves* reachability — the failure mode under test is "no answer", so any answer
refutes it.

**🔴 EXTRAPOLATING A RATE REQUIRES KNOWING WHETHER THE PROCESS HAS A NATURAL
END.** (Athena, correcting herself.) A good slope, no denominator: "compiler is
writing" read as "tank is draining". A rate has a numerator you can measure and a
denominator only the producer knows. Report the direction, not the ETA. Her
alarm would have cost more than the hazard.

**A REMEDY THAT NEEDS IDLENESS CANNOT BE TRIGGERED BY A CONDITION THAT ONLY
ARISES DURING WORK.** `rm -rf target/debug` needs nothing writing there; free
only falls while something is. Correct shape:
```
BEFORE a build:   free < 35 -> reclaim FIRST, then build
DURING, < 30:     a DECISION (let it run / kill it), never a reclaim
DURING, < 25:     Athena's hard floor, she acts
```
And the collapse: from ~38, reaching 30 means an "incremental" ate 8+ GiB — **the
disk number is reporting a SCOPE error, not a space problem.**

**A SCOPE DECLARATION COVERS EVERYTHING THE LANE RUNS, INCLUDING ITS GATES.** I
promised "incremental, I'll shout before anything wider" and then ran
`cargo clippy --workspace --all-targets`. My change was incremental; my
verification was not.

**A FIX'S SEARCH SPACE IS PART OF THE FIX.** #74 cured `tests/test_support/`
and pinned four copies; it missed `aion/src/engine/builder.rs` because that is
`src/`, and missed `aion-awl`'s copy because that was the ANCESTOR the cure was
ported FROM. **The source of a cure is the one place that does not receive it.**

**A GREP FOR A LITERAL SPAWN TARGET CANNOT FIND A CONFIGURABLE ONE.** Athena's
`Command::new("gleam")` sweep missed `aion-toolchain`, which takes an
operator-configured path — and being configurable is *what makes it the
production path*. Treat such a sweep as a lower bound.

**THE BETTER THE WORK, THE LOUDER THE FALSE POSITIVE.** `git grep '#\[ignore'`
returned 38 in a codebase whose whole cycle is about `#[ignore]` — every match
prose. Anchored to attribute position: 0.

## Board

| item | note |
|---|---|
| **#125** | COMMITTED `8c758d60`, battery running, then push |
| **#124** | out-of-target build leak, ~11.6 MiB, LOW. Glob trap: `CARGO_TARGET_DIR` points AT the temp dir, so `*/target` reads CLEAN |
| **#88** | needs its (a)/(b) ruling; the floor I tried was not option-independent, reverted at `9ccee6f9` |
| **#85** | open; `sqlite3_errmsg` lead REFUTED, cause unidentified |
| **#71** | GitHub Actions is DISABLED on the repo — a decision for Tom, not a build |
| **#7 / #77 / #82** | Tom's standing lanes: AWL properly, tharsis, the NIF-state P0 |
| **#108** | Tom's live server, pid 99844, running from a deleted binary — read-only probes ONLY |

### Follow-on found but NOT built (would need its own lane)

🔴 **`SHARED_COPIES` ENUMERATES THE ANSWER, NOT THE POPULATION** — the same
defect class as #86's tautological baseline, in the pin #74 shipped to prevent
divergence. Measured 2026-08-01 while ws4 ran:

```
613  crates/aion-awl-package/tests/test_support/gleam.rs   in SHARED_COPIES
613  crates/aion-cli/tests/test_support/gleam.rs           in SHARED_COPIES
613  crates/aion-server/tests/test_support/gleam.rs        in SHARED_COPIES
613  crates/aion/tests/test_support/gleam.rs               in SHARED_COPIES
398  crates/aion-awl/tests/test_support/gleam.rs           ← UNLISTED, unpinned
 77  crates/aion-package/tests/test_support/gleam.rs       ← UNLISTED, deliberate
```

`the_shared_gleam_support_copies_have_not_diverged` proves the four **listed**
copies are byte-identical. It cannot prove there are only four, because the list
IS the expectation. A seventh copy added tomorrow passes the pin by not being
written down. ⇒ **The fix is to enumerate the population**: walk the tree for
`tests/test_support/gleam.rs`, and require every path found to be either in
`SHARED_COPIES` or in an explicit, justified exclusion list. Then a new copy
fails until someone decides which it is.

The two unlisted copies are NOT the same kind of thing, and a fix that treats
them alike would be wrong:

- **`aion-awl`'s 398-line file is the ANCESTOR** the canonical 613 was ported
  FROM, and it never received the improvements made in transit. Its text still
  says "emitter defect" where the ported version says "code defect". This one is
  drift and should converge. (**The source of a cure is the one place that never
  receives it.**)
- **`aion-package`'s 77-line file is a deliberate, documented crate-local gate**
  — a different artefact, not a stale copy, and its own module doc explains why
  it is one definition included by three call sites including the `#[path]` from
  `src/structure/mod.rs`. It belongs in the **exclusion** list, with that reason
  written next to it, not converged.

**Verified NOT a gap:** every other test that spawns literal `gleam`
(`generate_types_first_e2e`, `new_scaffold_build`, `golden_gleam_exec`,
`schema_equivalence`, `nested_implicit_child_decode_e2e`,
`sdk_internal_privacy`) routes through the shared gate. The only ungated spawn
left is `aion-server/examples/failover_demo.rs`, which is an **example**, not a
test — it has no verdict to make vacuous, so it is out of scope for this class.
`aion-package/src/structure/tests.rs:1167` is gated via the `#[path]` above.

## Box rules

- ⛔ **Never bare `cargo clean`** — it takes `target/release`, which is Tom's
  #108 restart path. Reclaim is scoped `rm -rf target/debug` ONLY.
- `target/release/aion` sha256 `f748956ca23289a3…`, 58,245,680 bytes. Verified
  intact after the 21:44Z reclaim.
- ≥5 agent seats build on this box. **Free space is never "yours"** — state
  magnitude with ownership attached or not at all.
- 🔴 **Declare any detached run to Athena BEFORE the process exists.** Detaching
  severs the chain (PPID 1) and turns her attribution into my testimony.
