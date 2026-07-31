# 📍 SEAT RESUME — 2026-08-01 (F)

**Supersedes `SEAT-RESUME-2026-08-01-E.md`. Read this first.**

I am **Vesper Lynd**, session `5849e0d8-4802-4869-8e0f-9f7fd187e198`. Sign
`— Vesper`. **Fable is the MODEL I run on, not my name.**

## 🔴 STANDING ORDER CHANGED 2026-08-01 (Tom, via MU/TH/UR broadcast)

> "Delegate the work; do not do it in your own context. Spawn subagents on the
> Opus model... Keep the fleet modest... Your own seat's job is to dispatch,
> then verify and validate what comes back: check the claims, re-read the
> sources, confirm the tests actually ran. Doing the implementation yourself is
> the thing to stop."

This **supersedes** the session-prompt line "Do NOT call the AgentTool unless
the user requested it" — the user has now requested it. Agents run on **Opus**.
⚠️ Tom's NO-WORKTREES order still stands, so file-mutating agents run **one at
a time** in the main tree; parallelism is for read-only work.

## State

```
origin/main  3bee2840
local  HEAD  404b017f   (c5191054 = #90 WIP, + marker E)
```

Uncommitted working tree = the #90 second-site fix + reword (below).

## 🔴 #90 — where it actually stands

`c5191054` was committed as WIP and **was not the whole fix**. Two things were
found this session by sweeping rather than by a second sighting.

### Finding 1 — the committed wording was itself a false mechanism claim

`c5191054` wrote the head `"retry budget exhausted; last attempt: "`. Sweeping
the producers of a retained activity error shows a `retryable:`-classed reason
reaches a `Terminal` record from **two** producers:

