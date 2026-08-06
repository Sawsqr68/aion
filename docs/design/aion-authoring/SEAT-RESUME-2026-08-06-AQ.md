# Seat resume — marker AQ, 6 August 2026

Supersedes AP. **Latest letter wins.** AP holds the #146 tutorial self-run, the
five-issue triage, and Waffles' four dispositions; that record stands.

`origin/main` = `d32a41977`. Tree carries the #147 red-first working set,
uncommitted at time of writing (see "In flight"). **Tom's pids 45425 (server)
and 48399 (worker) untouched.**

## 🔴 #147 — the shape of the defect CHANGED, and it is worse

Amendment 2 to `SHARED-QUEUE-SILENT-REFUSAL-2026-08-06.md` reads the liminal
half that amendment 1 left open.

**One gate, two callers, opposite observability.**
`contracts.rs:120` `validate_worker_contracts` is the single gate. The liminal
transport funnels every refusal through `admit_registration`
(`liminal_transport.rs:1592`) and names it at WARN with pid, identity, queue,
namespaces, **both** advertised sets and the reason — pinned by
`tests/dead_man_switch_refusal_logs.rs:83`. The gRPC caller
(`worker_grpc.rs:67`) names **nothing**.

**And this defect was already found, fixed and pinned once.**
`admit_registration`'s doc comment records it: *on 2026-07-29 a worker was
refused on every single dial with the server silent on all of them, and "the
refused process looked merely asleep."* That is verbatim what I measured on
2026-08-06 on the other transport. **The fix landed in one CALLER instead of in
the shared gate, so the second transport re-manifested it within a week with
nothing in the tree able to notice.**

Sharpest detail: `worker_grpc.rs:632-640` **does** log — a WARN for the case
where the check was *skipped* for want of an engine handle. The refusal branch
thirty lines below is the one branch with no voice. An instrument that reports
its excuses and swallows its verdicts.

⇒ `one-truth-in-two-places-has-drifted`, literal form. **Cure is subtraction:
the naming goes into the gate, not into a second copy of the liminal log.**

**Brief:** `BRIEF-147-ADMISSION-REFUSAL-NAMES-ITSELF.md` (`d32a41977`).
R1 gate-side naming · R1a transition rule (identical repeat = silence; a
semantic, not an interval or cap — **this closes #94 in the same motion**) ·
R1b the transition-state owner · R2 the availability hint · R3 routed to Tom.

### Waffles' dispositions on the brief (00:16Z) — ALL BINDING

1. **One motion approved.** R1 + R1a together; splitting would land a defective
   log volume on a second transport.
2. **The transition-state map gets its lifecycle review BEFORE it is written** —
   bounded by construction, entries cleared on admission or departure. *"An
   unbounded key map keyed on refused identities is a slow leak wearing a
   diagnostic's clothes."*
3. **The amended pin must still fail when the naming is absent.** Amend, never
   delete. *"A guard that can't be shown to fail without the fix asserts on the
   world it happens to be in."*
4. **R3 is Waffles' to hold, stamped** — goes to Tom after the liminal chain
   closes. **I do not send it.**

## In flight — the red-first working set (UNCOMMITTED)

- **NEW** `tests/test_support/capture.rs` — the `tracing` capture layer, was
  duplicated byte-identically in two test files whose doc comments had already
  **drifted** (they disagreed about whether the layer captures process-wide or
  thread-locally; neither described the layer — that is a property of how the
  subscriber is INSTALLED). Extracted, with the installation trap documented.
  **140 lines subtracted, 10 added.**
- **NEW** `tests/test_support/grpc_admission.rs` — shared gRPC admission harness.
  ⚠️ **`runtime_config` is duplicated across 19 test files in this crate.** Out
  of scope to fix and **named in the module rather than quietly widened**; this
  deduplicates only the two on this lane's path so the lane adds no 20th copy.
- **NEW** `tests/grpc_admission_refusal_logs.rs` — the red. Own binary (the
  tonic server runs in a `tokio::spawn`, so only a GLOBAL subscriber sees it).
  Four phases: named at all · identical repeats silent · **changed reason speaks
  again** · a different build speaks for itself.
- **EDITED** `worker_contract_registration.rs`, `dead_man_switch_refusal_logs.rs`,
  `unserved_queue_honesty.rs` — rewired onto the shared support.

**Not yet written:** the R1/R1a implementation itself, and R2's red (it depends
on R1b's audit handle, which is gated on Waffles' lifecycle review).

