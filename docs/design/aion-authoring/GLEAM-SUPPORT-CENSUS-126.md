# The divergence pin could not see a copy nobody wrote down (#126)

Found 2026-08-01 while the #125 battery ran. It is the **same defect class as
#86's tautological baseline**, this time sitting inside the pin #74 shipped
specifically to prevent divergence.

## The defect

`the_shared_gleam_support_copies_have_not_diverged` reads `SHARED_COPIES` and
proves every path in it is byte-identical. That is a real proof and it is not
the one the comment above it implies. **The list it verifies is also the list it
derives its expectation from**: delete an entry and the pin stops looking for
that file and still passes. It can answer "are the copies I was told about the
same?" It cannot answer "how many copies are there?"

The census, measured:

```
613  crates/aion-awl-package/tests/test_support/gleam.rs   listed
613  crates/aion-cli/tests/test_support/gleam.rs           listed
613  crates/aion-server/tests/test_support/gleam.rs        listed
613  crates/aion/tests/test_support/gleam.rs               listed
398  crates/aion-awl/tests/test_support/gleam.rs           ← UNLISTED
 77  crates/aion-package/tests/test_support/gleam.rs       ← UNLISTED
```

Two copies existed that no pin could see, and the module doc **argued for one of
them**: *"`aion-awl`'s own copy is deliberately NOT in that set: it carries
emitter-specific wording and is owned by another lane."* That sentence is why
nobody looked. It has been replaced.

> 🔴 **A BASELINE IS ONLY A CONTROL IF IT IS INDEPENDENT OF WHAT IT MEASURES.**
> Enumerate the population, never the answer.

## What landed

Three lists, so the **name** carries the status and a reader never has to weigh
prose to tell "known-wrong" from "correctly different":

- `SHARED_COPIES` — must stay byte-identical. Unchanged, still pinned as before.
- `DIVERGENT_COPIES` — has drifted and should converge. **A debt, not a
  decision.**
- `SEPARATE_BY_DESIGN` — correctly a different artefact, with the reason.

Plus `every_gleam_support_file_is_classified`, which walks `src/` and `tests/`
under every directory in `crates/` for files named `gleam.rs` and asserts **both
directions, neither derived from the other**:

- **population → lists** — every file found must be classified by exactly one
  list. This is the direction the divergence pin is structurally blind to.
- **lists → population** — every listed path must have been found. Catches a
  stale entry, and is what keeps the test non-vacuous: a walk that found nothing
  fails loudly instead of passing on an empty set.

`src/` is searched, not just `tests/`, because #125 found a hand-rolled `gleam`
gate inside `src/` where a sweep over `tests/` could not see it. **A search
space that excludes a place the thing has actually been found is not a search
space.**

## The two unlisted copies are not the same kind of thing

Treating them alike would have been the wrong fix.

- **`aion-awl` (398 lines) is DRIFT.** It is the ancestor the canonical 613 was
  ported from at `8fb4c153`, and it never received the improvements made in
  transit — it still calls a rejected build an "emitter defect" where the ported
  text says "code defect", and it has no dependency-layer classification at all.
  (**The source of a cure is the one place that never receives it.**)
- **`aion-package` (77 lines) is CORRECT.** A deliberate crate-local gate
  carrying only the runtime skip and its announcement, and the single definition
  included by all three of that crate's call sites — including the unit test in
  `src/`, which reaches it through a `#[path]` in `src/structure/mod.rs`.
  Converging it would import machinery the crate does not use.

## Deliberately NOT done, and why

**The `aion-awl` convergence is not in this commit.** It is not a `cp`: that
crate's copy exposes `compile_project` with a different signature to
`compile_whole_project` here, and **13 test files** call it. That is an API
refactor in another crate, and folding it in silently under a measurement fix
would be the kind of scope drift this cycle keeps catching.

What this commit does is make the drift **stated instead of invisible**, in a
constant whose name says it is a debt. The census defect and the convergence are
two defects; this closes the first.

## Verified NOT a gap

Every other test that spawns literal `gleam` routes through the shared gate:
`generate_types_first_e2e`, `new_scaffold_build`, `golden_gleam_exec`,
`schema_equivalence`, `nested_implicit_child_decode_e2e`,
`sdk_internal_privacy`. The only ungated spawn left is
`aion-server/examples/failover_demo.rs` — an **example**, not a test, so it has
no verdict to make vacuous and is out of scope for this class.

## Evidence

Both directions proven by mutation, each with an isolation control:

| mutation | new pin | old divergence pin |
|---|---|---|
| an unlisted copy appears at `crates/aion-core/tests/test_support/gleam.rs` | **FAILED**, naming the path | **GREEN** |
| a listed path that does not exist (`SEPARATE_BY_DESIGN`, applied to all four copies so byte-identity holds) | **FAILED**, printing all six discovered paths | **GREEN** |

🔴 **The old pin staying green is the point, not a footnote.** It is what proves
the new test covers something the old one *cannot* reach, rather than being a
second way to catch the same thing.

**My first attempt at the second control was wrong and is worth recording.** I
mutated only the `aion` copy — which necessarily desyncs the four, so the
divergence pin fired too and the directions were not isolated at all. Applying
the same mutation to all four holds byte-identity still and varies only the list
content. **A control is identified by the mechanism it holds still.**

The census is also directly evidenced rather than assumed: the failing run
printed all six discovered paths, so the walk is known non-empty in the passing
case, not merely inferred from a green.

Gates: `cargo fmt --all` clean and the four copies byte-identical **after**
formatting (fmt is deterministic, so identity survives it);
`cargo clippy -p aion-rs -p aion-cli -p aion-awl-package -p aion-server --tests
-- -D warnings` **exit 0, 0 warnings**; both pins green in all four crates and in
the in-`src` consumer reached through `engine/mod.rs`.

One clippy finding was fixed rather than silenced: `crates_directory` /
`crate_directory` tripped `similar_names`, and the binding was renamed to
`crates_root`.
