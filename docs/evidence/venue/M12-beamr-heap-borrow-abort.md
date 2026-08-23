# M12 — an intermittent non-unwinding abort in beamr's heap-borrow accessor

**Status: OPEN. Not fixed by this flight, and not caused by it.** Recorded
here because a gate that aborts is a gate that can go red for a reason no
diff explains, and the next round should not have to rediscover this.

## The abort, verbatim

Seen once, during `cargo test --workspace --no-fail-fast`, in the
`aion-server` test binary `update_check_start_binding`:

```
running 4 tests
test a_cli_shaped_null_payload_start_is_refused_by_input_validation ... ok
test an_http_start_with_the_empty_object_input_is_accepted ... ok

thread 'beamr-sched-0' (2630336) panicked at /home/rocketfish/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/beamr-0.20.0/src/term/heap_borrow.rs:79:18:
unsafe precondition(s) violated: slice::from_raw_parts requires the pointer to be aligned and non-null, and the total size of the slice not to exceed `isize::MAX`

This indicates a bug in the program. This Undefined Behavior check is optional, and cannot be relied on for safety.
thread caused non-unwinding panic. aborting.
```

```
error: test failed, to rerun pass `-p aion-server --test update_check_start_binding`
TEST-EXIT=101
```

The abort kills the process, so the binary prints no `test result:` line at
all. A reader scanning for `FAILED` will not find this one — it is only
visible in the harness exit code and the panic text.

## It is not this flight's change

The baseline run of the same suite, taken *before* any edit in this fix
round, ran the same binary green:

```
     Running tests/update_check_start_binding.rs (target/debug/deps/update_check_start_binding-b466a7e66c8f41c8)
running 4 tests
... 4 ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The binary hash `b466a7e66c8f41c8` is **identical** in both runs — the same
compiled artifact passed and then aborted. Nothing in this round's diff
(a test-module helper in `nif_determinism.rs`, an exclusion class in
`awl_corpus_gate.rs`, two markdown changelogs) is linked into a difference
in that binary.

## It is intermittent, and load-dependent

Measured on this venue, same tree, same binary:

| How it was run | Runs | Aborts |
|---|---|---|
| `cargo test --workspace --no-fail-fast` (the gate's command) | 3 | **1** |
| `cargo test -p aion-server --test update_check_start_binding` alone | 12 | 0 |

The three workspace runs were: the pre-edit baseline (pass), the post-edit
verification (**abort**), and a second post-edit verification (pass, all four
tests green in the same binary). So it neither tracks the edit nor
reproduces on demand.

Isolation never triggered it. It surfaced only under the full `--workspace`
run, where many test binaries execute concurrently. That points at a race
rather than a deterministic input: the panicking thread is `beamr-sched-0`, a
scheduler thread, and the failing operation is the one place beamr turns a
raw heap pointer into a slice.

## Where it points

`beamr-0.20.0/src/term/heap_borrow.rs:79` is `HeapBorrow::slice`, whose own
safety comment states the obligation it forwards to callers:

> `ptr` must point at `len` consecutive initialised `T` values inside the
> storage this witness borrows, and that region must stay valid and
> unaliased-for-writes for `'heap`.

and whose sibling `with_frame` requires that `f` "must not allocate on a
process heap, run a collection, or drop a `Heap`".

A violated *alignment/non-null* precondition on a scheduler thread is
consistent with the borrowed region being collected or reallocated while a
witness is live — i.e. the obligation above being broken under concurrency,
not a malformed length.

This is the seam beamr 0.20.0 newly introduced. `CHANGELOG.md` for 0.20.0
describes it as "the accessor-lifetimes release — the heap-backed binary
accessors now take a `HeapBorrow<'heap>` witness", and already records one
known gap in the same area (beamr#36: a `NativeHandler` gets no heap borrow,
so the exception-reason path reads through an ETS-owned copy).

## What is NOT concluded here

Whether the defect is in beamr's accessor or in an aion caller that holds a
witness across an allocation is **not** established by this evidence. One
observation, not reproduced on demand, does not localise a race. Naming a
culprit on this much would be a guess.

## What the next round should do

1. Treat a `GATE-RED: workspace suite` whose log contains
   `non-unwinding panic` + `heap_borrow.rs` as **this** defect, not a new
   one, and do not "fix" it by re-running until green.
2. To reproduce deliberately, run the workspace suite under load repeatedly
   rather than the single binary — isolation does not trigger it.
3. Localising it wants the aion-side callers of the tied accessors under
   `crates/aion/src/runtime/` audited for a witness held across a heap
   allocation, and, if the caller side is clean, an upstream beamr report
   with this transcript.