## 🔴 THE WATCHDOG FINDING — a second road to a class Cally found tonight

Waffles gated the cold build (`stack/aion/target` was deleted whole by Tom's
reclaim) behind declaring the draw and running under band terms. Declared to
Cally and Athena; both cleared it. Cally required an **arming-time actuator
check** because Hermes's watchdog had been falsified an hour earlier: its
`pkill -f "$LANE"` matched only its own command line, so on a trip it would kill
itself, write a clean ABORTED receipt, and let the build run on through the
floor. Their mechanism: **`cwd` is not in `argv`** — a `-f` pattern can address
a process by what it was *typed as*, never by *where it runs*.

**My control failed on its first run, and caught a different instance.** The
pgid file was never written, so my target variable was **empty**, so
`kill -TERM -- "-$PGID"` evaluated to **`kill -TERM -- -`** — which SIGTERMed my
own shell (exit 144).

★ **`kill -- -` IS NOT A NO-OP. AN UNSET TARGET IN A GROUP-KILL ACTUATOR DOES
NOT FAIL CLOSED — IT RESOLVES TO SOMETHING, AND THAT SOMETHING IS YOU.** Same
terminal state as the name-based failure (actuator dead, build alive, receipt
clean) reached with no pattern matching anywhere. **The class is wider than
`pkill -f`: it is any actuator whose target can be empty at fire time.** The
review question is not "is the pattern right" but **"what does this fire at when
the target resolves to nothing?"**

Fixes: the actuator **refuses to fire at anything that is not a positive
integer**; and the target is no longer *named* at all — I launch the build, so I
own its process group (`perl -e 'setpgrp(0,0)'` writes the pgid; the watchdog
kills `-PGID`). The addressing problem is dissolved, not worked around.

**Positive control, against a real sacrificial group, before arming:** target
group `98577` vs my shell's `98574` (**distinct** — the thing that was false for
Hermes); 3 members before including **both child sleeps** (so it reaches
rustc-equivalents); **0 after**; actuator survived.

Scratchpad: `pgleader.py` (superseded by the perl one-liner — the `python3` on
PATH cannot exec under `nohup`), `watchdog.sh`, `floor.gib`, `build.log`,
`watch.log`, `build.pgid`.

### Disk terms accepted by Cally and Athena

- Fire time **00:23:39Z, 138 GiB available, floor 40 GiB** (floor read from a
  file every sample so it can be raised mid-flight).
- Sampling 20s. `df` = the **box**, not me. `du` on `aion/target` = my draw,
  and it is **target-only** — `~/.cargo` sampled separately, because a from-zero
  `target` is not a from-zero cargo (Athena).
- **An abort stops the bleeding; it does not recover space.** In the receipt
  verbatim.
- ⚠️ **Cally's correction, accepted:** my ~25-37 GiB prior is **suspect by
  category** — an accumulated `target/` is a HISTORY, not a build cost. Doug's
  full cold iridium workspace measured **3.04 GiB** against a 45.9 GiB
  contingency. **My `du` lands with no prior attached.**
- ⛔ `.aion-accepted-binary-20260731/aion` is now the **SOLE** copy of Tom's
  accepted build (`target/release` went in the same reclaim). Never touch it,
  never a bare `cargo clean` near stack/aion.

## Standing constraints (unchanged)

🔴 Worker rebuilds frozen until the #139 cut — **not touched**.
🔴 Tom's `~/.aion/config.toml` `[ops_console] source = { FileSystem = … }` stays
until he installs a binary from `b20d2da1`+. 🔴 beamr JIT blocker with Artemis.
⛔ Never `git add -A` here — name paths. ⛔ NO WORKTREES; one mutating lane.
⛔ Never publish artifacts for Tom. Backticked commit messages via
`git commit -F -`. Do not edit `CLAUDE.md`. Never mention prospekt.
NO-SWEEP untracked: `.aion-accepted-binary-20260731/`,
`.aion-jjuabk-snapshot-20260731/`, `.aion-data-preswap-20260804/`.
⚠️ A store cannot live under `/tmp` — path-safety gate refuses a world-writable
ancestor.

