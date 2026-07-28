# Red-then-green evidence — build/on-failure-lowering

Lane 1 of the ruled enforcement chain (RUNTIME-OPERATIONS R5 landing ruling,
recorded in `AWL-COMPLETION-COLD-READ.md` §2.3's margin): `on failure` —
parsed, printed, checked, and genuinely lowered by the reference (Gleam)
backend — stops being refused by the direct/MIR backend for **top-level
steps**. The checker edge rule is lane 2's and is NOT built here.

Toolchain stamped on `gate-logs/on-failure-lowering-stamp.log` and here:
`rustc 1.97.1 (8bab26f4f 2026-07-14)`, `cargo 1.97.1 (c980f4866 2026-06-30)`,
`jq-1.8.1`, `gleam 1.17.0` (Erlang/OTP 29 box). No `rust-toolchain.toml`
edit. Default target dirs. `df` preflight recorded in the same stamp file
(181Gi free at the battery).

## The ruling this builds

D1 (Waffles' key, 2026-07-28, tear PASS): **design B — If-cascade plus a
lifted compensation `FlowFn`. `Stmt::Attempt` stays unconstructed.**
D3: **the edge binds nothing.** Rider 1: the `Attempt` row is amended in-lane
and any future constructor of it needs a NEW ruling.

## Where the meaning lives

Every fallible statement in AWL already lowers to one `Stmt::TryBind` over its
`Result`. Under a guard, `failure::guard` rewrites each of them:

```
crates/aion-awl/src/mir/lower/failure.rs
    TryBind { dst, result }
  becomes
    Tail::If {
      test: Test::IsTagged { value: result, tag: ok, arity: 2 },
      then: [ FieldGet { dst, base: result, index: 1 } , …rest of the step… ],
      else: Tail::TailLocal { callee: <compensation>, args: <step avail_in> },
    }
```

`Test::IsTagged{ok,2}`, `FieldGet` at index 1, `Tail::If`, `Tail::TailLocal` —
every one of them is already live and proven in `mir/lower/fanout.rs`'s
tolerant collect. **Zero new instruction selection**, which is why the
reserved `Stmt::Attempt` never had to be built: BC-4 compares durable trails,
not bytes, so the reference-parity burst was work the differential could not
observe. `AWL-BC-IR.md` §2.5's `Attempt` row now records that ruling, and
states that any future CONSTRUCTOR of the op requires a new ruling.

The compensation function's parameters ARE the guarded step's `avail_in`,
resolved from the scope **after** a `max … visits` prologue and **before** any
body statement rebinds a name — exactly the scope the reference clones into
its `Error(_)` arm (`emitter/failure.rs:107`). No captures, no closure, no
defs tuple. A later reason-binding arrives as one more parameter.

### The two invariants are structural, not checked

1. **A body-terminal route can never compensate.** A route lowers to a `Tail`,
   never to a `TryBind`, so it cannot pass through the `IsTagged` test. The
   hazard is unrepresentable, not merely avoided. Pinned by the golden of
   `flow-shape/valid/on_failure_route_tail`, whose deliberate
   `route dead_end(...)` renders inside the SUCCESS arm:

```
== fn step_publish_it/1 origin=region(publish_it) ==
  v4 = call_rt aion@awl@error:map_activity_error/1(v3) live_after=[v0]
  if is_tagged v4 ok/2:
    v5 = field(v4, 1)
    …
    v9 = record(awl_outcome_failure, [lit#13, v8])
    v10 = record(error, [v9])
    return v10                       <- the routed failure outcome
  else:
    tail_local publish_it_on_failure(v0)
```

   That structural claim is also EXECUTED, not merely read off the golden:
   `compensation::a_routed_failure_outcome_never_compensates` runs the fixture
   on a real engine, on both productions, and asserts the trail carries
   `publish`, carries NO `ActivityFailed`, and does NOT carry `snapshot` — the
   action only the `on failure` block calls.

2. **The edge binds nothing** — stated in `failure.rs`'s module doc with the
   §2.3 citation, and true of the reference (`Error(_)`), the AST (`OnFailure`
   has no binding slot) and the checker's `avail_in` law alike.

### Slot inventory

One reserved compensation slot per guarded step, after every wait slot and
before the implicit-child adapter shells (`lower/plan_slots.rs` item 6). A
module with no `on failure` reserves none, so every pre-edge `FnRef` is
byte-identical. Measured, not asserted:

```
git diff --stat edc3c3af..HEAD -- crates/aion-awl/tests/mir-goldens
  12 files changed, 1913 insertions(+)
```

Twelve files, all NEW (six fixtures × `.mir` + `.gleam_types.hex`); not one
pre-existing golden byte changed.

### The rewrite proves itself complete

The cascade reaches the guarded step's own straight-line burst and everything
the recursion carries into the success arms. A `TryBind` left anywhere else —
inside a tail's arms, or inside a `ShortCircuit`'s conditionally-evaluated
right-hand burst (`lower/expr.rs:266` documents that one can live there) —
would return its `Error` straight out of the step function while the
reference, whose whole body sits inside the attempt closure, would have caught
it and compensated. `failure::guard` therefore walks the BUILT block for any
residual `TryBind` and refuses honestly if one survives. The check is over the
block, not over a list of shapes, so a future lowering that starts producing
fallible results in a new position is caught by construction rather than by
someone remembering to extend a list. No covered fixture trips it.

### Compensation-body inventory

Compensation bodies are lowered OUTSIDE the shared inventory traversal
(`plan_slots::for_each_step` walks `step.body` only), so a construct that
would consume a reserved slot inside an `on failure` block — loop, fork,
timeout wait, child spawn, substep, per-item region — refuses honestly instead
of taking an uncounted slot and misaligning every later ref. Visible
incompleteness, the same discipline substeps already use; extending the
inventories to descend into compensation bodies is the increment that retires
it.

## Red

Committed as `26e40029`, before any lowering existed. The four top-level
`on failure` fixtures joined `mir/covered.rs`; three sites went red, all
exit **101**:

`gate-logs/on-failure-lowering-red-awl.{log,exit}`:

```
mir::tests::lowers_cover_and_golden
  assertion `left == right` failed: BC-2 covered set drifted from the pinned
  COVERED list
  (the four are pinned; the lowering still refuses them)

mir::select::inspect_tests::covered_ratchet_upholds_ir14_and_section_11
  the sweep inspected 77 fixtures but the pinned ratchet has 81 — silent
  covered→refused drift (BC-5 review advisory B)
```

`gate-logs/on-failure-lowering-red-bc4.{log,exit}`:

```
covered::covered_set_is_byte_identical
  assertion `left == right` failed: covered ratchet size drifted (expected 77)
    left: 81
   right: 77
```

## Green

### Coverage

`COVERED` 77 → 83 in both ratchets (`aion-awl/src/mir/covered.rs` and
`aion/tests/awl_bc4_differential/covered.rs::COVERED_COUNT`), with blessed MIR
+ sidecar goldens for every newcomer. Three route-target shapes proven:

| fixture | compensation routes to | differential bucket |
| --- | --- | --- |
| `loop-outcomes/valid/on_failure_compensation` | workflow failure outcome | FAILED |
| `loop-outcomes/valid/nested_handlers` | workflow failure outcome | SUCCEEDED |
| `loop-outcomes/valid/ship_release_combined` | sibling step `cleanup` | FAILED |
| `flow-shape/valid/on_failure_route_tail` | workflow failure outcome | FAILED |
| `flow-shape/valid/on_failure_compensates_to_outcome` | workflow failure outcome | FAILED |
| `flow-shape/valid/on_failure_compensates_to_step` | sibling step `unwind` | SUCCEEDED |

### The arm is EXECUTED, not merely lowered

Every activity in the BC-4 corpus was a canned success, so no compensation
block had ever RUN. Failure induction, ruled NOT optional:
`fixtures::action_results` answers an action named `fail_<something>` with a
terminal `Err` instead of a canned success. The convention is documented at
its definition site, and the map is handed to the reference run and the direct
run alike by `driver::differential_one`, so it is **both-runs-by-construction**
and cannot manufacture a divergence. The dispatcher's map value type is now
`ActionResult = Result<String, String>`, so the failure is explicit at the
type level rather than a magic value.

`awl_bc4_differential/compensation.rs` then reads the durable trail for the
compensation block's OWN activity — an action that appears nowhere else in the
fixture, so nothing but an executed arm can put it there — on BOTH productions
of the same workflow:

```
compensation::compensation_arms_execute_on_both_backends ... ok
```

A bucket alone could not carry this: a failed activity that propagated
straight out and a failed activity whose compensation ran and routed a failure
outcome both end in `WorkflowFailed`. `on_failure_compensates_to_step` is the
sharper half — a COMPLETED run reachable ONLY through the failure edge.

### Replay

The same test replays each compensated history through
`aion::durability::replay_inspect::inspect_run` and asserts
`inspection.divergence.is_none()`. Proven, not asserted: the arm is ordinary
workflow code after an `ActivityFailed` that was recorded BEFORE the failure
value reached the program, the same class as tolerant collect.

### Tripwires

All five repointed, none left red across a commit boundary:

| # | site | disposition |
| --- | --- | --- |
| 1 | `mir/deferred_tests.rs::bc4_nested_handlers_refuses_on_failure` | → `nested_handlers_on_failure_now_lowers` (lower + verify pin) |
| 2 | `mir/deferred_tests.rs::combined_loop_fixtures_advance_to_the_next_refusal` | → `dev_brief_still_refuses_at_its_parallel_layer`; `ship_release_combined` retired into the ratchet |
| 3 | `awl_bc4_differential/adversarial.rs` | `nested_handlers` REFUSING → LOWERING (seven lower, one refuses) |
| 4 | `aion-awl/tests/compile.rs::refusal_passthrough_is_lossless` | repointed at `loop-outcomes/valid/substeps_two_stage` |
| 5 | `aion-server/tests/awl_deploy_direct_e2e.rs::LOWER_REFUSED` | repointed from an `on failure` body to a SUBSTEP body |

The exact-span pinning intent of tripwire 1 survives as a new test,
`deferred_tests::substep_fixtures_still_refuse`, over BOTH substep fixtures —
and it records that they refuse at two DIFFERENT gates:

- `flow-shape/valid/substep_on_failure_route_tail` carries a value route
  payload, so the driver's staging gate takes it first
  (`Unsupported { shape: "value route payloads …", span: 513..531 @ 20:43 }`);
- `loop-outcomes/valid/substeps_two_stage` reaches `step::lower_step` and
  refuses at `substeps` (`span: 503..510 @ 18:6`).

A future substep lowering must retire both knowingly.

## Findings recorded, not papered over

**1. A declared type that shadows a Gleam builtin breaks the REFERENCE
backend.** `nested_handlers` declared `type Result { order_id: String, ok:
Bool }`. It had never been `gleam build`-ed — it was an out-of-intersection
refusal, so `classify` never handed it to the reference builder. The moment it
joined the built corpus the reference emission failed to compile:

```
error: Type used as a type constructor
  reference_adversarial/src/nested_handlers.gleam:47:41
47 │ pub fn run(raw_input: Dynamic) -> Result(String, String) {
`Result` is a type with no parameters, but here it's being used as a type
constructor.
```

The fixture's type is renamed to `Shipment` to unblock coverage. The defect is
REAL and untouched by this lane: neither the checker nor the reference emitter
refuses or renames a declared type that collides with a Gleam builtin, so any
author writing `type Result` (or `Option`, `Nil`, …) gets an uncompilable
reference emission with a Gleam-level error. It belongs to whoever owns the
emitter's name hygiene.

**2. Brief-vs-tree drift, resolved at the tree's bytes.** The brief's scout
facts were taken on main `28eb1eec`; this lane cut from `edc3c3af`, which
carries the landed R5 advisory-class and R4 retry-counting lanes.

- `COVERED_COUNT` was already 77, not the brief's 76 (R5 added
  `advisory_side_channel` to both ratchets). 77 → 83 here, not 76 → 79.
- The brief named three fixtures joining the ratchet. Four pre-existing ones
  do: `flow-shape/valid/on_failure_route_tail` is a top-level `on failure` too
  and `lowers_cover_and_golden` compares the covered set EXACTLY, so it could
  not have been left out. It is also the fixture the Acceptance list's
  body-terminal-route invariant needs, so its inclusion is the brief's own
  intent.
- The two arm-executing fixtures are net-new files, taking the count to 83.
- `flow.rs` was 540 lines as the brief said; `mir/lower/fanout.rs` sits at
  EXACTLY 500 and this lane did not add a line to it (rider 2).

## The battery

At the committed tree, with HARNESS R2 on the clippy leg and the estate lead's
RAIL 6 amendment on the tests leg (standing-law `cargo test`, not nextest —
aion's server-booting e2e suites depend on cargo's binary-level
serialization).

| leg | command | exit | verdict |
| --- | --- | --- | --- |
| fmt | `cargo fmt --all` | 0 | applied, tree clean |
| clippy | `cargo clippy --workspace --all-targets --message-format=json` | **0** | see below |
| tests | `cargo test --workspace --no-fail-fast` | **0** | see below |

Battery run at the committed tree `bf4181ca` (`git rev-parse HEAD` recorded in
the stamp file and in the clippy verdict artifact). Two earlier runs were
discarded rather than cited across trees: the first when a source change
landed mid-run, the second when the executed body-terminal-route pin was
added.

`gate-logs/on-failure-lowering-clippy.json` is the raw JSON stream;
`…clippy.verdict.txt` is the jq extraction beside it:

```
DENOMINATOR compiler-artifact count: 804
error   diagnostics: 0
warning diagnostics: 0
```

The empty diagnostic extraction is green against a **proven nonzero
denominator of 804 compiler artifacts from the same stream** — the stream was
not empty, and the absence of findings is a real absence.

`gate-logs/on-failure-lowering-tests.{log,exit}` carries the workspace suite;
its own `test result:` lines are the verdict, summed over all 220 of them:

```
exit 0 — 220 suites, 3022 passed, 0 failed, 6 ignored
```

The 6 ignored are the pre-existing, individually-reasoned OS-process and
network suites (`osproc_*`, the hex.pm-downloading dev-pipeline template,
`stacked_dev_lands_through_the_real_worker_and_review_signal`); this lane
neither added nor changed an `#[ignore]`.

Both ratchet censuses are EQUALITY checks inside the suite
(`COVERED_COUNT == 83`, and `lowers_cover_and_golden`'s exact set comparison),
not `>=`.

### RAIL 6 (estate lead, at dispatch)

The tests leg is the standing-law `cargo test --workspace --no-fail-fast`, NOT
nextest: aion has server-booting e2e suites that depend on cargo's
binary-level serialization, and a naive nextest leg reds them on pure
contention. HARNESS R2's JSON + jq verdict layer applies to the clippy leg
only, which is where it is used above.
