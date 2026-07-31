# ✅ BLOCK G — #122 connection pool — LANDED `7ead877a`

## 📍 STATUS

**LANDED** as `7ead877a` (local, unpushed). 14 files, +694/-198, staged as the
crate alone — `examples/remote-gates/worker/Cargo.lock` and the two `.aion-*`
snapshot dirs were deliberately left out.

All three pins verified identical immediately before the commit:

```
git diff --stat block-g-stash -- crates/aion-store-libsql   → EMPTY (unmoved)
git hash-object …/src/pool.rs   → 34cd902a4b7fd206163101510e0901c16159601c ✓
git hash-object …/src/store.rs  → dbe1b817ae8814198b97ee9f748c5d51e243097c ✓
```

Tree was byte-identical to the gated state, so the gates were NOT re-run.

### 🔴 CORRECTION TO THIS MARKER'S OWN DENOMINATOR

The compaction marker said to report **`1 red in 8`**, reasoning that only gate
attempts 3 and 4 sat on this tree. **That arithmetic was wrong.** File mtimes:

```
gate-aion-rs.log    2026-08-01T02:19:43+1000
gate-aion-rs-2.log  2026-08-01T02:25:57+1000
gate-aion-rs-3.log  2026-08-01T02:32:01+1000   ← the red
gate-aion-rs-4.log  2026-08-01T02:37:00+1000
```

Six minutes apart — exactly one run each, back to back, **no gap in which an
edit could have happened**. All FOUR gate runs are on one tree.

Final, as committed: **3 red in 10** (4 gate runs, 1 red; 6 series runs, 2 red).

⇒ **A DENOMINATOR WRITTEN FROM MEMORY IS A MEASUREMENT LIKE ANY OTHER.** I very
nearly amended a correct commit to match an incorrect note, because the note was
in my own hand and sounded careful. The mtimes settled in one command what the
recollection could not.

### The series result

```
mine-1  ok=58 FAILED=0 total=58  COMPLETE
mine-2  ok=58 FAILED=0 total=58  COMPLETE
mine-3  ok=57 FAILED=1 total=58  COMPLETE  ← outbox_duplicate_completion… (#85 flake 1)
mine-4  ok=58 FAILED=0 total=58  COMPLETE
mine-5  ok=57 FAILED=1 total=58  COMPLETE  ← outbox_happy_path…           (#85 flake 3)
mine-6  ok=58 FAILED=0 total=58  COMPLETE
```

🔴 The series wrapper reported **exit code 0** for a run of six containing two
reds — the trailing-statement door again. The artifacts' own verdict lines are
what caught it.

Logs preserved in the session scratchpad `gate-evidence/` (all 12).
🔴 The copy of `mine-3.log` taken at compaction was a 9-verdict PARTIAL; it has
been overwritten with the complete 58-verdict artifact.

**Next, in order:** Waffles' graduated pin test for the `outbox_e2e.rs:534-547`
interleave → **Block H** (`BLOCK-H-READGUARD-COMPILE-GUARD.md`).
#85 stays OPEN with its hypothesis refuted — see below.

## ✅ RESTORED — the stash is popped and dropped, the work is in the tree

The baseline A/B is finished. Verified after popping: `pool.rs` `928f8520…`,
`connection.rs` `9a9e662f…`, `schedule_timer` on the **writer** (store.rs:507),
`transaction_lock` gone (0 hits). The section below is kept for its findings,
**not** as a live recipe — `stash@{0}` `91c637b8…` no longer exists.

### 🔴 AND THE PINNED-BY-HASH RECIPE I COMMITTED DID NOT WORK EITHER

```
git stash pop 91c637b86a234a1631ee1bc34c7755283bc8699e
  error: '91c637b8…' is not a stash reference
```

`git stash pop` takes a **stash ref**, not a commit. So the fix for "a label is
not an identity" was a command I had never run — **the exact law committed
twenty minutes earlier, broken while writing its own remedy.** Fourth time in
this block that I have filed a rule and then supplied the counterexample.

The form that actually works resolves hash → ref, then pops the ref:

```
PIN=<stash commit sha>
REF=$(git stash list --format='%gd %H' | awk -v p="$PIN" '$2==p {print $1}')
[ -n "$REF" ] || { echo "ABORT: pinned stash not present"; exit 1; }
git stash pop "$REF"
```

