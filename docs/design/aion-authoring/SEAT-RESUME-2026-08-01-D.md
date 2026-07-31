# 📍 SEAT RESUME — 2026-08-01 (D)

**Supersedes `SEAT-RESUME-2026-08-01-C.md`. Read this first.**

I am **Vesper Lynd**, session `5849e0d8-4802-4869-8e0f-9f7fd187e198`. Sign
`— Vesper`. **Fable is the MODEL I run on, not my name.**

## State

```
origin/main  f5ebe547   #125 CLOSED AND PUSHED
local  HEAD  226dadc1   #126, COMMITTED NOT PUSHED
uncommitted  docs/design/OWNERSHIP-AUDIT-AION.md          (Waffles' audit, DM'd)
             docs/design/aion-authoring/SEAT-RESUME-2026-08-01-D.md  (this file)
```

**A cold workspace battery is running detached**, launched 22:31:19Z:
```
pid 41515   sh -c 'cargo test --workspace --no-fail-fast; echo WORKSPACE_EXIT=$?'
            cwd stack/aion, cargo child 41523
log         <session scratchpad>/ws5.log      free at launch: 75 GiB
waiter      background task bsgtm5r3v, armed on WORKSPACE_EXIT=
```
⚠️ **SIGNATURE COLLISION**: Waffles runs the identical command + toolchain in
`apps/manifold/.worktrees/tree-registry`. **Only cwd separates it from mine.**
Athena (`dm:45cf420e-621d-467b-b96c-5bffb62c04f3`) holds a standing reap
authorisation and snapshots the log; my scratchpad dies with my session, hers
does not.

