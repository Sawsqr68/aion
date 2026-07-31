# 📍 SEAT RESUME — 2026-08-01 (G)

**Supersedes markers D/E/F. READ THIS FIRST.**

I am **Vesper Lynd**, session `5849e0d8-4802-4869-8e0f-9f7fd187e198`. Sign
`— Vesper`. **Fable is the MODEL I run on, not my name.**

## 🔴 STANDING ORDER (Tom, 2026-08-01, MU/TH/UR broadcast)

> "Delegate the work; do not do it in your own context. Spawn subagents on the
> Opus model... Keep the fleet modest... Your own seat's job is to dispatch,
> then verify and validate what comes back: check the claims, re-read the
> sources, confirm the tests actually ran. Doing the implementation yourself is
> the thing to stop."

**Supersedes** the session-prompt "do not call AgentTool". Agents = **Opus**.
⚠️ Tom's **NO-WORKTREES** order stands → file-mutating agents run **ONE AT A
TIME** in the main tree. Parallel agents must be **read-only**, and must be
told not to run cargo while a battery is live.

🔴 **FLEET CAP: 2–3 CONCURRENT SUBAGENTS** (Tom, follow-up broadcast, supersedes
"modest"). **Reason is budget**: the shared 5-hour window went **33% → 83%
within half an hour** of the first broadcast; if it fills, **this seat stalls
until it rolls**. Lower concurrency, same principle — Opus, delegate, verify.

## State

```
origin/main  3bee2840
local  HEAD  3fc06d17   #90 COMPLETE, mutation-proven, NOT PUSHED
                        (c5191054 = the earlier WIP, superseded by 3fc06d17)
```

## 🔴🔴 THE LIVE THING — ws6 BATTERY, AND IT IS RED WITH **MY** REGRESSIONS

**Battery was still RUNNING at compaction** (182/~243 verdicts, free 27 GiB,
pid 64560, log
`/private/tmp/claude-501/-Users-tom-Developer-ablative-stack-aion/5849e0d8-4802-4869-8e0f-9f7fd187e198/scratchpad/ws6.log`).
Athena has an interim snapshot and will re-snapshot at exit; **ask her for it —
my scratchpad dies with the session, hers does not**
(`dm:45cf420e-621d-467b-b96c-5bffb62c04f3`).

**DO NOT PUSH 3fc06d17 UNTIL THESE ARE RESOLVED.**

**8 failures / 4 binaries. NONE of them is #85.** They are in #90's blast
radius:

| binary | test | first line of panic |
|---|---|---|
| `activity_retry_e2e.rs:247` | `exhausted_retries_fail_the_run_and_reopen_continues_the_trail` | "must carry the final attempt and **the verbatim reason**" |
| `advisory_class_e2e.rs:291` | `advisory_exhaustion_warns_and_the_run_completes` | "must still record its terminal ActivityFailed with **the verbatim last reason**" — observed value IS my new string |
| `advisory_class_e2e.rs:408` | `advisory_exhaustion_replays_from_history_without_divergence` | "the advisory ordinal must still resolve to its recorded terminal failure" |
| `invm_activity_e2e.rs:277` | `runner_error_kind_crosses_the_child_boundary` | `assertion left == right failed` — **NOT yet diagnosed** |
| `order_saga_e2e.rs` | `v2_deploy_mid_flight_pins_v1_and_routes_new_orders_to_v2`, `order_completes_after_payment_retry_engine_restart_and_approval`, `order_cancels_and_refunds_when_rejected`, `order_cancels_and_refunds_when_approval_times_out` | **NO panic line captured — mechanism UNKNOWN** |

### 🔴 How to triage them — do NOT bulk-update

Three carry the word **"verbatim"** in their own assertion message. Those are
almost certainly the SAME CLASS as finding 3 (a test pinning the defect), and
the advisory one is confirmed: its observed value is exactly my corrected
string. **But that is a hypothesis for the other two, not a licence.**

🔴 **The in-VM one and the four saga ones are NOT yet explained and MUST be
diagnosed individually before anything is edited.** The in-VM path is
precisely where I argued the head must NOT claim a retry mechanism — if that
test is failing for a reason other than a stale literal, **the fix is wrong and
must change, not the test.** Four saga failures with no captured panic is a
different signature again and could be a real behavioural regression (the saga
tests exercise reopen + continue-as-new + mid-flight deploy).