⇒ **A CORRECTION IS AN UNTESTED CHANGE LIKE ANY OTHER.** Being the fix for a real
defect grants it no presumption of working. `--check`-equivalents are owed to
remedies most of all, because a broken remedy is reached for at exactly the
moment nothing else is left.

## 🚨 HISTORICAL — the stash-era recovery notes (recipe no longer live)

A baseline A/B was run by stashing the implementation to reach a clean HEAD.
Recovery, in order of preference:

🔴 **`stash@{0}` IS A POSITION, NOT AN IDENTITY.** There is already an unrelated
`stash@{1}`, and **any** `git stash push` — mine, a tool's, a later session's —
renumbers silently, leaving this recipe still *reading* correct while pointing at
something else. The stash's identity is its commit:

```
91c637b86a234a1631ee1bc34c7755283bc8699e
  "On main: block-G #122 pool implementation (baseline A/B in progress)"
  ^3 tree holds crates/aion-store-libsql/src/pool.rs at blob 34cd902a…
```

⇒ **IDENTITY OF AN ARTIFACT IS ITS HASH, NEVER ITS LABEL** — and `stash@{N}` is a
label wearing the costume of an index.

```
git stash list --format='%H %gd %s'   # find 91c637b8…, do NOT assume it is @{0}
git stash pop 91c637b86a234a1631ee1bc34c7755283bc8699e   # normal restore — PREFERRED

# fallback if the stash is gone. 🔴 THE --include IS LOAD-BEARING, NOT TIDINESS:
git apply --check --include='crates/aion-store-libsql/*' <scratchpad>/122-tracked.patch
git apply         --include='crates/aion-store-libsql/*' <scratchpad>/122-tracked.patch
cp <scratchpad>/pool.rs.bak crates/aion-store-libsql/src/pool.rs
```

Backups taken BEFORE stashing:

```
<scratchpad>/122-tracked.patch  sha256 547424915c470966dad5e67cb1fe32ff083613bd7a9214815e2396890404b60a
<scratchpad>/pool.rs.bak        sha256 928f85202d3d160e7be8df15bb483fc2a1aa0c7c86cf01a712c7726c5251ef26
                                git hash-object → 34cd902a4b7fd206163101510e0901c16159601c
                                = stash@{0}^3:…/pool.rs, byte-identical
```

`pool.rs` is UNTRACKED: it needs `-u` on the stash and is **not** in the patch.
⚠️ `git stash show --stat` does **not** list `-u` files, so pool.rs is invisible
there and present in the stash. Checking the obvious way concludes it was lost.

### 🔴 Why the naive `git apply` DOES NOT WORK — found by running it, not reading it

```
git apply <scratchpad>/122-tracked.patch
  error: patch failed: docs/design/aion-authoring/BLOCK-G-122-POOL-IN-FLIGHT.md:27
  error: patch failed: examples/remote-gates/worker/Cargo.lock:83
```

The patch was taken across the **whole tree** (15 files); the stash was
pathspec-limited to the **crate** (13 files). The two extras have both moved
since, and **`git apply` is atomic** — those two rejections discard all thirteen
crate files that were perfectly fine.

- ⇒ **A SUPERSET BACKUP IS MORE FRAGILE THAN AN EXACT ONE.** The extra content is
  not free insurance; it is an extra failure mode that takes the good hunks with it.
- ⇒ **TWO COPIES WITH DIFFERENT SCOPES ARE NOT TWO COPIES OF THE SAME THING.**
  The scope law again — this time hiding in the *recovery* path, where it would
  surface only while being needed.
- ⇒ **A BACKUP IS NOT A BACKUP UNTIL ITS RESTORE PATH HAS BEEN RUN.** `--check`
  costs nothing and is the difference between a net and a picture of a net. Same
  species as the inverted control: a fallback never watched succeeding is
  untested.

The `--include` form was verified: applies clean at **13 files, 366 insertions,
198 deletions**, which is the stash's diffstat to the line — so the flag both
repairs the fallback and independently proves patch ≡ stash across all thirteen.


**Written at compaction, 2026-08-01. The working tree carries an uncommitted
implementation. Nothing here is on `origin/main` except the docs.**

## State right now

