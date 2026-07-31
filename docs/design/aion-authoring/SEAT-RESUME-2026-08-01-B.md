# 📍 SEAT RESUME — 2026-08-01 (B)

**Supersedes `SEAT-RESUME-2026-08-01.md` for anything in flight. Read this first.**

## 🔴 STATE AT COMPACTION — one lane is BUILT AND UNCOMMITTED

**`origin/main` = `f7af328b`** (#76, pushed and green).

**#86 is fully built, gated, inverted-controlled — and NOT YET COMMITTED.**
A `--workspace` battery is running detached to close it.

```
pid 75765   cargo test --workspace --no-fail-fast   (detached, PPID 1)
log         <session scratchpad>/ws3.log            — Athena snapshots this
free        50 GiB at start
```

Athena Zooper Dooper holds a standing scoped reap authorisation for it and owns
a snapshot of the log (my scratchpad dies with my session, hers does not). If
the log is gone, ask her — she has `ws2-FINAL-verdict.txt` and the ws3 tick
snapshots.

### 🔴 RUN 3 IS NOT ALL GREEN — one failure, external, already diagnosed

`ws3.log:1157` `input_skeleton_round_trips_through_the_workflow_input_codec`
FAILED — `gleam export package-interface` could not reach `hex.pm`. **Expect
`WORKSPACE_EXIT=101`, and do NOT read it as the gate failing.** Whole-file scan
(not a tail): **1 non-clean verdict, 1 FAILED line, nothing else red.** Athena
A/B'd it against run 2 — same test, same log line number, `ok` twenty minutes
earlier. Transient network.

**Method note earned here: a TAIL IS A WINDOW, NOT A SCAN.** I declared "all
green so far" off `tail -c 400` while the failure sat 4 minutes back. Any claim
about a run's colour comes from a whole-file verdict scan:
`grep -n '^test result:' LOG | grep -v 'test result: ok\.'`

It opened **#125**, a real production defect: `generate.rs::export_package_interface`
has no registry classification, so on a hex.pm outage `aion generate` tells the
author *"the types module … must compile standalone"* — **blaming the operator's
source for a network outage.** #74 already built the correct classification
(`HTTP error` ∈ `REGISTRY_FAILURE_TITLES`, bounded retry, fail NAMED as
infrastructure, never skip) — it just stops at the test-support boundary and does
not follow the call through the binary. ⚠️ Do NOT "fix" this by gating the test:
#74 examined and refused that, because a fetch failure that skips lets the proof
pass while proving nothing.

### To finish #86

1. Read `ws3.log` for `WORKSPACE_EXIT=`. **A run without that line is TRUNCATED
   and its tally is not a coverage claim** — re-run rather than salvage.
2. Coverage control (the tally alone is not enough):
   ```
   awk '/^[[:space:]]+Running |^[[:space:]]+Doc-tests / { if (s) u++; s=1; t++ }
        /^test result:/ { s=0 }
        END { if (s) u++; print "targets:", t+0, "UNPAIRED:", u+0 }' ws3.log
   ```
   Expect `UNPAIRED: 0`. Note `^test result:` **needs its colon** — without it,
   a test named `result_...` inflates the count (that is the real 244-vs-243).
3. Commit the files listed below and push.

### 🔴 Two stale-doc fixes made AFTER the battery launched (post-compaction)

Both are comment-only, so the running battery still certifies the code:

1. `costly_gate.rs` named **`gates:costly-are-announced`** — a gate that does not
   exist (the real one is `gates:costly-announced`). Its whole argument is *"this
   conversion is only acceptable while that check exists"*, so a wrong name reads
   as **already deleted**, i.e. it argues for removing the thing it protects.
2. `scripts/battery.sh`'s comment block still **argued for the defect I removed** —
   "the expected count is derived from the `skip_unless_opted_in(` call sites …
   a baseline comparison encodes the invariant." That is the tautology that
   reported `4 vs 4` and PASSED while a proof was skipped. Left in place it is an
   instruction to reintroduce it. Rewritten to state why the list is deliberate:
   **enumerate the population, never the answer.**

Also unrelated and real: `examples/remote-gates/worker/Cargo.lock` was stale
since `16ba5157` (aion-package gained `jsonschema`; the example's lock never
caught up). A build regenerated it — **its own commit**, not part of #86.

### Files in the #86 change (all present in the tree, `cargo fmt`/clippy clean)

- `crates/aion-cli/tests/common/costly_gate.rs` — NEW, the runtime gate
- the five converted suites (see the table in `COSTLY-PROOF-GATES-86.md`)
- `scripts/battery.sh` — `gates:costly-announced` + the two-nouns comment
- `docs/design/aion-authoring/COSTLY-PROOF-GATES-86.md` — NEW, the record

Verified before compaction: `cargo fmt` 0, `cargo clippy --workspace
--all-targets -D warnings` 0, `bash -n scripts/battery.sh` 0, battery self-test
exit 1 (correct), the new gate green (5 suites, all announced) and RED under
mutation naming the right suite, `stacked_dev_live_e2e.rs` sha256-identical
after restore (`3512cec5…`).

## ✅ Closed today

- **#76** (`f7af328b`) — a transitive `serde_json/preserve_order` rewrites emitted
  bytes and nothing detected it. **The lane's whole result:** with `sorted_entries`
  mutated to not sort, the per-crate run was **0 failed / exit 0** and the
  `preserve_order` run was **11 failed**. Eleven tests whose names say "sorted"
  pass a build where nothing sorts. See `PRESERVE-ORDER-GATE-76.md`.
- **#117**, **#123**, **#44**, **#45** — earlier in the day, see resume marker A.

## Board

| item | note |
|---|---|
| **#86** | BUILT, UNCOMMITTED — finish per above |
| **#124** | out-of-target build leak: mechanism confirmed, **~11.6 MiB**, LOW priority. 🔴 The glob trap: `CARGO_TARGET_DIR` points AT the temp dir so the layout is `<tmp>/debug` — a hunt for `*/target` reads CLEAN. |
| **#88** | needs its (a)/(b) ruling — the floor I tried was not option-independent, reverted at `9ccee6f9` |
| **#85** | open investigation; `sqlite3_errmsg` lead REFUTED, cause unidentified |
| **#71** | GitHub Actions is DISABLED on the repo — a decision for Tom, not a build |
| **#7 / #77 / #82** | Tom's standing lanes: AWL properly, tharsis, the NIF-state P0 |
| **#117(c) live case** | run `0756ecd5`, still waiting on Tom's word to restart his server (#108) + a binary carrying (c) |

## 🔬 Method laws earned today — the durable part

Full catalogue is in my memory file `assert-the-invariant-not-the-enumeration.md`
(now 13 rules). The ones minted today:

**🔴 A BASELINE IS ONLY A CONTROL IF IT IS INDEPENDENT OF WHAT IT MEASURES.**
My #86 gate derived its expected count from the number of gate call sites, so
deleting a call deleted its own expectation — it reported `4 vs 4` and PASSED
while a proof was silently skipped. Two numbers moving together, agreeing
perfectly about nothing. I had "baseline-not-enumeration" written down as a law
and still built the same fact counted twice.

**A `cfg` LABEL LIES EXACTLY ON THE HAZARD.** A crate can only `cfg` on its own
features, so a forwarding `preserve-order` feature reports `false` while
`serde_json/preserve_order` is ON transitively — wrong in precisely the case it
exists to detect. Detect posture by BEHAVIOUR.

**A PATTERN THAT MATCHES THE SUBJECT MATTER MATCHES ITS SUCCESSES.** Athena's
`/error:/` matched 51 green `error::tests::` lines. Mine: `^test result` (no
colon) counted a test named `result_...`. And the #86 gate's bare token matched
cargo's dead-code warning **quoting the constant's own definition**.

**A VACUOUS ABSENCE CHECK LOOKS LIKE SUCCESS** — pair every "X is gone" with a
"Y is still here" under the SAME prefix; the positive arm is the only one whose
failure is loud.

**TWO POINTS ACROSS A STEP FUNCTION** give an artefact slope that extrapolates to
a deadline that never arrives. A rate claim needs the curve.

**AN ATTRIBUTED NUMBER IS MAGNITUDE + OWNER**, and only magnitude was measured.
There is a third agent seat building on this box (session `33ce25a4…`, repo
`libs/iridium`), so free-space movements are a sum over ≥3 consumers.

**SAMPLING A RUNNING POPULATION AS A RESTING ONE.** I nearly reported a temp-dir
leak measured mid-battery. Fix is not quiescence — it is a **timestamp against a
known event boundary**, which separates the populations in flight.

**COUNT VERDICT LINES, NEVER PROSE.** `^test result:` with the colon. And
`N passed` is a count of **pass EVENTS**, not distinct tests — subprocess tests
pass twice. 3952 events ≈ 3949 distinct.

## Box rules (unchanged, still true)

- `target/debug` was cleared once today; a cold full build costs ~25–37 GiB and
  needs ~40 GiB free. Name it to Athena BEFORE starting; **she grants, you
  execute** (she is read-only by standing constraint).
- ⛔ **Never bare `cargo clean`** — it takes `target/release`.
- `target/release/aion` and `.aion-accepted-binary-20260731/aion` are
  byte-identical (`f748956c…`) and are the restart path. Never sweep either.
- **Do not go near pid 99844** (Tom's live server) beyond read-only probes.
- 🔴 **Declare any detached run to Athena BEFORE the process exists.** Detaching
  severs the process chain (PPID 1), so her attribution becomes my testimony.
