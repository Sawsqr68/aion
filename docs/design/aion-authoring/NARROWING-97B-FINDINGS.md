# #97-B: what actually stops an optional narrowing

Investigated 2026-07-31 ~04:1xZ at main `6bfe9aa2`. **Reproduced, located, not
yet fixed.** These are measurements, not a plan.

## The brief's framing is slightly off, and that matters

The brief (`BRIEF-DECLARED-CONTRACT-AT-THE-BOUNDARY.md`, Part B) says the
`narrow` slot "never reaches a statement body" because `blocks.rs:102` and
`walk.rs:291` pass `narrow: None`. Both readings need correcting before anyone
spends time on those two lines:

1. **Outcome guards DO narrow, today.** `outcomes.rs:334-363` builds the narrowed
   view and threads it through the whole clause. Verified at my own hand:

   ```awl
   input flag: Bool?
   step decide
     outcome yes: when flag is present and flag, route done(taken: true)
   ```
   → `ok: … (1 step)`. So the machinery is not "half-built" in the sense of
   being unreachable; it works for the case it covers.

2. **The two `narrow: None` sites are not the defect.** `blocks.rs:102` is the
   `fork … in <collection>` header and `walk.rs:299` is a sibling header
   position. Neither sits inside a guard arm, so `None` is the correct value
   there. Changing them would be motion, not a fix.

## The real defect: narrowing keys on a NAME, so a FIELD PATH can never narrow

Reproduced exactly as the assistant port hit it:

```awl
type Nudge { end: Bool? }
step decide
  outcome yes: when nudge.end is present and nudge.end, route done(taken: true)
```
→ `error: `and` needs Bool operands, found Bool?` at the second `nudge.end`.

The cause is at `checker/outcomes.rs:391-407`, `narrowed_binding`:

```rust
let Expr::Ref { name, .. } = subject.as_ref() else {
    return None;      // <-- a field path is not a Ref, so it never narrows
};
```

and the consumption side matches it: `View::narrow` is
`Option<&(String, Ty)>` — **one name**, compared by string equality in
`View::get` (`exprs.rs:36-41`) and `view_lookup` (`outcomes.rs:274-280`). A
field access is typed by `type_field` → `field_access`
(`exprs.rs:174-193`), which never consults `view.narrow` at all. So the
representation itself cannot express "`nudge.end` is narrowed": it can only
name a binding.

## What a fix has to change (shape, not yet a decision)

- `View::narrow` must key on a **path** (`nudge.end`), not a bare name. Either a
  `Vec<String>`/dotted-string key, or the resolved expression's canonical form.
- `narrowed_binding` must accept `Expr::Field` chains, resolve the base through
  the scope, and confirm the tail field is `Optional(_)`.
- `type_field`/`field_access` must consult the narrowed path before unwrapping,
  and must do so for the WHOLE path so `a.b.c` narrows without `a.b` also
  appearing narrowed.
- Field access THROUGH an optional record already unwraps one level, which is
  why `guard_optional_wait.awl` LOOKS like it narrows and is not evidence that
  this works. Any test must use a **non-optional record with an optional
  field**, or it passes for the wrong reason.

## Why it is worth doing

The assistant port had to make BOTH `assistant_continue` signal fields
REQUIRED because of this. That changed the operator contract: `{"end": true}`
alone is valid against the Gleam original and is refused by the AWL port. So
this is not a checker nicety — it has already cost a live wire contract.

## Gate shape when it is built

- A valid fixture: non-optional record, optional `Bool` field, narrowed and
  used in an `and`.
- A valid fixture for the deeper path `a.b.c`.
- An INVALID fixture proving the narrowing is scoped to its arm — the same
  field used un-narrowed in a sibling `otherwise` arm must still be refused.
  Without that, a fix that narrows globally would pass every positive test.

---

# BUILT — 2026-07-31

The measurement above stands. What follows is what was built against it, and
the one thing the build found that the measurement had not.

## It was three implementations, not one

The finding above located the defect in the checker. It is in the checker, but
narrowing is written **three times**, and all three keyed on `Expr::Ref`:

| Layer | Site | What it does |
|---|---|---|
| Checker | `checker/narrow.rs` (new), `exprs.rs`, `operators.rs`, `outcomes.rs` | types the narrowed read |
| Direct lowering | `mir/lower/outcome.rs`, `mir/lower/expr.rs` | emits `AssertSome`, resolves the read |
| Gleam reference emitter | `emitter/exprs.rs`, `emitter/outcomes.rs` | emits `case`/`let assert`, renders the read |

Fixing only the checker would have been worse than not fixing it: the document
would type-check and then compile to a test against a wrapped optional —
always false, never reported. All three moved together.