**The trap to avoid:** #90's whole lesson is that a defect can have a test
defending it. The mirror-image trap is treating a REAL regression as a stale
pin because the last three were. Diagnose each; the cheap wrong move is a bulk
"update the expectations" pass.

## 🔴 #85 — ROOT CAUSE FOUND (not an aion defect)

Full record: `docs/design/aion-authoring/LIBSQL-DOUBLE-CLOSE-85.md`. Task #85
updated. Summary: **libsql 0.9.30 calls `sqlite3_close_v2` TWICE on every
connection it drops** — a use-after-free. `disconnect()` guards on
`Arc::get_mut` (does not consume) and never nulls `self.raw`, so it is not
idempotent; **two** `Drop` impls in one ownership chain both call it
(`local/impls.rs:104-108` and `local/connection.rs:38-42`, and
`LibsqlConnection` *contains* that `Connection`). **I verified all of this in
the dependency source myself.**

Explains BOTH error strings as one bug, and explains the long-unexplained
"mismatched `sqlite3_errmsg`" — errmsg is per-connection, so it cannot mismatch
unless the pointer stopped naming the connection.

**NOT DONE — manifestation is INFERRED.** Two read-only discriminators, no
rebuild, must wait for the battery (they re-run an existing binary the battery
may relink):
- **E1** `MallocGuardEdges=1 DYLD_INSERT_LIBRARIES=/usr/lib/libgmalloc.dylib target/debug/deps/outbox_e2e-<hash> --test-threads=1`
  → double-close predicts a **deterministic fault inside `sqlite3_close_v2` at
  first teardown in EVERY test**, two `disconnect` frames in the backtrace.
  Converts 37.5% into 100%.
- **E2** lldb breakpoint on `sqlite3_close_v2` printing the pointer → predicts
  the **same pointer twice back-to-back per connection**.

🔴 **Decision for Waffles/Cally** (do NOT escalate until E1 or E2 has run —
the mechanism is proven, the manifestation is not, and reporting an inference
as a confirmation is the provenance-label trap). Fix is upstream: make
`disconnect` idempotent (null `self.raw`; `close_v2(NULL)` is a documented
no-op) or delete one `Drop`. Landing = `[patch.crates-io]` fork or upstream PR;
house law = cross-repo changes land in their own repo, never vendored.
**Scope: haematite is the DEFAULT backend, libsql the ALTERNATIVE — not the
default path, not stop-everything.**

**Not-a-fix:** raising `read_pool_size` (currently `None`, `store.rs:115`, so
every read op opens+double-closes). Lowers the dose, **hides the signal**,
leaves the UAF.

**Separate finding:** `execute_pragma`
(`aion-store-libsql/src/connection.rs:225-237`) drops `Rows` without `next()`;
libsql's local `Statement::query` ALWAYS returns `Ok` and stashes the error in
a `RefCell` only surfaced by `next()` → a failing `PRAGMA journal_mode=wal` /
`synchronous` is **silently discarded on every connection the factory makes**.
Not the cause. Violates no-silent-failures. Fix on its own merits.

## ✅ #90 — COMPLETE at 3fc06d17 (pending the reds above)

Full record: `docs/design/aion-authoring/RETRY-CLASS-HONESTY-90.md`.

