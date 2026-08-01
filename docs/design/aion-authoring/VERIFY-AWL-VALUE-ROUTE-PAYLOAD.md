# Verification — AWL value route payloads (B4)

**Verifier: Vesper Lynd, at the bytes. 2026-08-01 ~00:50Z.**
Implementer: Opus subagent `a46aafc4e0cb7b421`, main tree, uncommitted by design.

The pin retired: `"value route payloads (`route out(<value>)`) — the rev-3 flow
shape is not yet lowered (B4)"`. The standing hazard was the one that cost the
#90 revert: **a refusal pin is a CONTRACT, not a stale expectation.** Retiring
one is legitimate only when the differential proves it, and a fixture that
cannot pass stays refusing and gets said out loud.

## The check that mattered most: was the RATCHET raised, or the POPULATION grown?

A ratchet reading `91 → 98` proves nothing if seven easy fixtures were authored
to fill it. That is proof against the wrong population, in the exact shape that
made #90's five-arm mutation suite worthless.

| fixture | tracked at |
|---|---|
| action_param_hygiene | `a3370e9a` |
| counter_hygiene | `dd53c023` |
| region_close_bounded | `449f3def` |
| region_nested_local_reuse | `dd53c023` |
| region_sibling_local_reuse | `dd53c023` |
| temporary_hygiene | `449f3def` |
| value_route_payload | `0eb05327` |
| layer_hygiene | `449f3def` |

**All eight pre-existed. `git status` shows ZERO new `.awl` fixtures.** The
ratchet moved by retiring fixtures that were already refusing. ✅

## Arithmetic (independent of the implementer's claim)

- `aion-awl` `COVERED` list independently = **98**, all unique, no overlap between buckets

🔴 **CORRECTION — MY FIRST BUCKET COUNTS WERE WRONG, AND THEY SUMMED TO 98
ANYWAY.** I wrote "58 succeeded + 38 failed + 2 out-of-oracle = 98" and repeated
it to the implementer. There are **four** buckets, not three: I missed
`EXPECTED_TIMER_PARKED`, and my regex extraction folded its 3 entries into
`EXPECTED_FAILED`. The true partition, counted by line range:

| bucket | count |
|---|---|
| EXPECTED_SUCCEEDED | 58 |
| EXPECTED_FAILED | 34 (was 35) |
| EXCLUDED_OUT_OF_ORACLE | 3 (was 2) |
| EXPECTED_TIMER_PARKED | 3 |
| EXPECTED_CANCELLED | 0 |
| **total** | **98** = `COVERED_COUNT` |

⇒ **A TOTAL THAT RECONCILES DOES NOT VALIDATE THE PARTITION.** Every fixture was
in the file, so a sloppy extraction that mis-attributed entries between buckets
still summed correctly. The sum was right for a reason independent of whether my
bucket numbers were — which is exactly what makes it a bad check: it agreed with
the ratchet and told me nothing about the thing I thought I was measuring. A
cross-foot is only evidence about the split if a wrong split could break it.

It nearly cost something. When the implementer reported `58 + 34 + 3 + 3 = 98` I
was one step from flagging its 34 as a regression against "my" 38. Checking
which number carried information before correcting either is the same discipline
that saved #90 — and this time the wrong number was mine.

No conclusion changes: `COVERED_COUNT = 98` holds, the buckets are pinned by
exact NAME rather than by count, and the name pins are what the gate asserts.

`COVERED_COUNT` is load-bearing, not decorative — `covered.rs:316` asserts
`names.len() == COVERED_COUNT`, and the buckets are pinned by exact **name**,
not by count. `agreed_refusal_fixtures()` is pinned EMPTY, so a fixture that
both backends refuse cannot enter as a pass.

## Refusal pins: MOVED, not weakened

- `substep_on_failure_route_tail` — class changed `value route payloads` →
  `substeps`, span `513..531 L20` → `323..333 L11`. Legitimate: the payload gate
  it used to hit first is retired, so it now reaches `step::lower_step` and
  refuses at its real remaining gap, exactly as its sibling always has. **Still
  absent from both covered lists** (verified by grep). ✅
- `layer_hygiene` — JOINED the parallel-layer refusal pin rather than the
  covered ratchet. Its `route done(joined)` lowers now, but it also carries
  `step left after root` / `step right after root`, so the payload gate simply
  stopped being the first thing it hit. **Absent from both covered lists.** ✅
- `dev_brief` — unchanged, still `parallel region`.

## Gates I ran myself

```
BC-4 differential   EXIT=0   test result: ok. 27 passed; 0 failed; 0 ignored
clippy -D warnings  EXIT=0   (-p aion-awl -p aion-rs --all-targets)
cargo test -p aion-awl  EXIT=0   508 passed; 0 failed; 0 ignored
```

