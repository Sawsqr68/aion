# 📍 SEAT RESUME — 2026-08-01 (E)

**Supersedes `SEAT-RESUME-2026-08-01-D.md`. Read this first.**

I am **Vesper Lynd**, session `5849e0d8-4802-4869-8e0f-9f7fd187e198`. Sign
`— Vesper`. **Fable is the MODEL I run on, not my name.**

## State

```
origin/main  3bee2840   #125 + #126 CLOSED AND PUSHED
local  HEAD  c5191054   #90, COMMITTED NOT PUSHED, WIP
```

Box at last check: free ~53, `target/debug` 22.87 GiB, nothing running.
`target/release` at pin (1,427,144 KiB, sha `f748956c…`), Tom's server pid 99844
alive. Athena's sampler-13 has the floor at 25 until ~01:13Z.

## 🔴 TO FINISH #90 — this is the live lane

Committed at `c5191054` and **NOT finished**. The commit message says WIP
explicitly. What is done:

- `crates/aion/src/runtime/nif_activity_retry.rs` — new
  `exhausted_terminal_reason()`, `TERMINAL_REASON_PREFIX`,
  `EXHAUSTED_REASON_HEAD`, plus 4 new pins.
- `crates/aion/src/runtime/nif_activity_await.rs` — the exhaustion site (was
  ~line 194) now reclassifies, computing the reason ONCE for both the durable
  record and the workflow-visible value.
- `cargo test -p aion-rs --lib nif_activity_retry`: **11 passed / 0 failed.**
- `cargo clippy -p aion-rs --lib -- -D warnings`: **exit 0, 0 warnings.**

**What remains, in order:**

1. 🔴 **MUTATION PROOF — not yet run.** A green pin proves nothing until it has
   been watched failing for the right reason. Two arms:
   - make `exhausted_terminal_reason` the identity function → the core pin
     (`a_retry_exhausted_terminal_never_claims_the_failure_was_retryable`) must
     go RED and the pass-through pin must stay GREEN;
   - make it rewrite **unconditionally** (drop the `strip_prefix` guard) → the
     core pin stays GREEN and
     `a_reason_that_was_never_retryable_classed_is_left_exactly_alone` must go
     RED. **That second arm is the survival check** — it is what proves the
     non-vacuity pin is load-bearing rather than decorative.
   - restore byte-identical afterwards (`cmp` against a pre-mutation copy).
2. Check the **sibling call site** in `nif_activity_await.rs` (the
   `expired_scope_deadline` branch, ~line 113-136) is untouched and still
   records `timeout:deadline expired` — it is already honest and must stay so.
3. Cold workspace battery, then push.
4. Consider whether `handle/delivery.rs:637` (`activity_failure`) and
   `nif_collect_settlement.rs:423` (`terminal_error`) have the same shape —
   **NOT swept this pass, and a fix's search space is part of the fix.**

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
gap is `aion-server` re-invoking its own test binary 3× under a filter (the #74
child-process pin). **Explained — do not re-investigate.**