- `nif_activity_retry_dispatch` — the retry loop giving up. A budget was spent.
- `nif_activity_in_vm` — the in-VM completion watcher, which has **zero** retry
  symbols in the file. A typed `error.Retryable` is delivered straight through,
  never retried (this is open defect #34).

So on the in-VM path the head named a mechanism that never ran — trading #90's
false claim for a new one of the same kind. Reworded to
`"no further attempts; last attempt was retryable-classed: "`, which states only
what the call site can know, and renamed `exhausted_terminal_reason` →
`reclassified_terminal_reason` because the function name carried the same
over-claim.

### Finding 2 — there were TWO consumers, and only one was fixed

`nif_collect_settlement.rs:220` is byte-for-byte the same defect as the await
path: same source (`take_activity_error`), same `Terminal` kind, message
verbatim. **Every `distribute` leg settles through it.** Fixed identically.

**Consumers are exactly two** (`take_activity_error` non-test callers:
`nif_activity_await.rs:194`, `nif_collect_settlement.rs:220`; a third at :275 is
a discard). The fix is applied at the CONSUMERS, deliberately not at any
producer — the consumer is the only place that knows a `Terminal` record is
what is being written, and there are three producers that would each need it.

### Finding 3 — the defect had a test DEFENDING it

`nif_activity_dispatch_tests/await_paths.rs:189`
(`awaited_terminal_records_the_noted_final_attempt`) asserts the terminal
carries `"retryable:reset three"` **verbatim**. It has been RED since
`c5191054` and I never saw it because I only ever ran the filtered
`--lib nif_activity_retry`. This is why #90 survived: it was pinned as expected
behaviour. **THIS IS THE ONLY THING LEFT TO FIX.**

### Mutation proof — DONE, four arms, all watched failing

| arm | mutation | expected | observed |
|---|---|---|---|
| A | `reclassified_terminal_reason` → identity | core pin RED, pass-through GREEN | ✅ RED, message printed the real production bytes |
| B | rewrite unconditionally (drop `strip_prefix`) | core GREEN, non-vacuity pin RED | ✅ 2 RED (non-vacuity + leading-only) |
| C | second site reverted to raw | new fan-out pin RED | ✅ RED on the class assertion |
| D | half-fix: record corrected, return raw | new pin RED | ✅ RED — but on assertion 1, NOT the equality one |
| E | mirror half-fix: record raw, return corrected | equality assertion RED | ✅ RED at settlement.rs:97, left/right IS the divergence |

**Arm E existed only because arm D fired on the wrong assertion.** D looked like
a pass for the equality check and proved nothing about it. Both pristine files
restored and `cmp`-verified byte-identical after each arm.

### Gates run so far

```
cargo test -p aion-rs --lib nif_activity_retry   12 passed / 0 failed / 0 ignored
cargo test -p aion-rs --lib nif_collect          23 passed / 0 failed / 0 ignored
cargo clippy -p aion-rs --all-targets -D warnings  EXIT 0, 0 warning/error lines
cargo test -p aion-rs --lib                      627 passed / 1 failed  <- Finding 3
```

### 🔴 REMAINING, in order

1. **Fix the stale pin** (Finding 3) — DISPATCHED to an Opus subagent.
   It must keep its real NOI-0 intent (the recorded terminal carries
   `attempt: 3`) and update only the message expectation, and it must say in a
   comment that it previously pinned the defect.
2. `cargo test -p aion-rs --lib` green.
3. Cold **workspace** battery, then push. Expect one red: **#85**.
4. Write the record doc `RETRY-CLASS-HONESTY-90.md`.

## ⚠️ Battery method — do not re-derive

```
grep 'WORKSPACE_EXIT=' LOG            # no marker = TRUNCATED, re-run
grep -n '^test result:' LOG | grep -v 'test result: ok\.'
grep -n '^test .* FAILED$' LOG
awk '/^[[:space:]]+Running |^[[:space:]]+Doc-tests / { if (s) u++; s=1; t++ }
     /^test result:/ { s=0 }
     END { if (s) u++; print "targets:", t+0, "UNPAIRED:", u+0 }' LOG
```
Baseline: **targets 240 / UNPAIRED 0 / verdicts 243 / IGNORED 0.** The 243>240
gap is `aion-server` re-invoking its own test binary 3× under a filter (#74
child-process pin). **Explained — do not re-investigate.**

🔴 **EXPECT ONE RED: #85.** `outbox_e2e.rs`, `SQLite failure: bad parameter or
other API misuse`, ~37.5% per-run. **Do not settle it statistically** — three
greens at p=0.375 is a 24% draw. Settle attribution *structurally*:
```
strings <subject-binary> | grep -c <symbol>   -> 0
strings <control-binary> | grep -c <symbol>   -> 1   <- the control is the point
```
Athena's refinement: a positive control only validates the instrument on the
**same kind of artifact** — `.rcgu.o` objects return 0 for the control too.

## 🔬 Method laws earned this session

**🔴 A DEFECT CAN HAVE A TEST DEFENDING IT.** A green suite is not evidence the
behaviour is wanted; it may be evidence the behaviour was pinned. When a fix
turns a suite red, read the red before assuming it is yours to revert.

**🔴 A FILTERED GATE HIDES ITS OWN BLAST RADIUS.** `--lib <filter>` was green
across the whole WIP commit while a sibling module's pin was red. The filter
was chosen from the module I edited, so it could only ever see what I already
knew about.

**🔴 A HALF-FIX ARM CAN FIRE ON THE WRONG ASSERTION.** Arm D killed the test via
assertion 1 and told me nothing about assertion 3. To prove a specific
assertion load-bearing, the mutation must be the one that leaves every OTHER
assertion satisfied. (Mirror the half-fix.)

**🔴 A FIX'S SEARCH SPACE IS PART OF THE FIX — and the search must run over the
CONSUMERS of the bad value, not the sites that resemble the one you found.**
Grepping `ActivityErrorKind::Terminal` gave 40 hits and would have buried it;
grepping `take_activity_error` gave 2 and both were defects.

**🔴 A WORDING THAT NAMES A MECHANISM IS A CLAIM ABOUT HISTORY.** Prefer a fact
about the record being written; that is the only thing a call site can know
without knowing which producer sent it.

Prior laws remain in `assert-the-invariant-not-the-enumeration.md` and marker E.

## Board

| item | note |
|---|---|
| **#90** | fix complete + 4-arm mutation-proven; stale pin dispatched; battery + push remain |
| **#34** | corroborated this session — the in-VM path really does deliver retryable and never retry |
| **#7 / #77 / #82** | Tom's standing lanes: AWL properly, tharsis, NIF-state P0 |
| **#85** | sixth test observed; lead = SQLITE_MISUSE + mismatched `sqlite3_errmsg` in vendored libsql |
| **#124 / #88 / #71** | see E; #71 is a Waffles/Cally decision |
| **#108** | Tom's live server pid 99844, deleted binary — read-only probes ONLY |

### Follow-on not built

**Converge `aion-awl/tests/test_support/gleam.rs`** — named in
`DIVERGENT_COPIES`, still divergent. Not a `cp`: different `compile_project`
signature, 13 test files call it.

## Box rules

- ⛔ **Never bare `cargo clean`** — it takes `target/release`, Tom's #108
  restart path. Reclaim is `rm -rf target/debug` ONLY, and pair the absence
  check with a survival check under the SAME prefix.
- **#108 pin, disambiguated 2026-08-01** (earlier markers were ambiguous and
  the two numbers look like corruption if you read them the wrong way round):
  - `target/release/aion` **the binary** — 58,245,680 bytes (~55.5 MiB),
    sha256 `f748956ca23289a3376d196219214bb9c903f083fffb0093cbbd573413ef4d8a`
  - `target/release` **the directory** — 1,427,144 KiB. This is what the
    "1,427,144 KiB" in markers D/E measured. It is NOT the binary.
  - Both verified intact 2026-08-01; Tom's server pid 99844 running from it.
- **BEFORE a build: free < 35 → reclaim FIRST.** DURING: a DECISION, never a
  reclaim.
- ≥5 agent seats build on this box. **Free space is never "yours."**
- 🔴 **Declare any detached run to Athena BEFORE the process exists**
  (`dm:45cf420e-621d-467b-b96c-5bffb62c04f3`). ⚠️ Waffles runs the identical
  battery command in `apps/manifold/.worktrees/tree-registry` — **cwd is the
  only discriminator.**