The shared identity is now `ast::access_path` / `ast::field_path`, in the AST
rather than in any one consumer, because the three layers have to agree on
*which reads a proof covers* and that agreement should not live in one of the
three.

## The narrowing is a set keyed on a path

`View::narrow` went from `Option<&(String, Ty)>` to `&[Narrowing]`, each a
`(path, ty)`. One guard can prove more than one path, and a later proof can
depend on an earlier one — `a is present and a.b is present` narrows `a.b`
only because the first proof made `a` a record. `path_type` consults the
proofs at **every prefix**, so `a.b.c` reads correctly when an outer arm
proved `a.b`.

The two backends hold the same thing in their own currency: the MIR `Scope`
maps a path to the `Var` holding the unwrapped value; the emitter `Scope` maps
it to the Gleam identifier. A bare binding still rebinds its own NAME in both
— that shadowing form is what every existing golden emits, and it is
untouched.

## What the build found that the measurement missed

**A checker may not be more permissive than the code generators.** Mid-build
the checker accepted `a is present and a.b is present and a.b`. Both backends
narrow from a binary's DIRECT left operand only, and `a and b and c` parses as
`(a and b) and c` — so `b`'s proof never reaches `c` in either generator. The
checker was pulled back to exactly that rule. **The three-conjunct chain remains
unsupported, deliberately and symmetrically.** Widening it is a three-layer
change; it is not done here and is not claimed.

The same asymmetry decides the ARM: a bare `is present` guard narrows the arm
in all three layers, but under a COMPOUND guard the emitter never narrows the
arm (it renders the guard as an expression, so the `case` binding does not
reach the route). The fixtures exercise each position where all three agree,
and no further.

> **SUPERSEDED — both paragraphs above.** #110, in the section below, made the
> three-layer change. A chain now carries every proof it makes, and the arm is
> narrowed under a compound guard in all three layers. What stands from this
> section is the RULE it states — a checker may not be more permissive than its
> code generators — which is why the limitation was recorded rather than
> patched at one layer. The limitation is gone; the rule that governed it is
> still the reason the fix took the shape it did.

## What is proven, and by what

- `step-bodies/valid/guard_narrows_an_optional_field` — both positions: the
  guard's short-circuit edge, and a bare-guard arm reading the payload.
- `step-bodies/valid/guard_narrows_a_deep_optional_field` — `a.b.c`.
- `step-bodies/invalid/narrowing_does_not_escape_its_arm` — the sibling
  `otherwise` arm still refuses the un-narrowed read.
- Both valid fixtures joined the BC-2 coverage ratchet and the BC-4
  differential corpus (`COVERED_COUNT` 88 → 90).

Verified failing at my own hand, per layer:

- Checker: with the narrowed-path lookup removed from `type_field`, both valid
  fixtures fail with the ORIGINAL defect — `` `and` needs Bool operands, found
  Bool? `` in the guard, and a `Bool?` payload in the arm.
- Lowering: with the narrowed-path lookup removed from `lower_expr`, the
  committed MIR goldens mismatch. The goldens are load-bearing for this.
- Emitter: with the narrowed-path lookup removed from `render_expr`, the
  reference emits `Some(awl_present_nudge_end) -> nudge.end` — it binds the
  unwrapped value and then reads the optional anyway, which is a Gleam type
  error, so BC-4 fails at `gleam build`. With it, the module carries
  `case nudge.end { Some(…) -> awl_present_nudge_end }` in the guard and
  `let assert Some(…) = nudge.end` in the arm, and BC-4 runs both backends
  through a real engine to byte-identical trails.

Gate on the final tree: `cargo test -p aion-awl` exit 0 (44 binaries),
`cargo test -p aion-rs --test awl_bc4_differential` exit 0 (26 passed),
`cargo clippy --workspace --all-targets -- -D warnings` exit 0.

## One golden changed, and why

`step-bodies/valid/predicates_and_operators` already guarded
`candidate.veto_note is present` — a field path that until now narrowed
nothing at all. It now materializes the proof on the proved edge (a `FieldGet`
plus an `AssertSome`), which is what a bare binding has always done. Net +2
instructions on one taken branch; the rest of the diff is variable
renumbering.

That arm does not read the value, so the unwrap is dead there — see the note at
the end of the #110 section, which now covers this case too. It is emitted
anyway, because the rule "the proof is materialized on the edge that proved
it" is the same rule names already follow, and making materialization
conditional on a read means threading a read-set through both generators'
decision builders. Worth doing as an optimization; not worth the complexity
inside a correctness fix. The Gleam side is safe: no `--warnings-as-errors` on
the reference build, so the unused binding is a warning at most.

---

# #110 — the chain carries every proof, not just the first

