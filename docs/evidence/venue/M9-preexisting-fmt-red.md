# M9 — `cargo fmt --all --check` is RED at the rig base, on files this flight never touched

Observed at HEAD `73ef76ff47c5a1661f9fbe87ebae43ad032c8707`, rig base `c590712f57a2fe8353c6cd42823200587f25d70e`.

`.fleet/gate-entry.sh` runs `cargo fmt --all --check` and exits 1 on failure.
On this venue that command fails **before any of this flight's work**, on eight
files under `crates/aion/src/runtime/`.

## Proof: the eight files are byte-identical at rig base, HEAD, and the working tree

| File | blob at rig base | rig base = HEAD = worktree |
|---|---|---|
| `crates/aion/src/runtime/nif_activity_dispatch.rs` | `b4646c57f5cf` | **IDENTICAL** |
| `crates/aion/src/runtime/nif_concurrency.rs` | `c85bb7b910fe` | **IDENTICAL** |
| `crates/aion/src/runtime/nif_continue_as_new.rs` | `9ff92f656fd3` | **IDENTICAL** |
| `crates/aion/src/runtime/nif_determinism.rs` | `0065ad9c9203` | **IDENTICAL** |
| `crates/aion/src/runtime/nif_query.rs` | `7cd1d864e119` | **IDENTICAL** |
| `crates/aion/src/runtime/nif_timeout.rs` | `22a35dfa902e` | **IDENTICAL** |
| `crates/aion/src/runtime/nif_timer.rs` | `95dbe58b22d6` | **IDENTICAL** |
| `crates/aion/src/runtime/payload.rs` | `6fa0bde9c211` | **IDENTICAL** |

```
$ git status --porcelain crates/aion/
(empty — this flight modified nothing under crates/aion/)
```

## Cause

Toolchain drift, not a code defect. This box carries:

```
rustfmt 1.9.0-stable (8bab26f4f6 2026-07-14)
rustc 1.94.0 (4a4ef493e 2026-03-02) (Arch Linux rust 1:1.94.0-3)
```

A representative diff rustfmt wants — an import-ordering change, i.e. a
style-edition difference, not a formatting error in the source:

```
Diff in /home/rocketfish/fleet/runs/seqlegs-impl-flight1/legs/build/crates/aion/src/runtime/nif_activity_dispatch.rs:12:
 use crate::runtime::nif_result_term::{NifRefusal, error_result_term, ok_result_term};
 use aion_core::ActivityId;
 use beamr::native::ProcessContext;
-use beamr::term::heap_borrow::HeapBorrow;
 use beamr::term::Term;
+use beamr::term::heap_borrow::HeapBorrow;
 
 /// NIF backing `aion_flow_ffi:dispatch_activity/3`.
 pub(super) fn dispatch_activity_impl(
Diff in /home/rocketfish/fleet/runs/seqlegs-impl-flight1/legs/build/crates/aion/src/runtime/nif_activity_dispatch.rs:131:
     await_activity_result_with_context(&state, context, &runtime.runtime, ctx, &correlation)
```

## Ruling

**Superseded by the round-1 fix.** The original ruling below was that this
red was "not this flight's to fix". The gate disagrees, and the gate is the
wall: `.fleet/gate-entry.sh` runs `cargo fmt --all --check` unconditionally
over the whole workspace, so a stale file anywhere is this flight's red no
matter who wrote it. Declining to run the formatter is a workaround, not a
report — the CAUSE is eight files that predate edition-2024 rustfmt's
version-sorted import ordering, and the fix for that cause is to run the
formatter.

Round 1 therefore ran `cargo fmt --all`. Exactly the eight files above
changed; nothing else in the workspace moved.

The alternative "fix" — pinning `style_edition` in a new `rustfmt.toml` to
make the old ordering valid again — was rejected. It suppresses the drift
rather than resolving it, and it would silently change the formatting
contract for every crate in the workspace including this flight's own.

## The change is formatting-only, proved not asserted

Per-character multiset delta, old blob vs new, for each of the eight files:

```
$ for f in $(git diff --name-only); do
    diff <(git show HEAD:$f | tr -d '[:space:]' | fold -w1 | sort | uniq -c) \
         <(cat $f            | tr -d '[:space:]' | fold -w1 | sort | uniq -c)
  done

nif_concurrency.rs        (no delta)
nif_continue_as_new.rs    (no delta)
nif_timeout.rs            (no delta)
payload.rs                (no delta)
nif_activity_dispatch.rs  ,  233 -> 234
nif_query.rs              ,  273 -> 274
nif_determinism.rs        ,  549 -> 563
nif_timer.rs              ,  409 -> 411   {  155 -> 156   }  155 -> 156
```

Every delta is a comma rustfmt adds when it reflows a call across lines.
The single `{`/`}` pair in `nif_timer.rs` is one match arm whose body no
longer fits on one line at the deeper indent `timer_call`'s reflowed
closure puts it at:

