# A `gleam` failure gets blamed on the author (#125)

**Found by run 3 on 2026-08-01, not by reasoning.** Athena caught a red line in a
workspace battery; the diagnosis walked one layer at a time from a test failure
to a server-side error type that states something untrue.

## The trigger

```
crates/aion-cli/tests/authoring_quality_e2e.rs:188  FAILED
  `aion generate` failed before the codec round-trip:
  error: interface export failed: `gleam export package-interface` exited with status 1
    Resolving versions
    error: HTTP error — error sending request for url
    (https://hex.pm/api/packages/gleam_stdlib/releases/1.0.3)
```

Transient, established **four independent ways**, none of which depends on
believing the others:

```
run 2  line 1157  ok        complete run, WORKSPACE_EXIT=0
run 3  line 1157  FAILED    error: HTTP error, hex.pm
       single-test re-run:  ok, bracketed by 4x HTTP 200 on the failing URL
       independent probes:  200 x5 across 21:20-21:23Z (Athena)
run 4  line 1664  ok        COLD full-workspace run, no special-casing
```

🔴 **The fourth one is not redundant, and it closes a confound I did not name.**
My probe-run-probe re-ran the test ALONE — one test, warm cache, no contention.
A pass under those conditions is consistent with "the network recovered" *and*
with "this test only fails under load or from cold". **An isolated re-run can
pass BECAUSE it is isolated.** Only the cold full-workspace green rules that out.
(Athena, 2026-08-01.)

⇒ Generalises: **when you re-run a failure in a narrower context than it failed
in, a pass has two explanations and you have chosen between them by assumption.**
The confirming run has to be at least as wide as the failing one.

## 🔴 The rule that makes this a defect and not a wording nit

`crates/aion-cli/tests/test_support/gleam.rs:22` states its polarity on purpose:

> "an unrecognised failure is classified as a code defect — **the safe polarity
> is 'blame the code, make a human look'**."

**That is correct in a test harness and inverted in shipped code.** The rule is
identical; the audience is not:

- harness → "blame the code" sends a **maintainer** to code they own and can
  fix. Safe.
- `aion generate` / the studio → "blame the code" sends an **author** to rewrite
  **working Gleam** over an outage they cannot fix, and their source was never
  wrong.

> **A diagnostic's safe default is a property of who RECEIVES it, not of the
> failure.** (Athena, 2026-08-01.)

⇒ Share the classification; **never share the default.**

## The measured surface — four sites, and the worst one is server-side

`Command::new("gleam")` in production code, plus the in-`src` test gate:

| # | site | what it says on a hex.pm outage | severity |
|---|---|---|---|
| 1 | `aion-toolchain/src/compile.rs:211` | `ToolchainError::TypeCheck { diagnostics }` | 🔴 **the error TYPE is the lie** |
| 2 | `aion-cli/src/generate.rs:345` | "the types module `src/<pkg>_io.gleam` … **must compile standalone**" | 🔴 accuses a named file |
| 3 | `aion-cli/src/package.rs:91` | "`gleam build` failed in <path> with <status>" | neutral, but unclassified + no retry |
| 4 | `aion/src/engine/builder.rs:883` | `println!("skipping Gleam-dependent test…")` | pre-#74 pattern, invisible skip |

**#1 is the worst and it is not about wording.** `ToolchainError::TypeCheck` is a
structured claim that type-checking *happened and produced diagnostics*. On an
outage the compiler was never reached. An author submitting source through the
studio is told their code failed type-checking, by the error's own type. No
rewording fixes that — the variant is wrong.

**#2 asserts a specific claim it never tested.** `bail!` fires on ANY non-zero
exit and states the io module "must compile standalone". It *does* append the
child's stderr, so the hex.pm error is right there —

> A correct detail beneath a false headline is worse than no detail, because it
> makes the message look thorough. (Athena)

— and an operator reads the sentence, not the appended stderr.

**#4 is a scope defect in #74's own fix.** #74 cured
`tests/test_support/gleam.rs` (four byte-identical copies, pinned by
`the_shared_gleam_support_copies_have_not_diverged`). `builder.rs` holds a
hand-rolled 12-line gate inside `#[cfg(test)] mod tests` in **`src/`**, so a
sweep over `tests/` could never see it. It announces with `println!` — the exact
pattern #74 replaced, swallowed by the harness on a passing test. Two tests
depend on it (`builder.rs:1343`, `:1446`).

⇒ **A fix's search space is part of the fix.** #74's cure was right and its sweep
was one directory wide.

`aion-package` already solved #4 correctly and the precedent is in-tree:
`crates/aion-package/src/structure/mod.rs:39` uses
`#[cfg(test)] #[path = "../../tests/test_support/gleam.rs"] mod gleam_test_support;`
with the comment *"instead of a third copy that looks right in isolation."*

