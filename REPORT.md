# aion#42 — parallel-fork ordinal assignment diverges under load

**Lane:** `42-fork-ordering` · **Builder seat** · **Date:** 2026-08-18
**Charter:** `gate-logs/42-fork-ordering/CHARTER.md` (Vesper Lynd, steward)
**Battery standard:** `ablative/docs/GATE-EXTRACTION-STANDARD.md`

**Snapshot.** Every SHA below is as of base `4150d3817` ("docs(42): lane charter
— parallel-fork ordinal assignment diverges under load"), itself off main
`efc71eda4` ("docs(36): battery of record on the rebased tree, and F8 closed
with a fix"). Commits are cited by subject; the SHAs are this snapshot's, not
durable names.

---

## Verdict in one line

**The charter's premise is falsified by measurement.** The fork's activity
ordinals are NOT assigned in opposite order — they are assigned identically,
deterministically, from written order, on both backends. What diverges is the
**order two `ActivityCompleted` events occupy in the recorded history**, and
that is a property the design deliberately declines to promise and that replay
is explicitly built to tolerate. Per the charter this is **not a lane
decision**: I have stopped short of a fix and the disposition is reserved to
the steward.

---

## 1. Built vs expressible — the answer, with citations

### 1a. Deterministic ordinal ASSIGNMENT is PROMISED, and is BUILT, and HOLDS

The promise is stated at three escalating levels of bindingness, and per
`CLAUDE.md:85` the brief is the authoritative one:

- `docs/design/aion-core/design.json:19` (D4) — "ActivityId derives from the
  scheduling sequence position so replay can match a recorded result to its
  call deterministically."
- `docs/design/aion-durability/DESIGN.md:197-213`, section titled *How replay
  matches a recorded result to a call*, opening "This is the load-bearing
  mechanism, so it is specified precisely" — "the Nth activity scheduled in a
  run is always activity N."
- `docs/design/aion-durability/design.json:21` (D3) — "Rejected content-hashing
  the call or relying on wall-clock ordering: ordinals are stable under replay
  precisely because workflow code is deterministic, which is the whole
  premise."
- `docs/design/aion-time-signals/briefs/AT-009.json:15` — the binding EARS
  requirement: "THE SYSTEM SHALL derive a per-spawn correlation token
  deterministically from the spawning sequence position."
- `docs/design/aion-time-signals/design.json:27` (D11) — the token "derives
  deterministically from the spawning sequence position so replay matches
  recorded results to spawn positions identically", and matching by
  activity/workflow type was **explicitly considered and rejected**.

As built, on the fan-out path a fork actually takes:

- `crates/aion/src/runtime/nif_collect.rs:224` — `let base_ordinal =
  context.allocate_activity_ordinals(count);` — one contiguous block, allocated
  once, **pinned before any side effect** (`nif_collect.rs:5-10`: an unpinned
  re-entry "would shear the ordinal↔event correlation").
- `crates/aion/src/runtime/nif_collect.rs:259-260` — `ordinal = base_ordinal +
  offset` over `specs.iter().enumerate()`: ordinal is base + index into the
  **written-order** spec list. Arrival order cannot reach it.
- `crates/aion/src/runtime/nif_collect.rs:329-340` — "Record the whole batch
  first so the Scheduled range stays contiguous."
- Both compilations agree on that written order:
  `crates/aion-awl/src/mir/lower/parallel.rs:1-3` (direct: a layer is "ONE
  `workflow.all`", members in written order) and
  `crates/aion-awl/src/emitter/forks.rs:361-383` (reference:
  `lower_hetero_parallel`, same source-order list).

**This half of the invariant is intact and I measured it holding** (§2).

### 1b. A deterministic TOTAL ORDER over concurrent terminal events is NOT promised — it is explicitly declined

- `crates/aion/src/durability/cursor.rs:289-296`, the decisive statement:
  "Asynchronous arrivals (signals, child terminals, **a parallel activity's
  events**) can be recorded between this activity's `Scheduled` anchor and its
  terminal. They belong to other commands: the walk skips them in place —
  neither consuming them nor failing replay... **a foreign interior event is an
  interleaving artifact, not a command-stream divergence.**"
- `crates/aion/src/runtime/nif_collect_settlement.rs:32-37` — "Fail fast: the
  lowest-ordinal recorded failure. **The rule is a function of the recorded
  terminal *set***, so replay derives the same value." The settlement rule was
  deliberately engineered to not depend on arrival order.
- `crates/aion/src/runtime/nif_collect.rs:22-25` — "Replay never re-races and
  never consults the runtime maps: the result is **a total function of the
  per-ordinal recorded terminal set**."
- `docs/design/CONTROL-PLANE-PHASE-2.md:130` — "the completion is recorded by
  the await path keyed on ordinal, **order-independent of dispatch timing**."
- `docs/design/aion-ops-console/VISION.md:47-50` — "a durable execution is **a
  partial order of concurrent events**."

### 1c. Therefore this is NOT a replay correctness hazard

The charter's sharpening ("a recorded history replayed against re-executed code
could bind results to the wrong branch") requires replay to bind by *position*.
It does not: both replay mechanisms bind by **ordinal**.

- Cursor path: `crates/aion/src/durability/correlation.rs:129-131` derives
  `CorrelationKey::Activity(activity_id.sequence_position())`; the terminal is
  then found by **activity-id equality within the walk**
  (`crates/aion/src/durability/cursor.rs:306-360`), not by position.
- Fan-out path (what a fork uses):
  `crates/aion/src/runtime/nif_collect_settlement.rs:280-306`
  (`recorded_terminal`) scans history for the event whose `activity_id ==
  ActivityId::from_sequence_position(ordinal)`. Not positional.

I did not take this on the citations alone — see the control arm in §2.

### 1d. Where the divergence actually comes from

`crates/aion/src/runtime/nif_collect_settlement.rs:24-31` (`settle_all`) sweeps
the range in ordinal order but records only what has **already arrived**, via
`take_and_record` (`nif_collect_settlement.rs:199-217`). Each member is a
detached tokio task (`crates/aion/src/runtime/nif_collect.rs:350-372`), and
each mailbox wake is a separate sweep. Idle box: both results are in the
runtime map at the first wake → one sweep records them in ordinal order. Loaded
box: two wakes → the trail records **arrival** order. That is the whole defect,
and it is why solo runs always pass.

The comparison is positional and unsorted:
`crates/aion/tests/common/trail_norm.rs:28-38` normalizes only identity fields;
`crates/aion/tests/awl_bc4_differential/report.rs:5` — "Decision 11 is law: a
divergence is never normalized away."

The BC-4 brief's premise is where the gap opened.
`docs/design/aion-authoring/awl/BC-4-DIFFERENTIAL-BRIEF.md:47-51` promises
"canned deterministic results from one dispatcher implementation used for BOTH
runs... **so trails can only diverge on backend behavior**." A deterministic
dispatcher does not make *completion order* deterministic once two activities
run concurrently. That premise was sound when the corpus was 70 fixtures; it
stopped being sound at 105, when dependency-parallel layers joined
(`crates/aion/tests/awl_bc4_differential/covered.rs:52-61`).

---

## 2. Load-independent reproduction — measured, not inferred

Commit: **`test(42): reproduce the parallel-layer trail divergence without
load`** (`d21abfc6e`).

Both tests live in `crates/aion/tests/concurrency_e2e.rs` and reuse the gate
seam that file already owns (`GateBoard`/`GatedDispatcher`, lines 43-116) — the
dispatcher blocks until the test releases a named gate, so settle order is
**imposed, never raced**. No load generator, no thread pinning, no ambient
noise.

### The red: `a_parallel_layer_records_one_trail_whichever_member_settles_first`

Runs `all_two` twice over fresh stores, releasing gates `a`->`b` and then
`b`->`a`, waiting for each member's terminal to be in history before releasing
the next.

```
thread 'a_parallel_layer_records_one_trail_whichever_member_settles_first' panicked at
crates/aion/tests/concurrency_e2e.rs:1068:5:
assertion `left == right` failed: the same layer over the same deterministic
dispatcher recorded two different durable trails purely because the members
settled in a different order
  left:  [.. "2|sched|0|gated_ok:a|\"in\"", "3|astart|0|1", "4|sched|1|gated_ok:b|\"in\"",
          "5|astart|1|1", "6|acomp|0|\"done-a\"|1", "7|acomp|1|\"done-b\"|1"]
 right:  [.. "2|sched|0|gated_ok:a|\"in\"", "3|astart|0|1", "4|sched|1|gated_ok:b|\"in\"",
          "5|astart|1|1", "6|acomp|1|\"done-b\"|1", "7|acomp|0|\"done-a\"|1"]
```

**5/5 red in ~0.66s on an idle box.** Receipts:
`gate-logs/42-fork-ordering/red-reproduction.txt`.

This is the discriminator the question needed, and it settles it three ways:

1. **The `sched` anchors are byte-identical in both runs** — `sched|0|gated_ok:a`
   and `sched|1|gated_ok:b`, written order, both times. The ordinals did not
   move.
2. The test's own earlier assertion — that the inverted run binds the same
   result to the same ordinal — **passes** before the trail assertion fails.
   Ordinal-to-branch binding is preserved under inversion.
3. Only the two `acomp` events trade places.

The five recorded sightings corroborate independently: in
`gate-logs/36-follow-on/round1-rebase-nextest.stderr:4887-4896`, reference
`event[9]` is `activity_id=1` with result bytes `f,a,c,t` and direct `event[10]`
is `activity_id=1` with the same bytes. Both backends bind ordinal 1 to
`extract_claims`/`facts` and ordinal 2 to `collect_citations`/`links`
(`crates/aion-awl/tests/fixtures/rev2/dag-fork/valid/after_multi_diamond.awl:24-28`).
The pairing is preserved; only the positions swap.

### The control arm: `a_layer_settled_in_inverted_order_replays_each_result_onto_its_own_member`

**PASSES.** Records the inverted-settle history (`6|acomp|1|"done-b"`,
`7|acomp|0|"done-a"`), restarts the engine over it, and requires replay to
resolve the collect from recorded events alone. Replay returns the
input-ordered `["done-a", "done-b"]`, appends only the release signal and the
terminal, and re-dispatches nothing
(`restart_replay_and_finish`, `concurrency_e2e.rs:406-443`, asserts
`gates.finished_dispatches() == 0`).

**Settle order does not corrupt replay.** That is the measurement, not a
reading.

---

## 3. Why I stopped instead of fixing

The charter's step 3 reserves the disposition to the steward when the design
does not promise the asserted property. It does not. Beyond that, I found a
**hard constraint that rules out the obvious fix**, and it is measured, not
argued:

The naive remedy — have `settle_all` record terminals only in contiguous
ordinal order — **breaks a currently-green pinned behaviour.**
`collect_all_fail_fast_cancels_unresolved_and_replays_byte_identically`
(`crates/aion/tests/concurrency_e2e.rs:723-773`) releases only the FAILING
member, ordinal 1, while ordinal 0 is still running, and pins the trail:

```
shape[5] == "6|afail|1|boom-b|1"
shape[6] == "7|acancel|0|1"
```

Ordinal 1's terminal is recorded **before** ordinal 0 resolves — that is
precisely what lets fail-fast cancel the still-running sibling. Deferring
higher-ordinal terminals until lower ones resolve would mean ordinal 1's
failure could not be recorded until ordinal 0 finished on its own, and the
early cancellation would never fire. **Out-of-ordinal-order terminal recording
is load-bearing for fail-fast, not incidental.** Any canonicalisation of trail
order has to price that.

Narrowing the differential is likewise **reserved to the reviewer** and I have
not touched it: no fixture excluded, no normalizer extended, no assertion
weakened, nothing added to `EXCLUDED_OUT_OF_ORACLE`.

### Prior art the steward should see

`docs/design/aion-authoring/FINDING-BC4-DIFFERENTIAL-NONDETERMINISM-2026-08-06.md`
reached the same conclusion on 2026-08-06 at tree `9189ba585`, was filed
explicitly "Filed, not fixed", and has never been dispositioned. Its lines
65-69 read "two concurrent branches landing in the opposite order — the trail
is being compared without a total order over concurrent events", and its
proposed remedy (lines 95-99) is "sort fork branches by a stable key, not by
arrival", with "**Do not** widen the `excluded` set to make it pass."

This lane's contribution over that finding is that it is now **measured and
deterministically reproducible** rather than inferred, and that the
ordinal-assignment half is proven intact.

---

## 4. Non-claims — stated explicitly

- **I do not claim the differential test is wrong.** I claim it asserts a total
  order over concurrent events that the durability design declines to promise.
  Whether the right resolution is to give the engine that total order, to give
  the harness a stable-key sort, or something else, is the steward's call.
- **I do not claim a fix is cheap.** Section 3 shows the obvious one has a real
  cost.
- **I did not run the five sightings to ground myself.** I read their receipts
  (`gate-logs/36-follow-on/round1-rebase-nextest.stderr`) and reproduced the
  *mechanism* in isolation. I did not reproduce
  `covered::covered_set_is_byte_identical` going red on demand.
- **I did not bisect** when the parallel-layer fixtures began diverging.
- **Secondary observation, not measured:** the determinism guard at
  `crates/aion/src/runtime/nif_collect.rs:261-269` compares the recorded
  activity *type* at an ordinal against what workflow code supplied. Where two
  fork branches call the *same* action, that guard is trivially satisfied and
  would not catch a branch-order change in edited workflow code. This is
  generic to the guard, not specific to #42, and I have not tested it.
- The lane touched **only** `crates/aion/tests/concurrency_e2e.rs` plus
  `gate-logs/42-fork-ordering/` and this file. **No spill beyond `crates/aion`.**
  No engine source was modified.

---

## 5. Battery

See `gate-logs/42-fork-ordering/BATTERY.md` for the verdict, the instrument's
proven-red control, and the per-leg completion proof.

**The single nextest finding is
`a_parallel_layer_records_one_trail_whichever_member_settles_first` — this
lane's charter-mandated red reproduction, left deliberately red and undisposed
because section 3 reserves its resolution to the steward.** It is disclosed
here rather than silenced: no `#[ignore]`, no `#[allow]`, no skip, no weakened
assertion.