## Board

- ✅ #146 self-run done; **remaining: walk it with Tom.**
- 🔨 **#147 IN FLIGHT** — brief landed, red written, build running.
- 🆕 **#94 rides #147's R1a** (transition rule lands in the shared gate).
- 🔜 **#7 (GitHub)** red-first; **#9** reply drafted past Waffles before posting;
  **#6** missing pin + `max_concurrency`-ignored-on-liminal as their own rows.
- Held on Tom: #139 live cut, #142, the 0.11.1 decision, and the deck question
  (him-over-shoulder vs someone-alone).

## Method memories earned or re-earned at this marker

`one-truth-in-two-places-has-drifted` (the whole #147 finding) ·
`mutation-must-sit-on-the-tests-path` (the red's phase 3 exists because a
suppression rule that only ever suppresses is indistinguishable from never
logging) · **new, unwritten:** *an actuator whose target can be empty does not
fail closed* · `a-stated-reason-is-evidence-both-ways` ·
`sampling-cannot-establish-quiet` (a sampled peak is a lower bound).

---

# AMENDMENT — RED CONFIRMED, and the exact next steps

## 🔴 The red is confirmed and its evidence is the best statement of the defect

`cargo test -p aion-server --test grpc_admission_refusal_logs` — **FAILED**, at
phase 1, with its own message:

> *the server refused the registration and logged NOTHING about it; captured:*
> **312 events captured in total (0 from aion targets, 312 from elsewhere — the
> subscriber was demonstrably live). NO aion-target events at all.**

Both halves matter:
- `assert!(first.is_err())` **passed first**, so the refusal genuinely happened.
  The subject occurred; the test is not vacuous.
- 312 events from `h2`/`hyper`/`tonic`/`mio` reached the subscriber during that
  refusal. **The instrument is demonstrably live, so the absence is real
  absence.** Absence paired with survival is the only form that proves anything.

`render` in `tests/test_support/capture.rs` was rewritten to make this readable:
aion-target events in full, everything else reduced to per-target counts. **The
counts are load-bearing, not politeness** — they are what proves the subscriber
was alive.

## Measured disk draw — OWED TO CALLY, send it

**`du` on `aion/target` = 4.42 GiB** at build completion (from `watch.log`).
`~/.cargo` **flat at 12.05 GiB throughout — +0**, so this lane pulled nothing
from the shared cache. State it **target-only** and as a **spend, not a
ceiling** (a peak allowance is added before it enters Σ). Cally's re-scaling was
right: the ~25-37 GiB prior was archaeology, not build cost. Fire 00:23:39Z,
138 GiB avail, floor 40, watchdog stood down cleanly on group exit.

**Cally has queued the chanreg measurement lane to me, to open once #147 lands** —
Athena's terms 1-9 plus Cally's added term (no other cold lane on the box during
the cache window). Full terms in the 00:29:32Z DM.

## 🔴 TOM'S ORDER, 00:30Z: "make sure that all gets fixed"

Everything, not just #7. I answered in plain language with the six-item
programme and he confirmed at 00:32Z. Order: **#147 first (half-built), then
down the list.** #9's public reply still goes past Waffles before posting.

## WHERE THE IMPLEMENTATION IS — resume exactly here

**Written and on disk:** `crates/aion-server/src/worker/admission_audit.rs`.
`AdmissionAudit` with `should_name`, `clear_admitted`, `clear_queue`,
`remembered_sites`. Keyed `(task_queue, node)` — server-derived — with the
digest over the client's part. Poisoned lock **recovers and still names**,
because declining to log would recreate the defect.

**Home decided:** `ConnectedWorkerRegistry` (`registry.rs:288`), which is
`#[derive(Clone)]` over `Arc<Mutex<RegistryState>>`, so an added
`Arc<AdmissionAudit>` field is shared across clones. **Both callers already hold
the registry**, so this is ONE home rather than the two the brief anticipated.

**Remaining, in order:**
1. `admission_audit_tests.rs` — the unit tests the module already `#[path]`s to
   (it will not compile until this exists).
2. Add `audit: Arc<AdmissionAudit>` to `ConnectedWorkerRegistry` + accessor.
3. `contracts.rs`: add the audit parameter; at the `Ok(())` return (`:146`) call
   `clear_admitted`; at the `Mismatch` construction (`:148`) consult
   `should_name` and `tracing::warn!` with message **`REFUSED worker
   registration`** and fields `task_queue`, `node`, `identity`, `reason`. The
   constant the red asserts on is `REFUSAL` in
   `tests/grpc_admission_refusal_logs.rs`.
4. Thread the audit through both callers: `worker_grpc.rs:625-666` wrapper and
   `liminal_transport.rs:1568` `validate_registration_contract`.
5. **Restructure the liminal caller** so it stops restating the contract class
   (`on_worker_registered`'s `inspect_err`, `:1687-1712`) and keeps its log for
   the classes the gate cannot see.
6. **Amend** `tests/dead_man_switch_refusal_logs.rs:83` to assert the gate's
   message. **Waffles' condition: it must still FAIL with the naming absent —
   show that, don't claim it.**
7. R2: the availability hint at `awl/run_loop.rs:275` reads the audit so it can
   tell "none started" from "one refused".
8. `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt`.

⚠️ **Unused-import/dead-code warnings are live** in
`tests/test_support/grpc_admission.rs` (`action_named`, `descriptor_for`,
`register`, `register_on_node` unused from the refusal-logs binary). Clippy is
`-D warnings`, so these must be resolved — the honest fix is that each test
binary only imports what it uses, not an allow.

---

# AMENDMENT 3 — #147 R1/R1a/R1b/R2 BUILT AND GREEN (6 August 2026)

**All eight committed steps are done.** `cargo clippy -p aion-server --all-targets
--features liminal-transport -- -D warnings` → **EXIT=0**, no allows anywhere.
`cargo fmt --all` run.

## What landed

1. **`worker/admission_audit.rs` + `admission_audit_tests.rs`** — 17 unit tests,
   all green. The value is now a `RememberedRefusal { node, identity, reason }`,
   **not a hash**: R2 needs to SAY the reason, and a digest can answer only
   "different?". `DefaultHasher` deleted entirely (subtraction).
   Key stays `(task_queue, node)` — server-derived, so bounded by construction;
   `the_refused_party_cannot_grow_the_map` drives 1000 client-chosen identities
   through one queue and asserts `remembered_sites() == 1`.
2. **`worker/contracts.rs`** — `validate_worker_contracts` takes `&AdmissionAudit`;
   `clear_admitted` on the `Ok(())` path; `name_refusal()` is the ONE log site,
   message **`REFUSED worker registration`**, fields `task_queue` `node`
   `identity` `reason`. **The catalog-failure branch names itself too** — leaving
   one refusal branch voiceless is how this survived its first fix.
   Named from `refusal.to_string()`, never `details` alone: the
   `WORKER_CONTRACT_MISMATCH:` prefix lives in the Display and is what makes it
   greppable. (Caught by the red — first implementation logged `details`.)
3. **`worker/registry.rs`** — `audit: Arc<AdmissionAudit>` + `admission_audit()`.
   Registry clones share one, so gRPC and liminal cannot drift again.
4. **Both callers threaded** — `worker_grpc.rs`, `liminal_transport.rs`.
5. **Liminal caller stops restating.** `on_worker_registered` now calls
   `validate_registration_contract` FIRST and propagates it silently; the
   transport's own WARN covers only the refusals it owns (supervisor, registry,
   lease). `admit_registration`'s doc records why the contract check left it.
6. **`awl/run_loop.rs` + `run_loop_tests.rs`** — R2. `unserved_hint()` reads
   `refusals_on_queue()`. No refusal on record → the old scaffold text. A refusal
   on record → *"A worker IS dialling task queue `X` and the server is REFUSING
   it… Starting another worker will not help"* + build identity + reason + node
   + a count of further refused connections. 4 tests.

## The controls (Waffles' condition 3 — DEMONSTRATED, not claimed)

Deleted the `tracing::warn!` from `name_refusal`, ran both pins, restored the
file **byte-identically** (`cmp` verified):

| pin | naming present | naming DELETED |
|---|---|---|
| `grpc_admission_refusal_logs` | ok | **FAILED** |
| `dead_man_switch_refusal_logs` | ok | **FAILED** |

The liminal absence report showed **875 events captured, 174 from aion targets**
— absence paired with survival. Control log:
`<scratchpad>/absence-control.log`.

## Dead-code warnings resolved BY USE, zero allows

- `mismatched_fetch` — `worker_contract_registration` now uses it instead of its
  own byte-identical inline copy (one truth, one place).
- `register`/`register_on_node`/`descriptor_for` — the refusal-logs binary uses
  them (`descriptor_for` powers new PHASE 5: admitted → site cleared → the same
  break speaks again).
- `PARTITIONED_SOURCE` + `action_named` — MOVED into
  `worker_contract_registration.rs`, its only consumer. Shared support carries
  the intersection; a single-consumer helper belongs to that consumer.
- `capture_log`/`render`/`target` — `dead_man` and `unserved_queue_honesty` now
  use them (the latter's raw `{captured:?}` dump replaced by `render`).
- **`panic_any` was clippy-denied** (`clippy::panic`). The poison fixture now
  unwinds via an assertion WRITTEN TO FAIL (`assert!(held.is_err(), ...)` on a
  healthy lock) — the harness's own failure mechanism, no exemption needed.
- Two `too_many_lines` splits: both refusal-log tests are now named phase/half
  functions.

## Test state at this point

- `-p aion-server --lib -- worker::admission_audit awl::run_loop` → 21 tests, ok
- `--test grpc_admission_refusal_logs` ok · `--test dead_man_switch_refusal_logs`
  ok · `--test worker_contract_registration` 4 ok · `--test unserved_queue_honesty` ok
- **NOT YET RUN: the whole `aion-server` suite.** That is the next command.

## Remaining on #147

1. `cargo test -p aion-server --features liminal-transport` (full crate).
2. Commit + push.
3. Amend the brief + `SHARED-QUEUE-SILENT-REFUSAL-2026-08-06.md` with what was
   built, and note **#94 is closed by the transition rule**.
4. Tell Waffles: R1/R1a/R1b/R2 built, condition 3 discharged with the control
   table above. R3 remains Waffles' to send to Tom — **I do not send it.**

## Then, Tom's "fix everything" order (unchanged, in order)

#7 completion-monitor transient store error (red-first, harness
`nif_child_watch.rs:1142`) · #9 operator-facing fact in `details` + a reply that
goes past Waffles BEFORE it posts · #6 `max_concurrency` ignored on liminal +
missing pin · #5 AWL `sum` combinator (bump `Keyword::ALL`'s `[Self; 57]` in the
SAME change) · #1 `workflow.now()` history tail vs replay position
(`Replay::now()` exists unwired).