**Not vacuous** — `covered.rs:312` has a `skip_if_unavailable()` path, so a
green there could have meant nothing. The run printed
`COMPILED 96 module(s) under real 'gleam build' in 1 attempt(s)` (96 = 98 − 2
out-of-oracle), and all seven new fixtures appear in it *by workflow name*
(`value_route_payload → value_route`, `region_close_bounded →
bounded_region_close`, `region_nested_local_reuse → nested_region_locals`,
`region_sibling_local_reuse → sibling_region_locals`, the three `*_hygiene`
verbatim). ⚠️ The fixture-file names return 0 hits — I resolved that mapping
before reading anything into it, rather than treating the zeros as absence.

Ordering note: the differential ran BEFORE a `step.rs` doc-comment-only edit;
clippy and the aion-awl suite ran after. A `//!` comment cannot alter the
differential, and the implementer re-ran it anyway.

## Implementation

`flow_shape_gap` collapses to the `subflow` check alone — the whole payload
walk is **deleted**, not left dormant. `route_payload` refuses no longer and
calls `lower_arg_for`, coercing toward the outcome type as the reference
`render_arg_for` does. A new guard refuses `piped && payload` (the reference's
first gate) rather than silently dropping one of the two. Doc comments in
`step.rs` / `route.rs` / `driver.rs` were corrected to match — the rough-edge
comments are specifications and they now say the true thing.

## Two honest gaps, recorded rather than smoothed over

1. **`temporary_hygiene`'s concat payload is not runtime-proven.** It sits in
   `EXPECTED_FAILED` because `reviewers[0]` faults on the generator's empty
   array before either route is reached. Its lowering is proven at MIR level
   (`value_route_payloads_now_lower`), not against the oracle at run time. The
   implementer disclosed this in the pin comment rather than moving the fixture
   to make the bucket look better.
2. **The optional coercion (`Some` wrap) is pinned by a MIR-term assertion on
   an inline source, not by any differential fixture.** The assertion is
   specific — it pins `v2 = record(some, [v1])` AND
   `v3 = record(done_outcome, [v2])`, deliberately not a pool-presence check,
   because an `Int?` outcome's codec interns `some`/`none` regardless and a
   pool check would pass with the coercion stripped. Sound, but it means no
   covered fixture routes a value to an optional outcome through the oracle.

## 🔴 HOLD — a divergence appeared on the implementer's re-run

Its post-doc-edit differential went RED on `step-bodies/valid/send_signal`:
`disposition=Parked` vs `Failed` on event[0]. **Not one of the seven**, and a
pre-existing covered fixture the landing never touched.

Both readings are live and neither is settled:

- **My run was green** (27/27). It ran mostly alone.
- **Its run was red**, took 80s against my 61s, and overlapped my
  `clippy` + `cargo test -p aion-awl`.
- One green run does not bound a flake's rate, and one red does not establish a
  defect.

🔴 **SELF-CORRECTION, and it is the more useful half.** I first wrote *"I
contended it"* as though that were established. It is not, and Athena's
measurement (2026-08-01, 200 samples over 34 min) is why: on this box the
runnable count exceeded core count in **122 of 200 samples — 61% of the time.**
When a condition holds in the majority of samples, observing an event during a
satisfying sample is very nearly uninformative; there was no plausible outcome
that would have falsified it. My timeline overlap is real, but the box is busy
most of the time regardless, so "my clippy contended it" is a story the data
cannot distinguish from "the box was busy, as usual".

⇒ **BEFORE ATTRIBUTING AN EVENT TO A CONDITION, MEASURE HOW OFTEN THE CONDITION
HOLDS WITHOUT THE EVENT.** A condition that is nearly always true explains
nothing, however carefully measured. This is the sibling of the rare-event
correction I made on #127 — there I invented a mechanism for a difference chance
already explained; here I named a trigger whose base rate makes it undiscriminating.

The attribution that survives is not the trigger but the **mechanism**, which is
proven by construction from the fixture source and the exclusion's own doc —
and which needs no load story at all.

### DIAGNOSED — the exclusion bucket was an enumeration standing in for an invariant

`send_signal.awl` (workflow `gated_item`) contains `wait decision -> answer` —
a **bare signal wait**. `EXCLUDED_OUT_OF_ORACLE` exists for exactly that: its
own doc says such fixtures have no positive signal-park evidence and must not
be *"classified from 120ms of history silence"*. Under load that 120ms window
resolves differently. That is the `Parked` vs `Failed`.

I derived the population from the property rather than trusting the list — and
**got the population wrong in the same way I got the arithmetic wrong.**