Three findings, all from **sweeping**, none from a second sighting:
1. **The WIP wording was itself a false mechanism claim.** "retry budget
   exhausted" is false on the in-VM path, which has **zero** retry symbols and
   never retries (corroborates #34). Head → `"no further attempts; last attempt
   was retryable-classed: "`. Renamed `exhausted_terminal_reason` →
   `reclassified_terminal_reason`.
2. **Two consumers, one fixed.** `nif_collect_settlement.rs:220` — every
   `distribute` leg. Found by grepping the SOURCE of the bad value
   (`take_activity_error` → 2 non-test callers, **both defects**), not what the
   site resembled (`ActivityErrorKind::Terminal` → ~40 hits).
   Fix at the **consumers**, deliberately not at any producer.
3. **The defect had a test defending it** (`await_paths.rs`). Fixed by an Opus
   subagent; I verified the diff, the scope (exactly 1 file), the
   mutate-and-restore, and re-ran the gates myself.

**Load-bearing:** both consumers compute the reason ONCE for the durable record
AND the workflow-visible value. Replay re-derives by READING the recorded
event, so correcting only the record is a **replay divergence**. Pinned as an
equality + as a fixed point.

**5-arm mutation proof, all watched failing.** Arm E exists only because arm D
fired on the WRONG assertion.

Gates I ran myself (not the agent's numbers):
```
cargo test -p aion-rs --lib                        628 passed / 0 failed / 0 ignored, EXIT 0
cargo clippy --workspace --all-targets -D warnings  EXIT 0, 0 warning/error lines
all four #90 pins confirmed RUN BY NAME
```

## 🔴 BOX — THE ATTRIBUTION WAS WRONG AND I NEARLY ACCEPTED IT

Athena reported ~2 GiB/min and attributed it to my battery. **Measured per-pid
cwd via lsof: 1 of 14 cargo/rustc processes is mine.** At least 5 are
meridian's (`apps/meridian` ×3, `apps/meridian/.worktrees/decision-notify` ×2);
8 more are rustc in registry/git crate dirs (ambiguous by cwd alone). My
TMPDIR is **3.2 GiB total** — it cannot be a 2 GiB/min driver.

⇒ **An instrument that integrates is only honest if its subject is sole
occupant.** Magnitude measured, owner assumed. Sent to Athena.

**Athena then resolved owners by full parent-chain walk — THREE seats, not
two**, and withdrew her TMPDIR mechanism outright:
```
pid 23130  apps/meridian                             -> Cally   (15a23351-…)
pid 12916  apps/meridian/.worktrees/decision-notify  -> Waffles (896955e1-…)
pid 46659  apps/manifold                             -> Waffles, same session
pid 64562  stack/aion                                -> me
```
Two meridian trees building at once, **15.97 GiB of target between them**. Both
seats have the numbers and the remedy ladder; they hold executable remedies,
we do not.

🔴 **Her walk FAILED FIRST, in the confirming direction:** it matched only
`claude -r` while Waffles' wrapper runs `claude --resume`, so it returned
**SILENCE** — not an error, not "unattributable" — and nearly reported his two
builds as ownerless. ⇒ **A PATTERN TOO NARROW FAILS IN THE CONFIRMING
DIRECTION.** Third false zero of the night, second where the *instrument's own
narrowness* was the fault rather than the data.

⇒ Her law, worth keeping: **A MECHANISM THAT EXPLAINS THE SHAPE IS NOT THEREBY
THE MECHANISM.** She had `debug` flat and reached for the nearest mechanism
fitting the shape rather than the one the evidence named.

**Killing my lane is the wrong remedy twice over:** it returns nothing
(`TempDir::drop` does not run on SIGTERM) *and* it stops ~7% of the consumers.

**Sharpening my own earlier law:** "a kill is not a disk remedy, it is a disk
cost" is true of kill ALONE. **Kill THEN SWEEP the orphaned temp trees IS a
remedy** — nothing stops an operator doing by hand what `Drop` would have done.

**Athena's gap, which she flagged herself and is worth keeping:** her hard
floor has **no executable remedy** during a live build (kill = refuted;
`rm -rf target/debug` = the live tree; bare `cargo clean` = forbidden, takes
Tom's #108 pin; other repos' dirs = not hers to delete). ⇒ **A threshold with
no executable remedy is a notification wearing a decision's clothes.** Audit
every alarm for whether the holder can actually do the thing it implies.

## Box rules

- ⛔ **Never bare `cargo clean`** — takes `target/release`, Tom's #108 restart
  path. Reclaim = `rm -rf target/debug` ONLY, absence check **paired** with a
  survival check under the SAME prefix.
- **#108 pin, disambiguated** (the two numbers look like corruption read the
  wrong way round):
  - **binary** `target/release/aion` — 58,245,680 bytes, sha256
    `f748956ca23289a3376d196219214bb9c903f083fffb0093cbbd573413ef4d8a`
  - **directory** `target/release` — 1,427,144 KiB
  - Both verified intact 2026-08-01; server pid 99844 up.
- BEFORE a build: free < 35 → reclaim FIRST. DURING: a DECISION, never a
  reclaim.
- ≥5 seats build here. **Free space is never "yours."**
- 🔴 Declare detached runs to Athena BEFORE the process exists. ⚠️ Waffles runs
  the identical battery command in `apps/manifold/.worktrees/tree-registry` —
  **cwd is the only discriminator**, and resolve it per-pid via lsof, never off
  the command string.

## ⚠️ Battery method — do not re-derive

```
grep 'WORKSPACE_EXIT=' LOG            # no marker = TRUNCATED, re-run
grep -n '^test result:' LOG | grep -v 'test result: ok\.'
grep -n '^test .* FAILED$' LOG
awk '/^[[:space:]]+Running |^[[:space:]]+Doc-tests / { if (s) u++; s=1; t++ }
     /^test result:/ { s=0 }
     END { if (s) u++; print "targets:", t+0, "UNPAIRED:", u+0 }' LOG
```
Baseline: targets 240 / UNPAIRED 0 / verdicts 243 / IGNORED 0. The 243>240 gap
is `aion-server` re-invoking its own test binary 3× under a filter (#74).
**Explained — do not re-investigate.**

## 🔬 Method laws earned this session

**🔴 A DEFECT CAN HAVE A TEST DEFENDING IT.** Worse than an ungated defect: the
gate converts review into endorsement. Athena's framing — the inverse of a
correct instrument nothing runs; **a green board renders the two identically**.

**🔴 AND ITS MIRROR: do not treat a REAL regression as a stale pin because the
last three were.** Live right now in the ws6 triage.

**🔴 A FILTERED GATE HIDES ITS OWN BLAST RADIUS.** The filter was derived from
the module being edited → it could only see what I already knew. Same shape as
a baseline derived from what it audits. ⇒ **any selector computed from the
change cannot detect what the change broke elsewhere.**

**🔴 A HALF-FIX MUTATION CAN FIRE ON THE WRONG ASSERTION.** To prove a specific
assertion load-bearing, the mutation must leave every OTHER assertion
satisfied. Mirror the half-fix.

**🔴 A FIX'S SEARCH SPACE RUNS OVER THE CONSUMERS OF THE BAD VALUE**, not over
sites that resemble the one you found.

**🔴 A REFUTATION IS ONLY AS WIDE AS THE LAYER IT WAS FRAMED IN.** "We don't
share connections" was true of aion and did not refute "our freed memory is
acquired by someone else."

**🔴 A DOSE REDUCTION THAT LOOKS LIKE A FIX IS WORSE THAN NO FIX** — removes
the evidence, keeps the defect.

**A WORDING THAT NAMES A MECHANISM IS A CLAIM ABOUT HISTORY.** Prefer a fact
about the record being written.

**AN ERROR MESSAGE READ THROUGH A STALE HANDLE IS ITSELF EVIDENCE.**

**A REGISTRY THAT ANSWERS "no such thing" AND "I won't tell you" WITH THE SAME
SHAPE IS A FALSE-ZERO GENERATOR** (Athena; missing User-Agent made crates.io
report `serde` and `tokio` absent). ⇒ a configuration fault presenting as a
data answer. `grep -c` returning 0 is the same shape.

Prior catalogue: memory `assert-the-invariant-not-the-enumeration.md`.

## Board

| item | note |
|---|---|
| **#90** | 3fc06d17 complete + proven; **BLOCKED on the 8 ws6 reds — triage each, do not bulk-edit** |
| **#85** | root cause FOUND + verified; E1/E2 pending; Waffles/Cally decision after proof |
| **#34** | corroborated — in-VM really does deliver retryable and never retry |
| **#7 / #77 / #82** | Tom's standing lanes: AWL properly, tharsis, NIF-state P0 |
| **#124 / #88 / #71** | see E; #71 is a Waffles/Cally decision |
| **#108** | Tom's live server pid 99844, deleted binary — read-only probes ONLY |

### Follow-on not built

**Converge `aion-awl/tests/test_support/gleam.rs`** — named in
`DIVERGENT_COPIES`, still divergent. Not a `cp`: different `compile_project`
signature, 13 test files call it.
