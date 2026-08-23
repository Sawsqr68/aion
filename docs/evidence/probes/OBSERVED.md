# Act-3 probes — the two OPEN questions, settled by the real checker

Plan §4.5 names two questions the builder must settle **before writing the
coordinator**, by real `aion awl check` on throwaway probe documents.
Two more (Q3, Q4) were forced by the answers and are recorded the same way.

Checker: `target/release/aion` built from **this tree** (`aion 0.20.0`).
The `aion` on `PATH` is **0.17.0** — stale against this v0.20.0 tree and
never used for any proof in this flight.

| # | Question | Probe | Exit | Answer |
|---|---|---|---|---|
| Q1 | Is an input-expression `max … visits` bound accepted? | `q1_input_visit_bound.awl` | **0** | **YES** |
| Q2a | Is a `child` call legal in a step carrying `max … visits` + a back-edge route? | `q2_child_in_visit_cycle.awl` | **0** | **YES** |
| Q2b | Does `on failure` bind the child's typed outcome label? | `q2b_failure_label_negative.awl` | **1** | **NO** |
| Q3 | Can a RECORD accumulator thread through bodiless worker actions in a `loop … counting`, with the step routing afterwards? | `q3_record_accumulator_loop.awl` | **0** | **YES** |
| Q4 | Can `run json` interpolate a RECORD param in a worker document? | `q4_record_param_run_json.awl` | **0** | **YES** |

## Q1 — OBSERVED YES. Movement finding M2 confirmed.

```
$ aion awl check docs/evidence/probes/q1_input_visit_bound.awl
ok: docs/evidence/probes/q1_input_visit_bound.awl (1 step)
$ echo $?
0
```

The probe binds `max max_stages visits` where `max_stages` is a flow
**input**. It checks clean.

`workflows/fleet-dev/fleet_dev.awl` (doc-comment paragraph beginning "CHECKS
RUN IN CODE", the sentence "The visit bound is structural (a literal, by
AWL's grammar); making it a dispatch input is named on the row as a
candidate engine extension") is **WRONG on this tree**. The checker's
`check_max_visits` (`crates/aion-awl/src/checker/calls.rs`, fn
`check_max_visits`) refuses only a *runtime* reference, and its helper
`runtime_ref` (same file) treats a name as known when
`w.flow.inputs.contains_key(name) || w.ctx.consts.contains_key(name)` — a
flow input is therefore a legal bound.

`fleet_dev.awl` is byte-frozen by R7. **This is a REPORT, never a fix.**

## Q2 — OBSERVED. Movement finding M4 confirmed in both halves.

(a) legal. (b) the label is **not** observable:

```
$ aion awl check docs/evidence/probes/q2b_failure_label_negative.awl
docs/evidence/probes/q2b_failure_label_negative.awl:18:25: error: expected an expression, found keyword `failure`
```

and the awaited binding is unreachable from the failure arm:

```
error: `staged` is bound on some path but not guaranteed on every path into
this step — bindings are readable only where guaranteed
```

`OnFailure` (`crates/aion-awl/src/ast/steps.rs`, struct `OnFailure`) carries
fields `span`, `lead`, `trailing`, `body` — **no binding for the failed
operation's typed label**.

**Consequence, binding on R4:** an AWL child-await failure edge cannot supply
R4's `cause` ("preserves the child's typed label", design §4). The
coordinator therefore does not use an AWL `child` call for stage dispatch at
all — see Q3/Q4 and the report's R4 section.

## Q3/Q4 — the shape the answers force

Q2b closes the `child`-call route for a coordinator that must report
`child_workflow_id` and a typed `cause`: an AWL `child` call surfaces
neither. (`workflow.id` is the run's OWN id; the corpus carries no accessor
for a child's id.)

Q3 and Q4 prove the shape that does work, and it is the shape the corpus
already flies — `crates/aion-awl/tests/fixtures/rev2/loop-outcomes/valid/loop_counting_until_max.awl`,
step `compose`: `loop draft = Draft(...) counting polish_rounds` threads an
accumulator through the body (`polish(topic: topic, prior: draft) -> draft`),
exits on `until`, and the loop-carrying step routes on its outcome clauses.

The coordinator threads a `ChainState` record through one bodiless worker
action per recorded boundary, so **each boundary is a separate recorded
activity result** (design §6 clauses 2–6), and routes to `done` /
`stage_failed` / `stage_conclusion_refused` / `plan_graph_invalid` from the
loop-carrying step's outcome clauses.

Incidental observations, recorded because they cost a probe each:
* `outcome` is a reserved keyword and cannot be used as a record field name.
* An AWL document must open with `//!` narration before the `workflow` or
  `worker` header.

**These documents are evidence, not product.** They are not deployed, not
referenced by the coordinator, and sit outside the R7 byte-identity
comparison set (design §7 final ¶: new fixtures are expected additions).