**Owed to Cally:** measured `du` = **4.42 GiB target-only**, `~/.cargo` **+0**,
stated as a spend not a ceiling. Sending it releases the chanreg lane.

**Tom's pids 45425 (server) / 48399 (worker): untouched all session.**

**LANDED `3d034c686`, pushed** — `origin/main` = `3d034c686`.

**Full crate suite RUNNING DETACHED** →
`/private/tmp/claude-501/-Users-tom-Developer-ablative-stack-aion/5849e0d8-4802-4869-8e0f-9f7fd187e198/scratchpad/aion-server-suite.log`
(`cargo test -p aion-server --features liminal-transport --no-fail-fast`).
On resume: `grep -E "^(test result|error)" ` that log. Count VERDICT LINES, not
prose. Any red is a finding to fix, not a flake to re-run.

---

# AMENDMENT 4 — 2026-08-06, #147 close-out found a defect in its own bound

## The suite verdict from amendment 3, read

**47 verdict lines. 46 green. One red, and it was NOT a defect.**

`build_identity::tests::the_stamped_commit_is_the_commit_that_is_checked_out`
failed: stamped `3d034c686`, tree `28a814b58`. Established rather than assumed —

- suite build started `2026-08-06T11:08:20+1000`
- marker commit `28a814b58` landed `2026-08-06T11:08:29+1000`

