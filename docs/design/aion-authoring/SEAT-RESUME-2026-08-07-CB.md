# Seat marker CB — 2026-08-07

Supersedes CA for §2 (Finding B) only. **Everything else in CA still stands** — read CA
for Findings A/C/D/E/F, the beamr 0.17.0 move, standing constraints, and the BC-4
attribution table.

## §1 🔴 FINDING B IS CLOSED — AND CA §2's DIAGNOSIS WAS WRONG

CA §2 concluded, from three process shapes in a bare harness, that "a workflow process
does not survive a NIF call that fails, whatever the NIF, and Erlang `try…catch` does not
intercept it", and that this might make Review 18's F2 premise FALSE. **That conclusion is
refuted.** The premise stands and the `cancel_pid` gate is load-bearing.

### What actually killed those processes: the harness never installed the NIFs

Seven process shapes in ONE harness, measured (all with `RuntimeHandle::new` only):

| shape | live? |
|---|---|
| `park_only` — no call at all | ALIVE |
| `try_only` — `try ok of _ -> ok catch _:_ -> ok end` | ALIVE |
| `once_bif_ok` — `erlang:atom_to_list(ok)` | ALIVE |
| `once_bif_raises` — `erlang:atom_to_list(<<"x">>)`, badarg, inside try/catch | **ALIVE** |
| `once_local_raises` — `erlang:error(deliberate)` in a local fn, inside try/catch | **ALIVE** |
| `once_undef` — `no_such_module_at_all:nope()` inside try/catch | DEAD |
| `once_failing` / `once_pure` / `once_bare` — any `aion_flow_ffi:*` | DEAD |

**`try…catch` WORKS.** Raised exceptions are caught and the process lives. What kills is an
unresolvable call — and `aion_flow_ffi:*` behaved *identically to a module that does not
exist*, because it WAS one in that harness.

🔴 **CA §2 ruled undef out on a code reading, and the code reading was wrong.**
`RuntimeHandle::new` (`handle.rs:143`) calls `register_all_bifs` — beamr's BIFs. The
`aion_flow_ffi` table comes from `install_nifs(NifRegistration::new().add_engine_nifs())`
(`nif.rs:118`), which nothing in that harness called. An inference from reading the
constructor stood in for a measurement, and it survived because every alternative
hypothesis it generated (heap exhaustion, dirty schedulers, try/catch) was also false.

**With `install_nifs` in place a workflow process SURVIVES a refused `continue_as_new`.**
That is exactly the condition that makes the gate load-bearing rather than decorative.

### A second wrong reading, caught before it was filed

Mid-diagnosis I measured that adding a `park_only` process made the treatment stop dying,
and that raising `thread_count` 2→3→4→8 did not help, and concluded that
**`receive … after N` does not fire while another process sits in `receive`** — a beamr
scheduler defect I was about to report to Artemis.

**It is not a defect.** The waiting fixture was spinning 400 000 times on a NIF that
failed (see below), which is CPU-bound; that is what starved the timer. With the harness
correct, the timer-based waiter synchronises in **27 ms** with a parked process present.
Nothing to report. A near-miss worth keeping: a starvation symptom attributed to the
*other* process, when the cause was the measuring process's own load.

### The third thing the harness was missing

`aion_flow_ffi:workflow_id/0` resolves through `state.context_source`, a **separate**
installation from `runtime_context` — `install_nif_context_source` (`nif_determinism.rs:63`),
which `builder_assembly.rs:57` does and my harness did not. Without it the fixture's
"has the test finished arranging the world?" poll could never succeed, so the fixture only
ever called after its retry budget expired. **That made the wait a disguised sleep**, and a
first "it works" reading was that sleep landing luckily. It was caught by making budget
exhaustion PARK instead of call: an unsynchronised run then measures nothing and fails
loudly. With the context source installed, the poll resolves in **27–30 ms**.

### Measured, with the harness correct (all four cells)

| epoch | waiter | parked control | outcome |
|---|---|---|---|
| OPEN | spin | present | terminal recorded, process DEAD, 30 ms |
| CLOSED | spin | present | history UNMOVED, process DEAD, 27 ms |
| OPEN | timer | present | terminal recorded, process DEAD, 27 ms |
| CLOSED | timer | present | history UNMOVED, process DEAD, 27 ms |

`park_only` ALIVE in every one.

## §2 WHAT IS BUILT (uncommitted)

- `crates/aion/tests/fixtures/aion_continue_fixture.erl` + `.beam` — `park_only/0` and
  `await_then_continue/0`. `wait_registered(0)` PARKS rather than calling, so a run that
  never synchronised fails loudly instead of measuring a disguised sleep.
- `crates/aion/src/store_faults.rs` — `unspent_append_failures()`, the witness that the
  injected fault was actually SPENT **by the durable half** (see §4 for why appends and
  not reads).