Built 2026-07-31, immediately after the above. #97-B made an optional FIELD
narrow; it left the reach of a proof pinned to one operand. This closes that.

## What was wrong

`a and b and c` parses as `(a and b) and c`. All three layers narrowed from a
binary's DIRECT left operand, so on the outer node the left was another binary,
matched no predicate, and proved nothing. A proof the MIDDLE conjunct
established never reached the last one. The natural way to write an optional
record with an optional field —

```awl
when nudge is present and nudge.end is present and nudge.end
```

— was refused, and #97-B recorded that refusal as deliberate rather than fix it,
because widening the checker alone would have been worse than the gap.

## The shape of the fix

`ast::chain_operands(expr, op)` flattens a chain of one connective into its
operands in source order, and joins `access_path`/`field_path` as AST-level
shared identity: all three layers must agree on which operands a proof spans,
so the definition does not live in any one of them.

Each layer then keys on the chain, and they do it DIFFERENTLY on purpose,
because they reach an arm by different routes:

| Layer | Guard position | Arm position |
|---|---|---|
| Checker (`checker/narrow.rs::collect`) | collect from EVERY operand of the left chain, left to right, each proof visible to the next | `from_guard` collects every conjunct of the whole guard |
| Direct lowering (`mir/lower/outcome.rs`) | `narrow_short_circuit_rhs` narrows from the chain's LAST operand — `guard_decision`'s recursion already wrapped all the earlier ones | `narrow_present` narrows the LAST conjunct only, for exactly that reason |
| Reference emitter (`emitter/exprs.rs::render_chain`) | one nested `case` per proving operand, everything after it rendered INSIDE that branch | `emit_arm` re-establishes EVERY conjunct as a `let assert`, because the guard is rendered as an expression and its bindings never reach the route |

Same unwraps, same order, three routes. The asymmetry is not an accident and is
documented at each site.

The emitter restructures a chain ONLY when some non-final operand proves
something. A chain that proves nothing renders as the flat operator expression
it always has, so no existing document's Gleam moved a byte.

## Proof directionality is preserved

Flattening could plausibly have made proofs commute. It does not: collection
runs left to right and a read is typed against only the proofs already
collected. `when session.ended and session.ended is present` is still refused —
that is the `invalid/proof_does_not_flow_backwards` fixture, and it exists
because a fix that collected the whole chain position-blind would pass every
positive test while compiling to a test against a still-wrapped optional.

## What is proven, and by what

`step-bodies/valid/guard_chain_carries_every_proof` covers both carriers and
both positions in one document, with no redundant conjunct:

- `hinted` chains over bare BINDINGS (a proof rebinds its own name);
- `ending`/`resumed` chain over FIELD PATHS (a proof mints a name);
- in each, the MIDDLE conjunct's proof is what makes the third conjunct type,
  and the FIRST conjunct's proof is what makes the arm's route payload type.

Against the PRE-FIX binary the fixture fails with five errors — three
`` `and` needs Bool operands, found Bool? `` (guard position, both carriers) and
two `field `note` … expects String, found String?` (arm position). It joined the
BC-2 coverage ratchet and the BC-4 differential corpus (`COVERED_COUNT` 90 → 91).

Verified failing at my own hand, per layer, one injection at a time:

- **Checker** — `collect` iterating `[expr]` instead of the chain: the fixture
  fails with the original defect at line 45, the third conjunct.
- **Lowering** — `narrow_short_circuit_rhs` reading `left` instead of the
  chain's last operand: `golden mismatch at …/guard_chain_carries_every_proof.mir`.
  The golden shows why it is load-bearing — `assert_some v0` (the middle proof)
  followed by `if is_true v12`, testing the UNWRAPPED value, and the arm reading
  `v10` from the first proof.
- **Emitter** — `render_chain` gated to two operands: `gleam build` refuses it,
  `The && operator expects Bool … but this argument has type Option(Bool)`, on
  both the bare-binding chain and the field-path chain.

Gate on the final tree, exit codes captured: `cargo test -p aion-awl` exit 0
(44 binaries, 489 passed, 0 failed), `cargo test -p aion-rs --test
awl_bc4_differential` exit 0 (26 passed, both backends byte-identical),
`cargo clippy --workspace --all-targets -- -D warnings` exit 0.

## Still not supported, and said so

A proof established inside a nested `or` does not reach a sibling `and` chain
and vice versa — flattening is per connective, which is what both generators
compile. Nothing in the language reads as if it should.

The dead-unwrap note from #97-B now applies here too: a proof is materialized on
the edge that proved it even where that arm never reads it, so
`route confirm` under a three-conjunct guard carries two unwraps it does not
use. Same reasoning, same conclusion — an optimization, not a correctness
matter, and not worth threading a read-set through two decision builders inside
a correctness fix.