- **Block G is OPEN with Athena.** GO given, naming exactly four invocations:
  `rm -rf target/debug` (done), `cargo clippy -p aion-store-libsql --all-targets`,
  `cargo test -p aion-store-libsql`, `cargo test -p aion-rs --all-targets --no-fail-fast`.
  **Anything beyond those four re-declares.**
- Sampler-6 armed, 100 min, expires **~17:42Z**, band 50. If running at 17:34 she re-arms.
- Disk: reclaimed `target/debug` only (**not** `cargo clean` — that would take
  `target/release`, Tom's #108 server build). free 71 → 89 GiB, release intact at 1.4G.

## 🔴 PINS TAKEN BY ATHENA AT 16:13:59Z — hand BOTH back after the revert

```
crates/aion-store-libsql/src/connection.rs
  sha256 9a9e662f348ec04dbce5135a4fb2eb2d87825869a56071d619bb31a2e94d3363
  14884 bytes

crate-tree digest 2e2f4a19b82364b25d63488f89899d8abb32459d72924cb8338992a6a99b83af
  = sha256 of the sorted per-file sha256s across crates/aion-store-libsql/src/**.rs
```

The second exists because a single-file pin is blind to a stray edit elsewhere by
construction. Both go back to her.

## ✅ THE INVERTED CONTROL RAN — AND IT CAUGHT A VACUOUS TEST

**Sequence, 2026-08-01:**

1. `connection.rs` verified against Athena's pin **before** touching it: hash and
   byte count both exact.
2. `apply_pragmas` call deleted from `ConnectionFactory::connect`.
3. `a_pooled_connection_carries_the_configured_pragmas_not_only_the_first`
   → **PASSED**. 🔴 The control did not fire.

**Why it was vacuous.** The test asserted a hardcoded `synchronous == 2`,
reasoning that WAL's default is `1` so `2` could only come from the PRAGMA. That
reasoning was wrong: `journal_mode = wal` is applied **after** the connection
opens, so the WAL-specific default never applies and the connection already
reports `2`. **The expected value equalled the default.** The assertion could not
fail, and nothing about the pool's correctness was ever being tested.

**The fix is the property, not a better number.** The test now measures a
baseline from an *unconfigured* store's pooled connection, asserts the value
under test differs from it (**non-vacuity guard**), then asserts the configured
store's pooled connection reports it. `off` = `0` is the discriminator purely
because no `SQLite` default is `0` — a fixture choice, not a durability opinion.
A future change to libSQL's defaults cannot make this vacuous again; it would
trip the guard loudly instead.

4. Rewritten test against the still-broken factory → **RED**, with its own
   message: `left: 2, right: 0`, naming the exact defect.
5. `connection.rs` restored → hash back to `9a9e662f…`, 14884 bytes, **exact**.
6. Gates on the corrected tree: clippy `-D warnings` **0**, `cargo test -p
   aion-store-libsql` **0**, 90 tests.

**Scope note, declared not buried:** the granted mutation class was *one file,
one function, restore, nothing else moves*. `pool.rs` moved too — because the
control found a defect in the test rather than in the code. `connection.rs` is
byte-identical to its pin; the crate-tree digest differs **solely** because of
that test rewrite.

### Law earned

**A control that passes is not a control.** The whole value was in watching it
fail; had I skipped the step as ceremony, a test asserting a tautology would have
shipped as the proof that the pool preserves operator durability settings. Second
time in two blocks (#119, #120) that the *obvious* control was itself wrong —
which is the argument for running them, not for trusting them.

### Second finding, unrelated and free

Every `echo "EXIT=${PIPESTATUS[0]}"` I ran this session **printed empty**. This
shell is **zsh**, where the array is `pipestatus` and is **1-indexed**;
`PIPESTATUS` is a bash-ism and is unset here. Every one of those exit lines was
vacuous, and I read past them. Use `set -o pipefail` with `$?`, or
`${pipestatus[1]}`. Cousin of the diff-driver grep trap: **a reporting construct
borrowed from another shell fails silent, not loud.**

## 🔴 GATE RED, UNATTRIBUTED — and two vacuous exit codes found under it

`cargo test -p aion-rs --all-targets --no-fail-fast`, attempt 3, **EXIT=101**.
Two failures in `outbox_e2e`, both:

```
Error: Store(Backend("SQLite failure: `bad parameter or other API misuse`"))
  outbox_crash_recovery_rearms_stranded_done_row_and_finishes
  outbox_failure_fails_fast_and_cancels_unresolved_siblings
```

### 🔴 The notification's exit code was NOT cargo's

The task notification said **"completed (exit code 0)"** for a run whose real code
was **101**. The command was `cargo test … > log 2>&1; echo "EXIT=$?" >> log` —
the harness reports the *last* command's status, which is the `echo`. **A trailing
`echo` launders any failure into a success.** I would have accepted that green if
the log had not disagreed with it.

⇒ **THE EXIT CODE OF A COMPOUND COMMAND BELONGS TO ITS LAST STATEMENT, NOT ITS
SUBJECT.** Same family as the zsh `PIPESTATUS` finding an hour earlier: both are
reporting constructs that fail silent, and both were caught only by reading the
thing they claimed to summarise. Gates 1 and 2 were re-checked from their logs —
`EXIT=0` with zero `test result: FAILED` lines, genuinely green.

### What this red is, and what it is NOT

`SQLITE_MISUSE` / "bad parameter or other API misuse" **is flake (2) in the #85
record** — documented in `LIBSQL-SHARED-CONNECTION-FINDING.md:259` *before* this
change, and explicitly **not** traced to an interleaving there.

🔴 **A name matching a known flake is suggestive, not exculpatory.** This is the
symmetric form of constraint (a): if flake disappearance may not be credited to
the fix, flake familiarity may not excuse a red on the fixed tree. Both directions
of that argument are cheap, and only measurement is not.

Measured, not assumed:

| attempt | tree | result |
|---|---|---|
| 1 | pool, pre-inverted-control | EXIT=0 |
| 2 | pool, post-restore, **pre** timer fix | EXIT=0 |
| 3 | pool + timer fix | **EXIT=101**, 2 failures |
| — | isolation, single test, ×1 | pass |
| — | whole `outbox_e2e` binary, ×3 | 8/8 pass each |
| — | 8 concurrent instances × 3 rounds | **0 failures / 24 runs** |

So it did not reproduce in 27 runs of the failing binary, including 24 under
deliberate load. It has appeared exactly once, under whole-workspace load, which
is a heavier and different regime than any of the above.

**Current honest position: the red is UNATTRIBUTED.** One event cannot support a
rate — the same statistical point already banked for #85. What can be said:
the signature pre-dates the change, and the change was never justified by these
flakes in the first place.

### 🔴 CORRECTION: the failing tests are NOT #85's recorded flakes

#85 names three: `outbox_duplicate_completion_records_exactly_one_terminal`,
the aion-server unit test `paused_run_row_is_held_pending_then_dispatches_after_release`
(which is where the MISUSE *text* comes from), and
`outbox_happy_path_completes_with_n_terminals_in_order`.

**Neither failure is on that list.** I matched on the error string and called it
the known flake — the exact move I had warned against one paragraph earlier.

⇒ **A SIGNATURE MATCH IS NOT AN INSTANCE MATCH.**

Counterweight, stated rather than left for someone else: #85 records what was
**observed**, not a census. Both failures live in the same file as (1) and (3)
and share their `wait_for_history` structure. This raises the concern; it does
not settle it.

### 🔴 CORRECTION: the 90-green battery barely tests contention at all

Counted, not assumed — `crates/aion/tests/outbox_e2e.rs`:

| battery | shape |
|---|---|
| `aion-store-libsql` (the 90 green) | ~78 bare `#[tokio::test]` (**current-thread**), 9 sync `#[test]`, **2** multi_thread |
| `outbox_e2e` (the red) | **8 of 8** `#[tokio::test(flavor = "multi_thread", worker_threads = 4)]` — including both failures |

So the green battery is ~98% contention-free: it is **silence about exactly the
property the red is a candidate instance of.** Treating 2 greens against 1 red as
2–1 was wrong; the greens were mostly not asking the question.

⇒ **A GREEN'S WEIGHT IS THE FRACTION OF IT THAT COULD HAVE GONE RED.**

Also excluded, by reading `outbox_e2e.rs:124`: temp paths carry pid + nanos + a
unique counter, so cross-process db collision cannot explain any of it. Whatever
this is, it is **intra-process** — squarely this change's subject matter, not
outside it.

## 🔴 P0 FOUND IN MY OWN UNCOMMITTED FIX — a WRITE routed onto a READ connection

Found by auditing the transform before committing, not by any test.

```
store.rs:504   crate::timer::schedule_timer(&*self.reader().await?, …)
timer.rs:33    conn.execute(UPSERT_TIMER_SQL, …)
```

**Durable timer scheduling was executing on a pooled READ connection** — every
workflow sleep, every activity timeout — outside the writer mutex, concurrent
with the writer connection's `BEGIN IMMEDIATE`. That is the #122 mechanism
exactly, rebuilt inside the commit whose purpose is to remove it.

**Cause: I classified by NAME.** `schedule_timer` reads like scheduling, not
writing. The same depth-1 classifier already missed `put_package` and three
namespace methods earlier in this block; I corrected its *outputs* twice and
never corrected the *method*.

⇒ **A NAME IS NOT A SIGNATURE. Classify by what the callee EXECUTES.**

**The audit, done properly this time.** Every `.execute(`, `BEGIN`, INSERT,
UPDATE and DELETE in the crate extracted and mapped to its enclosing function by
line range; all 24 `reader()` sites checked against their callee. `schedule_timer`
is the **only** defect — the other 23 callees are verified read-only individually,
not assumed.

### 🔴 A habit caught it — nothing MECHANICAL could have

Amended wording. "Nothing could have caught this" is false and is the kind of
sentence that excuses the next one: I caught it by asking the blast-radius
question during a gate wait. That is a **habit**, and habits die with whoever
holds them — which is precisely the argument for the compiler cure, not against
it. The accurate finding is narrower and worse:

A write on a read connection **compiles cleanly and passes every test** — 90
green, including the new concurrency test, with the defect live. Single-threaded
tests never contend, so this class of bug is invisible to precisely the battery
meant to guard it. The greens were real and said nothing about it.

**Structural answer, declared before starting:** `ReadGuard` stops `Deref`-ing to
`libsql::Connection` and exposes a read-only surface, so handing a read
connection to anything that executes **fails to compile**. The compiler becomes
the classifier and my judgement stops being load-bearing. ~20 signature changes
across the read helpers. **Next step in this block, after the commit** — not
deferred, sequenced, and declared to Athena in advance rather than slid into.

## The one step left mid-air: the INVERTED CONTROL (amendment granted) — DONE, see above

**Mutation class declared and approved: one file, one function, reverted.**

1. Break `ConnectionFactory::connect` in `connection.rs` — drop the PRAGMA
   application.
2. `cargo test -p aion-store-libsql pool::tests::a_pooled_connection_carries` →
   must go **RED with its own message**.
3. Restore.
4. Recompute both hashes and hand them to Athena.

Why it matters: the test asserts `synchronous = full` (SQLite **2**, vs the WAL
default **1**) on a **pooled** connection, never the first. That is precisely the
property the pool's existence could silently break. A control never observed
failing is indistinguishable from one that cannot fail.

## What is built (uncommitted, gates green)

```
cargo clippy -p aion-store-libsql --all-targets -- -D warnings   exit 0
cargo test   -p aion-store-libsql                                exit 0, 96 tests
```

- **`connection.rs`** — `ConnectionFactory` is now the ONE place a configured
  connection is produced; `open_connection` is written in terms of it.
  `BUSY_TIMEOUT` renamed **`INHERITED_BUSY_TIMEOUT`** with docs saying the 10 s is
  inherited and unchosen; `LibSqlConfig::busy_timeout_ms` overrides it.
- **`pool.rs` (NEW)** — `ReadPool` + `ReadGuard`. Retention cap, **never**
  concurrency: an empty pool **opens rather than waits**. Poisoning is recovered
  totally via `PoisonError::into_inner` (a `Vec` of handles has no invariant a
  panic can break), so there is **no silent-drop branch**.
- **`store.rs`** — `LibSqlStore { factory, writer: Arc<Mutex<Connection>>, readers }`.
  `transaction_lock` **deleted**. **`connection()` DELETED** — that deletion broke
  43 call sites across four files, which is the accessor proving it was reachable
  from everywhere.
- **`config.rs`** — `busy_timeout_ms: Option<u64>`, `read_pool_size: Option<usize>`,
  both in the crate's existing unset-means-unset idiom. **No values chosen.**
- Call sites: writes → `&*self.writer().await`, reads → `&*self.reader().await?`.
- Test writes rerouted from `reader()` to `writer()` (5 sites) after the bulk
  transform proved too coarse.
- `connection_accessor_reuses_same_database_handle` **renamed** to
  `a_write_is_visible_from_a_different_pooled_connection` — the old name claimed a
  surface the run could no longer touch (#119's defect one level down).

New tests, all passing: `a_pooled_connection_carries_the_configured_pragmas_not_only_the_first`,
`the_pool_retains_at_most_the_configured_count`, `an_absent_read_pool_size_retains_nothing`,
`many_tasks_against_one_store_interleave_writes_and_reads_safely`.

## Still to do after the revert

1. **The dependents gate** — `cargo test -p aion-rs --all-targets --no-fail-fast`,
   launched as background task **`bxctvwjg2`**, log `/tmp/gate-aion-rs.log`
   (writes `EXIT=<code>` as its last line). Already inside the GO.
2. **Commit.** Nothing of the implementation is committed yet.
3. 🔴 **The GRADUATED PIN TEST — Waffles' constraint, not optional.** The
   `crates/aion/tests/outbox_e2e.rs:534-547` interleave (a guarded `append`
   concurrent with an unguarded `complete_outbox_row` on one store) becomes a
   **deliberate** test. It must NOT be allowed to vanish as "the flake went away".
4. `aion-server` may need the two new `LibSqlConfig` fields at its construction
   sites — not yet checked; the dependents gate will say.

## 🔴 CONSTRAINT (a), which binds the commit message

**The pool is justified by the PROVEN MECHANISM, never by #85's flakes.** If the
flakes disappear, that is recorded **consistent, not load-bearing**. No commit
message, doc, or task update may cite flake disappearance as evidence the fix
worked.

## The evidence, indexed by the verdict each artifact EMITTED

Not a tally of verdicts — the verdict lines themselves. Logs preserved outside
`/tmp` before write-up.

```
── gate-aion-rs-3.log   EXIT=101
     test result: FAILED. 6 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
     ↳ test outbox_crash_recovery_rearms_stranded_done_row_and_finishes ... FAILED
     ↳ test outbox_failure_fails_fast_and_cancels_unresolved_siblings ... FAILED
── gate-aion-rs.log · gate-aion-rs-2.log · gate-aion-rs-4.log   EXIT=0, no FAILED verdict
── base-1.log · base-2.log (clean HEAD)                         EXIT=0, no FAILED verdict
```

### 🔴 `FAILED = 0` CANNOT SEE A RUN THAT NEVER FINISHED

Every complete `cargo test -p aion-rs --all-targets` run emits exactly **58**
`test result:` lines. The red is 57 ok + 1 FAILED = 58.

A run that dies early — truncation, OOM kill, linker failure, `^C`, the box going
down mid-series — emits **fewer verdicts and zero FAILED**, which under a
`FAILED = 0` index is **indistinguishable from green**. `EXIT=0` does not rescue
it either; that is the trailing-`echo` door.

⇒ **AN ORACLE THAT ONLY DETECTS FAILURE CANNOT DETECT ABSENCE.** A positive
verdict is required, not merely the lack of a negative one.

**Index every run by three numbers: EXIT, the quoted verdict, and `ok + FAILED`,
which must equal 58.** The total is the one that proves the run *happened*.

```
base-1 · base-2 · gate-aion-rs · gate-aion-rs-2 · gate-aion-rs-4 · mine-1
                        EXIT=0    ok=58  FAILED=0  total=58  COMPLETE
gate-aion-rs-3          EXIT=101  ok=57  FAILED=1  total=58  COMPLETE  ← the red
```

This was live risk, not theory: six unattended gates were running back to back
with nothing between a killed run and a recorded green but a count nobody was
taking.

### 🔴 The first version of this table under-reported by half

It had a `FAILED` column holding `1` — a true count of **binaries reporting
failure** (both failures are in one binary) under a header every reader parses as
**failing tests**. There were two. It failed toward *less severe*, the direction
nobody audits.

The table did not drift from the logs; it drifted from **the question a reader
brings to it**. And it was built specifically as an anti-drift device — the third
time in this block a remedy carried the defect it was written against.

⇒ **A COUNT OF ORACLES IS NOT AN ORACLE.** Don't tally verdicts, **quote** them:
`6 passed; 2 failed` carries both numbers, names both tests, and cannot be
misread as either.

⇒ **(VALUE, NOUN) TRAVEL TOGETHER OR THE VALUE IS FREE TO MEAN ANYTHING.**

## 🔴 SEVEN REPORTING DEFECTS, ONE TRAP: a SUMMARY diverging from its SUBJECT

Every reporting failure in this block is the same bug wearing a different door:

| the summary | what it was actually reporting |
|---|---|
| trailing `echo "EXIT=$?"` | the `echo`, not `cargo` |
| `rc=$?` after a pipe (**twice**) | `head`, not the command |
| `${PIPESTATUS[0]}` in zsh | nothing — it is a bash-ism, silently unset |
| `git stash show --stat` | tracked files only; `-u` files invisible |
| task notification "exit code 0" | the wrapper's last statement, for a run that exited 101 |
| `stat` format string with a literal `Z` | a hand-typed label where a unit goes |

Not seven traps. **One trap with seven doors** — and every escape came from the
same move: reading the artifact and finding it disagreed with the summary.

⇒ ★ **PREFER AN ORACLE THE SUBJECT EMITTED OVER A STATUS THE WRAPPER REPORTED.**
`cargo` writes `test result: FAILED` into its own output — that is the subject
speaking. `$?` after a pipe, a notification, a `--stat`, a format string are all
something else speaking *about* it. When a summary must be used, the obligation
is to know **what that particular summary omits** — which is exactly how
`--stat` nearly cost the only untracked file in the backup.

⇒ **Typeable form: grep the artifact for its own verdict.**

⇒ And the meta-finding, because prose has demonstrably not changed behaviour —
each of these was written into this file and then re-committed within the hour:
**THESE ARE NOT LAWS TO REMEMBER, THEY ARE FLAGS TO TYPE.** `--check` before
trusting a patch. `set -o pipefail` before reading `$?`. An explicit path list
before calling two things copies. A hash before calling a label an identity.

## Laws earned in this block

- **Check the COMMAND'S blast radius against the rule, not your INTENTION against
  the rule.** I held "never touch `target/release`" as a rule I was following
  while proposing `cargo clean`, which takes it. That is how a careful person
  deletes something.
- **The rationalisation arrives before the search does.** I wrote a
  `std::process::abort()` into a library `Deref` and justified it as "honest about
  the impossibility". The clean shape took thirty seconds once I looked. If you can
  say why the ugly thing is acceptable before spending two minutes looking for a
  clean one, the articulation is the symptom.
- **Exit 0 from a text transform means "I ran", never "I changed anything."** BSD
  `sed` has no `\b`; my substitution silently did nothing and was caught only by
  grepping after.
- **A published predicate transfers its unstated SCOPE along with its logic.**
  Athena reproduced my count exactly because she inherited the same one-file scope.

---

## 🔴 THE SERIES CAUGHT A SECOND RED — and it refutes #85's stated hypothesis

`/tmp/mine-3.log`, on this tree, `ok=57 FAILED=1 total=58` (run completed):

```
test result: FAILED. 7 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.24s
failures:
    outbox_duplicate_completion_records_exactly_one_terminal
```

Same error as the gate red:
`Store(Backend("SQLite failure: `bad parameter or other API misuse`"))` — this
time surfaced *inside the workflow*, at `collect_all:durability:store error`,
i.e. the fan-in settling four activities at once.

**This one IS on #85's recorded list** — it is flake (1), by name. So the
observed set now spans #85's list AND two tests outside it, all in `outbox_e2e`,
all one error. The counterweight written above — *"#85 records what was observed,
not a census"* — is now carried by evidence rather than offered as a caveat.

### The audit that changes the story

Counting the pre-change code at HEAD rather than restating the design note:

| claim | drafted | **counted** |
|---|---|---|
| `self.connection()` sites in `store.rs` | "36 methods" | **48 sites** |
| guarded entry points | — | **13** `let _guard` |
| unguarded `BEGIN IMMEDIATE` entry points | "six" | **five** |

The five: `put_package_with_routes`, `register_namespace`, `put_namespace`,
`set_namespace_placement`, `deprecate_namespace`.

🔴 **All five are deploy/namespace admin. NONE is on the outbox path.** Every
outbox WRITE was already serialised by `transaction_lock` at HEAD.

⇒ **The unguarded-BEGIN defect cannot have been producing the outbox flakes**,
and fixing it was never predicted to stop them. #85's stated cause — *"suspected
shared-connection defect in the durable write path"* — is **refuted** for the
write path specifically.

### What DID touch the outbox path — and still did not explain it

Eight outbox READS ran unguarded on the very handle the guarded outbox writes
were using: `outbox_row_state`, `list_stale_claimed_outbox_rows`,
`list_unsettled_outbox_workflow_ids`, `list_dead_lettered_outbox_rows`,
`count_inflight_outbox_rows`, `count_claimed_outbox_rows`,
`count_claimed_outbox_rows_by_namespace`, `pending_outbox_routes`.

That is a real shared-handle hazard on the outbox path, and Block G removes it
(reads now take private connections). It was the one candidate that could have
explained the flake. **It did not** — the flake survives at a comparable rate:
**2 red in 9** on the fixed tree.

⇒ #85 stays OPEN, with its hypothesis marked refuted rather than quietly closed
by association with this commit. The cause is still unidentified.

### Why this matters more than the fix

Landing G with a green series would have produced the sentence *"the pool fixed
the flakes"* with nothing but coincidence behind it. Waffles' constraint (a) —
*the pool is justified by the proven mechanism, never by the flakes* — was
written to stop exactly that. It earned its keep in the opposite direction from
the one expected: the flakes **persisted**, and the constraint is what made that
a finding instead of an embarrassment.

⇒ **A CONSTRAINT THAT ONLY PAYS OFF WHEN THE NEWS IS GOOD IS NOT A CONSTRAINT.**

### Law earned

The audit that refuted the hypothesis was arithmetic on the OLD code — countable
at any point in the last three days, needing no run at all. I reached for it only
after a second red made the statistical story uncomfortable.

⇒ **WHEN A MEASUREMENT DISAPPOINTS, COUNT THE MECHANISM BEFORE ENLARGING THE
SAMPLE.** Another six runs would have refined a number that was answering the
wrong question.

---

## 🔴 A THIRD RED — and a SECOND error string that names the mechanism

`/tmp/mine-5.log`, `ok=57 FAILED=1 total=58` (run completed):

```
test result: FAILED. 7 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.81s
failures:
    outbox_happy_path_completes_with_n_terminals_in_order
---- outbox_happy_path_completes_with_n_terminals_in_order stdout ----
Error: Store(Backend("SQLite failure: `not an error`"))
```

That test is **#85's recorded flake (3)**, by name. Four distinct tests observed
now, all in `outbox_e2e`.

### 🔴 `not an error` is SQLITE_OK's message text

`crates/aion-store-libsql/src/error.rs:7` is a pass-through —
`StoreError::Backend(error.to_string())` — so this string is libSQL's own
`Display` for `Error::SqliteFailure(code, msg)`. libSQL took a failure path and
then formatted a message that says **no error occurred**.

An error raised while the error state reads OK means the `sqlite3_errmsg` read
did not correspond to the statement that failed.

⇒ **`sqlite3_errmsg` IS CONNECTION-LEVEL STATE — EXACTLY LIKE
`sqlite3_changes64`.** This block already documented the `changes64` hazard (a
`changed > 0` check answering for somebody else's statement). This is the same
hazard on the error channel, and it is the strongest lead available.

### Strongest surviving hypothesis — UNTESTED, recorded as a lead not a finding

If errmsg can be clobbered between the failing step and the errmsg read, then
some connection is still being entered by two tasks at once. Reads now take
private connections and writes serialise on the writer's mutex, so the naive
candidates are closed. What is NOT yet ruled out:

- `libsql::Transaction`'s `Drop` path when a helper returns early on error
  without an explicit `commit`/`rollback` — does it run work on the connection
  after the writer guard has been released?
- Anything libSQL does on the connection off the caller's task.

🔴 **This is a lead, not a diagnosis.** It has not been tested, and the last
mechanism I found this plausible (`shared handle ⇒ outbox flake`) was refuted by
counting. Count before believing it.

### The honest reading of the series

Waffles' constraint (a) has now paid twice. The first payment stopped a false
"the pool fixed the flakes". The second is this: because the commit never leaned
on the flake rate, three reds arriving mid-series cost the commit **nothing**.
Nothing had to be retracted, because nothing had been claimed.

⇒ **A CLAIM YOU NEVER MADE IS A CLAIM YOU NEVER HAVE TO WITHDRAW.**
