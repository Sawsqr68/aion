# AWL Data Transforms, Templating, and Querying

**Status:** Design — Phase 3 peer of the worker shapes (Tom's brief, Jul 24 2026)
**Author:** Cally Ray
**Doctrine:** Every transformation done inside AWL is one the type system sees. Data munging outsourced to shell `jq`, ad-hoc string concatenation, or a model call is a shim by the estate's definition — typed transforms are the foundation that replaces it.

## 1. Why this belongs in the language

Workflows are mostly plumbing between typed surfaces: a worker's output feeds another worker's input, a tool's JSON feeds a prompt, a collection of results folds into a report. Today the gaps between those surfaces are crossed by untyped means, and every untyped crossing is a future mid-run death of exactly the class the worker-contract handshake (`docs/design/aion-workers/WORKER-CONTRACT-HANDSHAKE.md`) exists to refuse. The language already carries the foundations: typed pipes with filter/map/sort/count/any/all stages, records, variants, accessors, predicates, and schema-checked inputs/outcomes. This design completes them into a first-class transformation capability.

## 2. Transforms — first-class, pure, checked

```awl
transform brief_to_lane(brief: Brief) -> LaneSpec {
  ...
}
```

- **Schema-in → schema-out, verified by the checker** like every other declaration. An unknown field, a narrowing without handling, a missing variant arm — compile errors, never runtime nils.
- **Pure and total.** No activities, no IO, no clock, no randomness. Purity means transforms execute inline in workflow bytecode with no durability events and are trivially replayable.
- **Failure is typed, never thrown.** An operation that can structurally fail (index out of range, absent optional) either returns an option/result the transform must handle, or the transform's signature declares `-> Result(Out, ReasonType)` and callers route it like any outcome. A transform cannot be a source of mid-run structural death — that is the cardinal rule applied inside the expression layer.
- Usable anywhere an expression is: pipe stages, worker-call arguments, route payloads, fork collections.

## 3. Querying — typed reach into structured data

`jq`-class projection and filtering, but checked:

- Deep field paths with **compile-time verification against the declared schema** — querying a field that doesn't exist is a compile error.
- Optional-chaining with **typed narrowing**: traversing an optional yields an optional; the checker forces the absent case to be handled or explicitly propagated.
- Collection queries compose with the existing pipe stages (filter/map/sort/any/all) — this is an extension of that surface, not a second dialect of it.
- No dynamic/stringly path evaluation. A query is syntax, not data; there is nothing to inject and nothing the checker can't see.

## 4. Templating — typed holes, safe construction

```awl
template lane_prompt(brief: Brief, wave: Wave) -> String {
  ...
}
```

- String/document rendering with **holes typed against schemas** — a template referencing a field the schema doesn't carry fails at compile time. This is the prompt-construction story for agent steps, done safely: the thing feeding models stops being ad-hoc concatenation.
- **JSON is never built by templating.** Structured values are constructed as records/variants and serialized by the codec layer — injection-proof by construction, not by escaping discipline. Templates produce human-/model-facing text only.
- Templates are transforms (pure, total), so everything in §2 applies.

## 5. Shape-mapping between contracts

The highest-value application: a declared transform from worker A's **output schema** to worker B's **input schema**, checked against **both contracts** via the handshake machinery. The adapter worker shapes (MCP especially — tools emitting arbitrary JSON) make this mandatory: the MCP adapter's output contract meets a declared transform, and the checker proves the chain end-to-end at compile time. Today that seam is exactly where `jq` shims and silent mismatches live.

## 6. Relation to the authoring harness

Transforms are pure, so **Tier 0 executes them at edit time**: fixture inputs run through every transform in the document on keystroke, outputs validated against target schemas. A transform bug is caught in the editor before even the simulated run — the cheapest possible failure. Fixtures for a shape-mapped chain double as documentation of the real payloads crossing it.

## 7. Non-goals

- Not a general-purpose language: no IO, no effects, no unbounded recursion (bounded iteration only, matching existing loop semantics).
- No schema inference — transforms declare their shapes; the checker verifies, never guesses.
- No stringly-typed query evaluation at runtime.
- No auto-coercion between shapes: every mapping is a declared, reviewable artifact.

## 8. Open design questions (for the checker/MIR owners)

- Surface syntax for deep paths and narrowing (`?.` vs match-based) — pick what composes with existing accessor syntax.
- Whether transforms compile to dedicated MIR functions or inline at call sites (replay-neutral either way; the choice is code-size vs locality).
- Template output types beyond `String` (markdown/document builders) — defer until a consumer demands it.
- Interaction with the parity-first construct landings (`on failure`, substeps) — transforms should land after those, on a compiler that already expresses the full language.