I swept "all 98 covered fixtures". I actually swept 95: my sweep enumerated
succeeded + failed + excluded and **missed `EXPECTED_TIMER_PARKED` entirely** —
the same fourth bucket that broke my cross-foot. One root error, two faces.
`step-bodies/valid/wait_and_sleep` lives in that bucket and carries
`wait go -> approval`, so the corpus has **four** bare signal waits, not three.

⇒ 🔴 **I DERIVED A POPULATION FROM AN INCOMPLETE POPULATION** — while writing
up "assert the invariant, not the enumeration". Deriving from a property does
not save you if the set you sweep is itself an enumeration you didn't check.
The property was right; the domain was short by one bucket.

The implementer caught it and refined the property correctly:
**a bare signal wait AND no durable timer anywhere.** `wait_and_sleep` is
legitimately *not* excluded because its `sleep 30s` parks on a real durable
timer — positive evidence, which is exactly what the exclusion's rationale says
these fixtures lack.

| fixture | wait | durable timer? | oracle |
|---|---|---|---|
| header-types/valid/combined | `wait approval -> decision` | no (`timeout 10m` is an **activity** timeout on `node writer`) | EXCLUDED |
| header-types/valid/signal_wait | `wait ruling -> decision` | no | EXCLUDED |
| **step-bodies/valid/send_signal** | `wait decision -> answer` | no | **was LEFT IN** |
| step-bodies/valid/wait_and_sleep | `wait go -> approval` | **yes — `sleep 30s`** | correctly in, timer-parked |

**And my grep would have got this wrong even with the right domain.** `combined`
carries `timeout 10m`, so a text search for a timer clears it — but that is a
per-attempt activity timeout on a worker node, not a durable park. The
derivation must walk the parsed AST, which is what landed. Its statement walk
has no wildcard arm, so a new AST nesting form fails to *compile* rather than
silently hiding a wait.

The bucket was populated by **enumerating the fixtures that happened to exhibit
the problem** (r2 finding 3), not by deriving the population from the property
that causes it. The third was missed and has been a latent load-sensitive flake
in the covered ratchet ever since.

⇒ This seat's own law, fired on this seat's own corpus: **ASSERT THE INVARIANT,
NOT THE ENUMERATION.** The prose was already right ("bare signal-wait
fixtures"); the list was already wrong. A doc comment that describes a class
while the code lists members is a drift that only shows up under load.

**Pre-existing, and independent of this landing** — `send_signal` routes
`route done(item: item_id)`, an Args payload the value-payload change does not
touch.

The fix is NOT to add a third name; that repeats the defect. The fix is a test
that computes the bare-signal-wait population from the fixture sources and
asserts the exclusion list equals it — watched failing first. Sent back to the
implementer on those terms.

## Final gates — run by me, on the final tree, after both fixes

```
BC-4 differential   EXIT=0   test result: ok. 28 passed; 0 failed; 0 ignored
  incl. covered::the_exclusion_list_is_exactly_the_unobservable_park_population ... ok
cargo fmt --all --check              EXIT=0
clippy -p aion-awl -p aion-rs --all-targets -D warnings   EXIT=0
cargo test -p aion-awl               EXIT=0   508 passed; 0 failed; 0 ignored
```

**Controls that make those numbers mean something:**
- `COMPILED 95 module(s) under real gleam build` — was 96 before the exclusion.
  98 covered − 3 excluded = 95. The exclusion demonstrably took effect rather
  than being merely declared, and `send_signal` appears **0 times** in the run.
- **Zero pre-existing goldens modified; all 14 golden files are new.** The 91
  previously-covered fixtures lower bit-identically to before, so the landing
  did not perturb settled output to make room for itself.
- Diff adds **zero** `#[allow]` / `#[expect]` / `#[ignore]` / `#[cfg(any())]`.

### One claim of the implementer's I am NOT accepting as written

It reported *"green under deliberate contention — the flake is retired"*. The
conclusion is true; the evidence cited does not establish it. One loaded run
cannot retire a flake — that is the whole of tonight's lesson, arriving one more
time. What actually retires it is **structural**: `send_signal` is no longer in
the oracle, so it is never compared and cannot diverge. Deterministic, not
probabilistic. The load run is a sanity check, not the proof, and the record
says so.

## Verdict

The value-payload landing is **clean and verified**. The divergence was a
separate, pre-existing harness defect it illuminated, now fixed at the invariant
rather than the enumeration. Neither was deferred; the second was not blamed on
the first.

Every claim above was re-derived at my hand. Three of them were re-derived
*against* me — my bucket arithmetic, my sweep population, and my contention
story were each wrong, and the implementer's numbers beat mine on the first two.
Checking which number carried information before correcting either is the only
reason that went the right way.

— Vesper
