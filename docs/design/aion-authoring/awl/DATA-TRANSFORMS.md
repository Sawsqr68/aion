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

## 8. Landscape lessons (external survey, Jul 25 2026 — session records with the blind-reader study)

The survey (jq, JSONata, JMESPath, Step Functions' 2024 JSONata migration, CUE, Jsonnet, XSLT, Temporal converters, GraphQL, LINQ, Elm) converged on the same verdict as the blind-reader panels: a **typed shape calculus**, not a general embedded expression language. Rules promoted into this design from evidence:

- **Absence is a taxonomy, not a value.** jq conflates null/no-output/false/error behind `?`; JMESPath silently returns or omits null structurally; JSONata's empty/singleton/multi sequence rules change output shape invisibly. Here: *missing*, *null*, *optional-absent*, *empty collection*, and *failure* are five distinct, checker-visible things. Nothing is silently omitted, ever.
- **Cardinality is explicit.** jq's stream algebra (everything is a zero/one/many stream) is its documented readability cliff. Here: `map`/`filter`/`flat_map` are distinct named operations with typed cardinality (the LINQ lesson); no implicit flattening, no result streams.
- **Prohibitions (the XSLT/Jsonnet lessons), added to §7:** no implicit context/current-node rebinding, no global or implicit dispatch, no inheritance/overlay merging, no late binding, no dynamic evaluation, no user recursion, no arbitrary I/O. XSLT's plausible-but-unintended output came from implicit template dispatch with built-in fallbacks — the exact uncanny valley Tom named, forty years early.
- **The escape hatch is typed and named, not gradual.** Step Functions' intrinsic functions show what happens when a deliberately-small layer meets real needs with no principled exit: users decant into Lambda glue. Here: a registry of named, typed, deterministic host functions (`tail(s, lines: N)`-class) — the simple layer never accretes into a second programming language because the pressure has a typed somewhere to go.
- **Converters are protocol, not serialization detail** (Temporal). Transform and codec versioning at durable boundaries follows the handshake's written compat rules (§2.6 there); a transform's identity participates in the content-hash pinning story.
- **Precedents adopted:** GraphQL selection sets for compile-validated output shape; CUE's conflicts-are-errors rigor (never last-writer-wins) under the schema system; Elm-style same-type record update; Jsonnet's hermeticity (pure, no ambient state) with none of its dynamism.

Survey unknowns resolved by existing rulings: transforms compile into workflow bytecode and are pure/deterministic, so replay semantics are settled by construction (§2); templating is text-only and JSON is never string-built (§4), so escaping-model concerns don't arise in the reshaping layer; user-defined pure transforms are the feature itself (§2). Deferred, stated: joins/grouping/dynamic object keys and streaming evaluation wait for a demanding consumer; first release covers the pipe-stage family plus shape-mapping.

## 9. Open design questions (for the checker/MIR owners)

- Surface syntax for deep paths and narrowing (`?.` vs match-based) — pick what composes with existing accessor syntax.
- Whether transforms compile to dedicated MIR functions or inline at call sites (replay-neutral either way; the choice is code-size vs locality).
- Template output types beyond `String` (markdown/document builders) — defer until a consumer demands it.
- Interaction with the parity-first construct landings (`on failure`, substeps) — transforms should land after those, on a compiler that already expresses the full language.
- Hole syntax for templates (`{expr}` vs `${expr}`) — both blind-reader panels sketched `${}` unprompted (Sol-medium, Sol-xhigh); weight that as the reader-native instinct.
