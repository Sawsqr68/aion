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