- `crates/aion/src/runtime/nif_continue_as_new_tests.rs` —
  `the_nif_gate_ends_a_live_process_for_the_epoch_refusal_and_spares_the_others`:
  treatment (closed epoch → process ends, history unmoved), CONTROL 1 (`park_only`
  survives the same runtime), CONTROL 2 (an append failure raised BEFORE the terminal →
  predicate spares it → process survives, fault proven spent). Plus
  `install_determinism_context`, which documents at the call site that the determinism
  context source is a SECOND installation and that a harness missing it leaves the pure
  NIF answering `{error, …}` forever.
- Module doc rewritten: it no longer claims the NIF gate is measured by nothing.

## §3 WHAT IS STILL OWED ON THIS LANE

Everything in §1/§2/§4/§5 is DONE. Remaining:

1. **Review 20** (Opus sub-agent, per CLAUDE.md) over the whole uncommitted diff.
2. Only when it clears: push `9d259b185`, `88b010961`, `610ae17f7`.
3. Then CA §4's carried items — beamr → published **0.17.0** (manifest edit, NOT a
   rev-pin), re-cut the differential pin for Artemis, judge the stray `beamr = "0.6.1"`.

## §4 THE TEST IS MUTATION-PROVEN — INCLUDING THE REVIEWER'S OWN MUTATION

⚠️ **Superseded by the Review-20 fix pass:** the live test was restructured (findings
M6/M7/M8) and the source file changed (B2/B3), so the runs below are against bytes that
no longer exist. All three gate mutations were RE-RUN against the restructured test and
are RED again — plus the three predicate-arm mutations (m15) — recorded with the new
pinned baseline in `REVIEW-20-FINDINGS.md`. This section stands as the record of the
first proof, not the current one.

Baseline `nif_continue_as_new.rs` shasum `90484d0f58981ea7f70982a6c7fc947fafa6d45a`,
restored byte-identical after every run (verified each time, not once at the end).
Target text `if outcome_must_end_the_process(&recorded) {`, asserted present exactly once
by `scratchpad/mutate.py` — a target that is not found is a NON-RUN, not a survivor.

| # | mutation | result | died at |
|---|---|---|---|
| M5 | `if recorded.is_ok()` — **the reviewer's exact pre-remediation form** | **RED** | the treatment |
| M6 | `if false` — gate never fires | **RED** | the treatment |
| M7 | `if true` — gate always fires | **RED** | **CONTROL 2, uniquely** |

M5 is the one that matters: it is the mutation that previously left the whole suite green,
which is what Finding B WAS. It is now red.

### One more control defect caught and fixed while building this

CONTROL 2's first draft injected a **read** failure and the control died anyway. The
witness (`unspent_read_failures() == 0`) fired — for the wrong caller. The fixture's own
registration poll spends the read budget long before the durable call is made, so the call
under test ran against a healthy store, recorded its terminal, and was correctly killed
for it. **Only the durable half appends**, so the budget was moved to appends
(`unspent_append_failures`). A witness has to be spendable by nothing but the thing it
witnesses.

## §5 GATES AT THIS CHECKPOINT — CLAIMED "ALL GREEN", AND THAT CLAIM WAS OVER A SUBSET

🔴 **Correction (Review 20 B1, same day):** this table as first written omitted the
`cargo fmt` gate entirely — and when Review 20 ran `cargo fmt --check`, it FAILED with
exit 1 (five diffs, all in `nif_continue_as_new_tests.rs`). "ALL GREEN" was a claim over
a proper subset of CLAUDE.md's named gates. The current gate truth for this lane,
including the fmt row and the post-fix re-runs of everything below, lives in
`REVIEW-20-FINDINGS.md` ("GATES AT THE FIX-PASS CHECKPOINT"); this table stands only as
the record of what was claimed at this checkpoint.

| gate | result at this checkpoint |
|---|---|
| `cargo fmt --check` | **NOT RUN — the omission is the B1 finding** (was exit 1) |
| `cargo clippy -p aion-rs --all-targets -- -D warnings` | **EXIT 0** |
| `cargo clippy --workspace --all-targets -- -D warnings` | **EXIT 0**, 0 warning/error lines |
| `cargo test -p aion-rs` (to a FILE) | **EXIT 0** — 60 verdict lines, **1130 passed / 0 failed / 0 ignored** |

Verdict lines reconciled against binaries: 58 `Running` + 1 `Doc-tests` = 59, and the
doc-test runner emits **two** verdict blocks (merged doctests, then the compile-fail
doctest), which is the whole of the 60-vs-59 gap. No binary is missing and none crashed
without a verdict.

🔴 **CA §8's suite figure was wrong and is corrected here.** It recorded "9 verdict lines,
758 passed / 1 failed"; the real shape of this suite is 60 lines / 1130 tests. That number
came from a `| tail`-truncated read, which is the exact defect my own notes warn about —
suite output goes to a FILE, then the file is counted.

**The BC-4 differential ran GREEN this time** (29 passed / 0 failed), which is the fourth
outcome in four runs of #149's nondeterministic target and is consistent with CA §8's
conclusion. #149 stays open; it is not this lane's.