## What must NOT be done

**Do not gate the failing test off.** #74 examined announce-and-skip and refused
it: *"a fetch failure that skipped would let the proof pass while proving
nothing."* Gating trades a loud red for a quiet green — the exact vacuity this
cycle is about. The test stays a FAIL; what changes is that the failure tells the
truth about whose fault it is.

## What landed

1. **`aion-toolchain/src/gleam_failure.rs`** — new. `GleamFailureLayer` +
   `classify_gleam_failure`, anchored to `gleam`'s own `error: <Title>` lines.
   The variants are deliberately asymmetric: `DependencyLayer` is a **finding**,
   `NotDependencyLayer` is an **absence**. There is no "the author's code is
   broken" variant to reach for, so a caller cannot read the default as a
   diagnosis. 9 unit tests, including the source-snippet hazard (a Gleam file
   containing the literal text `"HTTP error"` must not classify its own compile
   failure as an outage) and the mirror-image honesty check (an *unsatisfiable*
   requirement is the author's to fix and is NOT excused as infrastructure).
2. **`ToolchainError::DependencyLayer`** — new variant. The registry case stops
   being called `TypeCheck`.
3. **`aion-server` `map_toolchain_error`** — routes it to the retryable **503**,
   not the inline **400**. This is where the polarity actually bit: the old path
   told an author two untrue things in one response — that type-checking produced
   these diagnostics, and (by the 400) that their submission was at fault.
4. **`aion-cli/src/generate.rs`** — classifies, and on a registry failure says
   *"this is a network or registry failure, NOT a problem with your code"*. The
   unrecognised branch keeps the useful hint but demotes it from an assertion to
   a hypothesis: *"the most common cause is…"* rather than *"must compile
   standalone"*.
5. **`aion/src/engine/builder.rs`** — the hand-rolled `println!` gate is gone,
   replaced by the canonical copy. Declared in `engine/mod.rs`, not beside its
   consumer, for a mechanical reason worth recording: `#[path]` resolves against
   the declaring module's directory, `builder.rs` is not a `mod.rs`, so its
   children resolve under a `src/engine/builder/` **that does not exist** — the
   `..` traversal fails at the OS before rustc reads the target.

## Deliberately NOT done, and why

- **`aion-cli/src/package.rs` is not classified.** It inherits stdio, so
  `gleam`'s own error — hex.pm included — is already on the user's terminal
  verbatim and live, and its message asserts nothing about their code. There is
  no false claim to remove. Classifying would mean capturing output and printing
  it afterwards, trading streaming progress on a multi-second build for wording
  that is already honest. Marked in the source so it does not read as an
  oversight.
- **No retry was added to the CLI.** The test harness retries because a test
  must not flake; a human at a terminal is better served by failing fast with a
  message that names the cause. Adding a hidden bounded backoff would change
  `aion generate`'s latency profile on a value nobody has ruled on.
- **`TypeCheck` still covers every other non-zero exit.** Narrowing it further
  (disk-full inside `build/`, a permission fault) is a wider API change that
  needs the server's 400-mapping reviewed with it. The registry case was the
  one making a false structured claim; that is fixed.

## Evidence

Both arms proven by mutation, and re-proven after the assertions were rewritten
to satisfy the `panic` lint:

- route `DependencyLayer` back to `TypeError` → the two registry pins go RED,
  and `a_real_type_error_is_still_the_inline_400` stays GREEN. That positive arm
  is the survival check: without it the negative assertion would still pass if
  the mapper stopped producing `TypeError` for anything at all.
- make `classify_gleam_failure` always answer `NotDependencyLayer` → the three
  identification tests go RED.
- both files restored byte-identical afterwards (`cmp` against the pre-mutation
  copies).

`cargo fmt --all`, `cargo clippy --workspace --all-targets -D warnings` clean.

## Status

**LANDED.** Certified by a COLD full-workspace battery (`target/debug` reclaimed
first, so every crate rebuilt from source):

```
WORKSPACE_EXIT=0            complete run, not a truncated one
targets 240   UNPAIRED 0    every target reached a verdict
verdicts 243  NON-CLEAN 0   whole-file scan, not a tail
FAILED 0      failures: 0   compile errors 0
IGNORED 0                   #86's floor held through this change
line 1664  input_skeleton_round_trips_… ok      <- the #125 trigger
```

The 243/240 gap is accounted for, not waved past: `aion-server` re-invokes its
own test binary three times under a filter (the #74 child-process pin, which
proves a skip announcement survives into a *passing* run), so three targets emit
two verdict lines each. `UNPAIRED 0` is the control that matters — it is the
claim that no target started and vanished, and it is independent of the tally.

Cite `IGNORED: 0` and `UNPAIRED: 0`, **not** the 3975 pass total. Making a gap
visible raises that number, so it moves opposite to the truth it encodes.