```rust
-            ResolveOutcome::Recorded(Resolution::TimerCancelled) => Ok(error_result("cancelled")),
+                ResolveOutcome::Recorded(Resolution::TimerCancelled) => {
+                    Ok(error_result("cancelled"))
+                }
```

No token was added, removed, or reordered other than trailing commas and
that block wrap. Nothing under `crates/aion/src/runtime/` changed behavior,
so the R7 byte-identity obligation (§7) is untouched: this seam does not
participate in `aion awl emit` output, and the design's only `fmt`
requirement (§504) is about `aion awl fmt` on AWL documents, not `cargo
fmt` on Rust source.

This flight's own crate was, and remains, fmt-clean:

```
$ cargo fmt -p aion-fleet-plan -- --check
(no output: clean)
```

## Round 2: the fmt fix moved a test past clippy's line budget

Running the formatter cleared `GATE-RED: fmt` and exposed the next red in the
same causal chain:

```
error: this function has too many lines (107/100)
   --> crates/aion/src/runtime/nif_determinism.rs:734:5
    |
734 |     fn live_and_replayed_activity_serve_identical_now_sequences() -> TestResult {
    |
    = note: `-D clippy::too-many-lines` implied by `-D clippy::pedantic`
```

**Cause, not coincidence.** By clippy's own counting rule that test measured
**71** before round 1. It calls
`decode_result_tuple(now_from_context(&mut ctx, &c), ctx.borrow_terms())?.1`
six times, and each of those one-liners overran the 100-column width once
rustfmt reflowed it, becoming a seven-line block:

```rust
        live_now.push(
            decode_result_tuple(
                now_from_context(&mut ctx, &live.context),
                ctx.borrow_terms(),
            )?
            .1,
        );
```

Six sites x six added lines = 36, and 71 + 36 = **107**, the figure the gate
reported. The formatter did not add code; it made an already-repeated
expression cost seven times as much vertical space.

(Clippy counts a function's non-blank, non-comment span lines minus the two
brace lines. That rule was calibrated against nine functions clippy reported
itself, by re-running it over this workspace with
`too-many-lines-threshold = 10`; it reproduces clippy's number exactly in
every case. The probe `clippy.toml` was removed afterwards and is not part of
this change.)

**Fix: name the repetition.** The six blocks collapse onto one helper beside
`decode_result_tuple`, which is where the identical logic in `now_through_nif`
already lived:

```rust
    fn now_value(ctx: &mut ProcessContext, context: &NifContext) -> TestResult<String> {
        let (tag, value) = decode_result_tuple(now_from_context(ctx, context), ctx.borrow_terms())?;
        if tag != "ok" {
            return Err(format!("now returned an error: {value}").into());
        }
        Ok(value)
    }
```

Each site becomes `live_now.push(now_value(&mut ctx, &live.context)?);`. The
test drops back to **71** — exactly its pre-round-1 count, because the fix
restores the density the reflow cost rather than deleting anything.

**What was NOT done.** No `#[allow(clippy::too_many_lines)]`, in any spelling —
the lint was reporting a real duplication and the duplication is gone. No
assertion was removed, weakened, or split across functions: the test still
observes `now` at the same six positions in the same order, still compares the
full live sequence against the full replay sequence, and still pins the tail at
`1700000000900`. The helper is strictly stricter than the code it replaces —
the old sites took `.1` off the tuple and discarded the tag, so an `{error, _}`
reply would have been compared as if it were a timestamp; `now_value` fails
instead.

Nothing under `crates/aion/src/runtime/` changed behavior: the edit is confined
to `mod tests`.

With `fmt` and `clippy` green the gate reached `cargo test --workspace` for
the first time in this flight. The two reds it found there are recorded
separately: `M10-corpus-gate-vs-negative-probe.md` (this flight's own) and
`M11-changelog-embed-drift.md` (pre-existing at the rig base, fixed under the
same "the gate is the wall" ruling this document establishes).

---

## Original ruling (round 0, superseded)

**Not this flight's to fix, and deliberately not fixed.** Reformatting
`crates/aion/src/runtime/` would touch a seam this brief did not ask for,
would pollute the diff the gate measures, and is outside the scope walls
(the brief's NOT-IN-SCOPE list plus "do not silently widen the engine").

This flight's own crate IS fmt-clean:

```
$ cargo fmt -p aion-fleet-plan -- --check
(no output: clean)
```

Consequence for the dispatch gate: `.fleet/gate-entry.sh` will report
`GATE-RED: fmt` on this venue for a reason that predates the leg. That is a
venue fact of the same kind as the plan's M7 (`deliverable_absent` requires
a `^crates/.*\.rs$` path), and it is REPORTED here rather than worked around.