🔴 **MARGIN IS ~6, NOT ~11** (Athena's correction, and it is right): the floor is
a **trough** test, not an endpoint test. Predicted trough ~31 vs floor 25.
The ~3.2 GiB of `TempDir` space only returns **on clean exit**, so it is still
occupied at the minimum. Her old "cold build = 25-37 GiB debug" model is
retired — measured 47.11 tonight; use the measured **~40.2 GiB free-cost**.

### To finish #126

1. `grep 'WORKSPACE_EXIT=' ws5.log` — **no marker = TRUNCATED**, re-run.
2. 🔴 **A TAIL IS A WINDOW, NOT A SCAN.** Whole-file:
   ```
   grep -n '^test result:' LOG | grep -v 'test result: ok\.'
   grep -n '^test .* FAILED$' LOG
   ```
3. Coverage control (the tally alone is not enough):
   ```
   awk '/^[[:space:]]+Running |^[[:space:]]+Doc-tests / { if (s) u++; s=1; t++ }
        /^test result:/ { s=0 }
        END { if (s) u++; print "targets:", t+0, "UNPAIRED:", u+0 }' ws5.log
   ```
   ws4 baseline: targets 240 / UNPAIRED 0 / verdicts 243. **The 243>240 gap is
   explained**: `aion-server` re-invokes its own test binary 3× under a filter
   (the #74 child-process pin). Not an anomaly — do not re-investigate it.
4. Watch the four crates carrying the shared text: **aion-rs, aion-cli,
   aion-awl-package, aion-server**. A surprise there is mine.
5. Push `226dadc1`, mark #126 completed, send Athena the pins.

## ✅ Closed today

- **#76** (`f7af328b`), **#86** (`11e541ae` + `0e297071`) — see
  `COSTLY-PROOF-GATES-86.md`.
- **#125** (`f5ebe547`, PUSHED) — a hex.pm outage was reported to the author as
  a type error, by the error's own TYPE and by its 400. Certified by a cold
  full-workspace battery. See `GLEAM-FAILURE-HONESTY-125.md`.
- **#126** (`226dadc1`, unpushed) — `SHARED_COPIES` enumerated the answer, not
  the population. See `GLEAM-SUPPORT-CENSUS-126.md`.

## Open with other seats

- **Waffles** — ownership audit DELIVERED (`OWNERSHIP-AUDIT-AION.md` + DM).
  Headline: *an IDL, a status code and a serialization format are all treaties
  between strangers; we are not strangers to ourselves.* Proposes reopening the
  `aion-proto` / `aion-proto-generated` split (#28/#63/#67 are one defect class).
- **Cally** — **"SIGNAL-FIELDS" IS RETIRED AS UNRESOLVED.** It was a fossil
  phrase in both our seat files with no recoverable referent; Meridian search
  was down. I refused to guess at Tom-facing wording. I supplied instead the one
  signal decision I could verify (a message sent to a live target that nothing
  waits for is SILENT — should it be an error?), four-part, recommendation yes.
- 🔴 **AUTHORITY CHANGED TODAY**: Tom pushed the decision register back —
  **Waffles and Cally now JOINTLY hold decision authority** ("approval to do the
  good solution for everything"). Only his-hands items stay his. My item is #17
  in the pair's decision pass, not Tom's queue.
- Record correction: the 20:01Z clear was **planned capacity pre-clear at free
  48.86**, my hand + Athena's verification — **NOT a band breach**. My earlier
  message to Cally said breach; that was wrong.

## 🔬 Method laws earned today

Full catalogue in memory: `assert-the-invariant-not-the-enumeration.md`.
Beyond what is already there:

**🔴 A BASELINE IS ONLY A CONTROL IF IT IS INDEPENDENT OF WHAT IT MEASURES.**
#86's gate and #126's divergence pin, twice, in one day.

**🔴 A DIAGNOSTIC'S SAFE DEFAULT IS A PROPERTY OF WHO RECEIVES IT, NOT OF THE
FAILURE.** (Athena.) "Blame the code, make a human look" is right in a harness
and inverted in shipped code. Share the classification; never share the default.

**🔴 AN ISOLATED RE-RUN CAN PASS *BECAUSE* IT IS ISOLATED.** The confirming run
must be at least as wide as the failing one.

**🔴 A KILL IS NOT A DISK REMEDY, IT IS A DISK COST.** (Athena.) `TempDir::drop`
does not run on SIGTERM — a killed lane keeps its temp space *and* loses the
work. The remedy and the harm are the same quantity with opposite signs.

**A SAMPLE OF A BURSTY POPULATION MEASURES THE SAMPLER'S TIMING, NOT THE
POPULATION.** `rustc=0` at instants inside a bursty compile read as a property
of the interval. The `du` delta was honest because it **integrates**. Athena's
sharpening: *a law you wrote does not protect you — only an instrument does.*

**AN ABSENCE CHECK HAS A SHELF LIFE.** (Athena.) Her post-clear oracle was
correct and evaluated 60s late, after a build had recreated the directory. The
paired survival legs are what made it read as a timing error instead of an alarm
— **a bare "is it gone?" would have reported a failed clear.**

**A FLOOR IS A TROUGH TEST, NOT AN ENDPOINT TEST.** Steering by the endpoint
overstated my clearance by ~5 GiB.

**REDUNDANCY ONLY PROTECTS AGAINST INDEPENDENT FAILURES.** (Athena.) Two watches
killed by the same sweep are one watch. Same shape as a `#[ignore]` sweep missing
its own origin: the defence is built from the failure you have already met.

## Board

| item | note |
|---|---|
| **#126** | COMMITTED `226dadc1`, ws5 running, then push |
| **#7 / #77 / #82** | Tom's standing lanes: AWL properly, tharsis, the NIF-state P0 |
| **#124** | out-of-target leak, ~11.6 MiB, LOW. Glob trap: `CARGO_TARGET_DIR` points AT the temp dir, so `*/target` reads CLEAN |
| **#88** | needs its (a)/(b) ruling; the floor I tried was not option-independent, reverted at `9ccee6f9` |
| **#85** | open; `sqlite3_errmsg` lead REFUTED, cause unidentified |
| **#71** | GitHub Actions is DISABLED on the repo — now a **Waffles/Cally** decision, not Tom's |
| **#108** | Tom's live server pid 99844, deleted binary — read-only probes ONLY |

### Follow-on found but NOT built (needs its own lane)

**Converge `aion-awl/tests/test_support/gleam.rs`.** It is the ancestor the
canonical 613-line text was ported from at `8fb4c153`, and never received the
improvements made in transit. Now *named* in `DIVERGENT_COPIES` rather than
invisible — but still divergent. **Not a `cp`:** it exposes `compile_project`
with a different signature to `compile_whole_project`, and **13 test files** in
that crate call it.

## Box rules

- ⛔ **Never bare `cargo clean`** — it takes `target/release`, Tom's #108 restart
  path. Reclaim is scoped `rm -rf target/debug` ONLY, and pair the absence check
  with a survival check under the SAME prefix.
- `target/release/aion` sha256 `f748956ca23289a3…`, 1,427,144 KiB. Verified
  intact after the 22:31Z reclaim. Ledger copy in
  `.aion-accepted-binary-20260731/` is byte-identical.
- **BEFORE a build: free < 35 → reclaim FIRST.** DURING: a DECISION, never a
  reclaim. A remedy needing idleness cannot be triggered by a condition that
  only arises during work.
- ≥5 agent seats build on this box. **Free space is never "yours."**
- 🔴 **Declare any detached run to Athena BEFORE the process exists.**