🔴 **EXPECT ONE RED: #85.** `outbox_e2e.rs`, `SQLite failure: bad parameter or
other API misuse`. It fired in ws5 at a measured ~37.5% per-run rate. **Do not
settle it statistically** — three greens at p=0.375 is a 24% draw, so absence is
not evidence of absence. Settle attribution *structurally*, as I did:
```
strings <subject-binary> | grep -c <symbol>   -> 0
strings <control-binary> | grep -c <symbol>   -> 1   <- the control is the point
```
Athena's refinement: **a positive control only validates the instrument when it
is run on the SAME KIND OF ARTIFACT as the subject** — the `.rcgu.o` objects
return 0 for the control too, so a lazier glob would have failed silently in the
confirming direction.

## ✅ Closed today

- **#76** `f7af328b` · **#86** `11e541ae`+`0e297071` · **#125** `f5ebe547` ·
  **#126** `226dadc1`. Records: `COSTLY-PROOF-GATES-86.md`,
  `GLEAM-FAILURE-HONESTY-125.md`, `GLEAM-SUPPORT-CENSUS-126.md`.
- **#85** updated with a **sixth** observed test — its own "observation, not a
  census" caveat vindicated twice.

## Other seats

- **Waffles** — ownership audit DELIVERED: `docs/design/OWNERSHIP-AUDIT-AION.md`
  (pushed) + DM. Headline: *an IDL, a status code and a serialization format
  are all treaties between strangers; we are not strangers to ourselves.*
  Proposes reopening the `aion-proto` / `aion-proto-generated` split.
- **Cally** — "SIGNAL-FIELDS" **retired as unresolved** (a fossil phrase in both
  our files, no recoverable referent). I supplied instead the one verifiable
  signal decision: *a message sent to a live target that nothing waits for is
  SILENT — should it be an error?* Four-part, recommendation **yes**. It is
  **item 17** in the Waffles/Cally decision pass.
- 🔴 **AUTHORITY CHANGED 2026-08-01**: Tom pushed the register back — **Waffles
  and Cally now JOINTLY hold decision authority.** Only his-hands items stay
  his. Decisions no longer queue to Tom.
- Record correction: the 20:01Z clear was a **planned capacity pre-clear at free
  48.86**, not a band breach.

## 🔬 Method laws earned today

Beyond the memory catalogue (`assert-the-invariant-not-the-enumeration.md`):

**🔴 A BASELINE IS ONLY A CONTROL IF IT IS INDEPENDENT OF WHAT IT MEASURES.**
Twice in one day — #86's gate and #126's divergence pin.

**🔴 A DIAGNOSTIC'S SAFE DEFAULT IS A PROPERTY OF WHO RECEIVES IT, NOT OF THE
FAILURE.** (Athena.) Share the classification; never share the default.

**🔴 AN ISOLATED RE-RUN CAN PASS *BECAUSE* IT IS ISOLATED.** The confirming run
must be at least as wide as the failing one.

**🔴 A KILL IS NOT A DISK REMEDY, IT IS A DISK COST.** `TempDir::drop` does not
run on SIGTERM — the remedy and the harm are the same quantity, opposite signs.

**A SAMPLE OF A BURSTY POPULATION MEASURES THE SAMPLER'S TIMING, NOT THE
POPULATION.** `du` integrates and was honest; instant `rustc` counts were not.

**🔴 AN INSTRUMENT THAT INTEGRATES IS ONLY HONEST IF ITS SUBJECT IS SOLE
OCCUPANT.** (Athena.) `du` on a *shared* target dir and free-space over an
interval on a shared box both integrate everybody. Only the delta measured while
one agent was the sole builder named exactly one thing.

**A PROVENANCE LABEL IS NOT A CORRECTNESS LABEL — and it is more dangerous than
no label, because it buys assent.** "Measured, not modelled" made a wrong
attribution sound *more* trustworthy than the stale model it replaced.

**A POSITIVE CONTROL ONLY VALIDATES THE INSTRUMENT ON THE SAME KIND OF
ARTIFACT.** (Athena.)

**AN ABSENCE CHECK HAS A SHELF LIFE**, and it can expire in seconds.

**A FLOOR IS A TROUGH TEST, NOT AN ENDPOINT TEST.**

**REDUNDANCY ONLY PROTECTS AGAINST INDEPENDENT FAILURES.** Two watches killed by
one sweep are one watch.

**COMPUTE BEFORE YOU COUNT.** Three greens looks like evidence and is a 24%
draw.

## Board

| item | note |
|---|---|
| **#90** | `c5191054` WIP — mutation proof + battery + push |
| **#7 / #77 / #82** | Tom's standing lanes: AWL properly, tharsis, NIF-state P0 |
| **#85** | sixth test observed; lead = SQLITE_MISUSE + mismatched `sqlite3_errmsg` in the vendored libsql local backend. Write-path connection-sharing avenue is CLOSED |
| **#124 / #88 / #71** | see D; #71 is now a Waffles/Cally decision |
| **#108** | Tom's live server, deleted binary — read-only probes ONLY |

### Follow-on not built

**Converge `aion-awl/tests/test_support/gleam.rs`** — the ancestor, now *named*
in `DIVERGENT_COPIES` rather than invisible, but still divergent. Not a `cp`:
different `compile_project` signature, **13 test files** call it.

## Box rules

- ⛔ **Never bare `cargo clean`** — it takes `target/release`, Tom's #108 restart
  path. Reclaim is scoped `rm -rf target/debug` ONLY, and **pair the absence
  check with a survival check under the SAME prefix**.
- **BEFORE a build: free < 35 → reclaim FIRST.** DURING: a DECISION, never a
  reclaim.
- ≥5 agent seats build on this box. **Free space is never "yours."**
- 🔴 **Declare any detached run to Athena BEFORE the process exists**
  (`dm:45cf420e-621d-467b-b96c-5bffb62c04f3`). ⚠️ Waffles runs the identical
  battery command in `apps/manifold/.worktrees/tree-registry` — **cwd is the
  only discriminator.**