**Nine seconds.** I committed the marker while my own battery was mid-build. The
build script stamped the tree it started on, its rerun triggers are correct
(`.git/HEAD`, the named ref file AND `packed-refs` — the branch-moves case is
explicitly handled), and #123's pin caught exactly what it was built to catch.

🔴 **PROCESS LAW EARNED: do not commit while a battery is in flight.** The
build-identity pin will fail, and the failure is indistinguishable at a glance
from a stale-stamp defect. Land, then measure.

## 🔴 THE REAL FINDING: R1b's bound was FALSE as shipped

Waffles' condition 2 was *bounded by construction*. The approved R1b design
justified it with *"`(task_queue, node)` — both server-derived, and already
enumerated by the gate from the deployed catalog."*

**Both arrive on the wire** — `worker_grpc.rs:652-653`,
`liminal_transport.rs:1571-1572`. The design NAMED this trap in its own words
(*"a map keyed on anything the refused party controls cannot be bounded by
construction"*), moved `identity` out of the key, and **left `node` in it**.

`dispatch_can_reach` demands every UNPINNED action of every locality, so a
refused worker varying its advertised node allocated one entry per dial with
nothing able to evict it — a refused connection never enters the registry.
**Measured under control: 100 dials → 100 sites.**

`task_queue` was bounded after all, but by a mechanism nobody had written down:
a queue with no reachable contract demands nothing, so it ADMITS
(`worker_contracts_for_admission` → empty) and never reaches the map.

### Fixed, landed `fdd694c4b`, pushed

`RefusalSite` is now built by the gate from the CATALOG
(`contracts::refusal_site`). A node names a site only when a demanded action is
pinned to it. An unreadable catalog is ONE site, not one per queue name tried.
Bound: catalog queues × (pinned nodes + 1) + 1, every component server-derived.
`clear_queue` wired to the queue-retired case (fossil-refusal hygiene, NOT the
bound).

**Control table — both directions, because a bound test alone is satisfiable by
collapsing everything:**

| mutation | `…never_pins_cannot_allocate_a_site` | `a_node_the_catalog_pins_is_a_site_of_its_own` |
|---|---|---|
| none (as built) | ok | ok |
| key on the raw advertised node | **FAILED** (100 ≠ 1) | ok |
| collapse every locality | ok | **FAILED** (1 ≠ 2) |

Each control kills exactly its own test. `contracts.rs` restored `cmp`-verified
byte-identical after both.

Gate-level bound measured THROUGH `validate_worker_contracts` against a real
deployed catalog carrying a node-pinned action (`V_PINNED` fixture), not
asserted of a hand-built site.

### 🔴 THE LAW THIS EARNED

**A bound is a claim about every dimension the client can move, so it takes one
witness per dimension.** The original test gave the refused party a thousand
identities and proved only that `identity` had been kept out of the key — which
it had. The dimension actually left open was never asked about, and **the
module's own doc asserting it was safe is what stopped anyone checking.** A
single witness over one dimension reads exactly like a proof of the whole claim.

Sibling of `confidence-is-what-stops-you-measuring` and
`assert-the-invariant-not-the-enumeration`.

## State

- `origin/main` = `fdd694c4b`. Tree clean but for the three NO-SWEEP dirs.
- clippy `-p aion-server --all-targets --features liminal-transport -D warnings`
  **EXIT=0**, no allows. 36 unit + 4 integration pins green.
- Brief § AS BUILT and the incident record's § RESOLVED both written and
  committed. **#94 closed by the transition rule.**
- **Cally: row CLOSED AS INVALID BY CONSTRUCTION** — a net pool delta cannot be
  closed by a sum of gross draws. My 11.49 GiB is NOT to be entered as a
  summand. Sampler declined by Cally; board rule is now `du` at lane open/close.
  Also corrected: `du` over-read is TREE-DEPENDENT — near 1 on `aion/target`
  (df −5 vs du +4.0 over 00:24–00:31Z), not the 3–9× measured on shards.
- **Full crate suite #2 RUNNING DETACHED** on the settled tree `fdd694c4b` →
  `<scratchpad>/aion-server-suite2.log`. **Nothing may be committed until it
  finishes** (see the process law above). On resume:
  `grep -E "^test result" ` and count lines that do NOT say `0 failed`.
- **Owed:** report R1/R1a/R1b/R2 + the bound correction to Waffles. **R3 stays
  Waffles' to send — I do not send it.**
- Tom's pids 45425 / 48399 untouched.

---

# AMENDMENT 5 — 2026-08-06T01:40Z, compaction marker

## Waffles ACCEPTED #147 in full (01:36Z) — lane closed bar the suite result

He took joint signature on the R1b defect: *"I banked your trap-law the hour you
wrote it and then approved a key without asking the one question the law demands
per dimension: where does this component ARRIVE from."*

**BOTH proposed laws are ON THE ESTATE BOOKS as written:**

1. **A bound is a claim about every dimension the client can move, so it takes
   one witness per dimension.**
2. **Do not commit while a battery is in flight** — the #123 build-identity pin
   goes red and is indistinguishable at a glance from a real stale-stamp fault.

**🔴 THE MECHANISM BEHIND BOTH, from Waffles, worth more than either law.**
Hermes banked *"an arming probe that executes the path under test is a
contaminant"* on the liminal lane the same hour — and in the SAME message failed
to apply it, retracting a true finding on a contaminated instrument. Same shape
as mine: I held the trap-law and asked it of one dimension only.

⇒ **A LAW BANKED IS NOT A LAW APPLIED, AND THE WINDOW WHERE IT IS MOST LIKELY TO
BE SKIPPED IS THE ONE RIGHT AFTER WRITING IT, WHEN IT FEELS MOST OWNED.**

Also worth keeping, his phrasing: *"a bound nobody wrote down is not a bound,
it's a coincidence with good posture"* — re `task_queue`, which was safe only via
the unwritten admits-when-nothing-is-demanded path, now written down.

**Nothing for him to re-gate.** R3 CONFIRMED HELD at his seat for Tom, untouched,
after the liminal chain closes. **I do not send it.** He asked only for the suite
result when it settles.

## Suite #2 status at compaction: STILL RUNNING

- verdict lines so far: **19**, of which NOT-"0 failed": **0**
- lines matching `^error`: **0**
- **lib target SETTLED GREEN: `796 passed; 0 failed`** — and 790→796 is its own
  arithmetic check that exactly the six new tests landed and ran (4 audit-bound
  witnesses + 2 gate-level).
- log: `<scratchpad>/aion-server-suite2.log`, tree `fdd694c4b`

## 🔴 A THIRD TRAP, EARNED WHILE WRITING THIS AMENDMENT

`cat >> file <<MD` with an **unquoted** delimiter lets zsh evaluate backticks in
the heredoc body — `` `task_queue` `` ran as a command and mangled the append.
This is the known `git commit -m` backtick trap in a second costume.

⇒ **Quote every heredoc delimiter (`<<'MD'`) whenever the body contains
backticks, or write the file with the Write tool instead.** The commit-message
memory names only `git commit`; the hazard is the SHELL, not the verb.

## 🔴 ON RESUME, IN ORDER

1. `grep -E "^test result" <scratchpad>/aion-server-suite2.log | grep -v " 0 failed"`
   — count VERDICT LINES. Empty output + no `^error` = green.
   **NO COMMITS until it is settled.**
2. **Send the settled result to Waffles**
   (`dm:896955e1-86dd-4d6a-9c25-f3f978b189a9`). That closes the #147 lane.
   **R3 NOT sent.**
3. Then Tom's "fix everything" queue, in order: **#7** completion-monitor
   transient store error (red-first, harness `nif_child_watch.rs:1142`) · **#9**
   operator-facing fact in `details` + a reply that goes past Waffles BEFORE it
   posts · **#6** `max_concurrency` ignored on liminal + missing pin · **#5** AWL
   `sum` combinator (bump `Keyword::ALL`'s `[Self; 57]` in the SAME change) ·
   **#1** `workflow.now()` history tail vs replay position (`Replay::now()`
   exists unwired).
4. Chanreg measurement lane is RELEASED (Cally's row closed as invalid by
   construction) but still queues behind #147.

**Tom's pids 45425 (server) / 48399 (worker): untouched all session.**
**origin/main = `fdd694c4b`.** Tree clean but for the three NO-SWEEP dirs.

---

# AMENDMENT 6 — 2026-08-06, suite settled GREEN, #147 lane closed

## The verdict

`cargo test -p aion-server --features liminal-transport --no-fail-fast`, tree
`fdd694c4b`, run detached to `<scratchpad>/aion-server-suite2.log`:

- **46 verdict lines**, every one `0 failed`
- **0 verdict lines with a non-zero `ignored`** — nothing skipped its way to green
- **0** lines matching `^failures:`, `^error`, or `thread ... panicked`
- lib target **796 passed** (790 before, and 790→796 is the arithmetic check that
  exactly the six new tests landed AND ran: 4 audit-bound witnesses + 2
  gate-level)
- 981 passed summed across all targets

Two verdict lines read `1 passed; 795 filtered out` — those are child re-execs,
where a test spawns a process running one named test. Expected, not a partial run.

## 🔴 THE RESUME GREP IN AMENDMENT 5 WAS ITSELF MIS-KEYED

Amendment 5 told the next seat to run:

```
grep -E "^test result" LOG | grep -v " 0 failed"
```

That printed one line, which for a moment looked like a failure:

```
test result_frames_are_acked_including_duplicates ... ok
```

It is not a verdict line. It is a TEST NAME that begins with `result`, and
`^test result` matched it because the pattern was not anchored on the colon that
actually distinguishes a verdict from a test name. The correct pattern is
`^test result: `, with the colon and the space.

⇒ **A COUNTER KEYED ON A PREFIX COUNTS EVERYTHING THAT SHARES THE PREFIX.** The
estate already holds *count verdict lines, never prose* — this is the same law
one level down: the thing that makes a verdict line a verdict line is its
SEPARATOR, not its opening word, and a pattern that stops short of the separator
is reading prose again. It failed in the safe direction here (a false alarm, not
a false green) but the identical mistake in `grep -c` would have inflated the
verdict count by one and nobody would have looked.

Both directions matter, so the settled check is now three greps, not one:

```
grep -cE "^test result: " LOG                              # how many verdicts
grep -E  "^test result: " LOG | grep -v "; 0 failed;"      # any red
grep -E  "^test result: " LOG | grep -v "; 0 ignored;"     # any skipped
grep -cE "^(failures:|error(\[|:)|thread .* panicked)" LOG # any harness death
```

The `; ` and `;` around `0 failed` matter for the same reason the colon does.

## Lane state

- **#147 CLOSED.** `origin/main = fdd694c4b`. Waffles accepted in full at 01:36Z;
  both laws are on the estate books; **R3 stays at his seat, unsent by me.**
- Suite result sent to Waffles, which was the one thing he asked for.
- **Chanreg measurement lane is now unblocked** — it queued behind #147 and #147
  is done. Cally's row stays closed as invalid by construction.

## NEXT, in order (unchanged from Amendment 5 bar the head)

1. **#7** completion-monitor transient store error — red-first, harness
   `nif_child_watch.rs:1142`.
2. **#9** operator-facing fact in `details` (`RETRY-CLASS-HONESTY-90.md` §5) —
   **the reply goes past Waffles BEFORE it posts.**
3. **#6** `max_concurrency` silently ignored on liminal + the missing pin.
4. **#5** AWL `sum` combinator — bump `Keyword::ALL`'s `[Self; 57]` in the SAME
   change.
5. **#1** `workflow.now()` history tail vs replay position (`Replay::now()`
   exists unwired).

**Tom's pids 45425 (server) / 48399 (worker): untouched.** Worker rebuilds still
frozen estate-wide until the #139 live cut.
