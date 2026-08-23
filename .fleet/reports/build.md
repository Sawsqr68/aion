# BUILD LEG REPORT — awl-cut2 flight1: recipes / staging assembly

Written by the build leg on rocketfish. Every citation disambiguates
`BRIEF-Rn` (`.fleet/BRIEF.md`, R1–R8), `DESIGN-Rn` (the design brief's own
R1–R8) and `CUT1-Rn` (cut-1's brief, `git show 1bab9547:.fleet/BRIEF.md`).
A bare `Rn` is two different requirements and is not written here.

⚠️ This path previously held **cut-1's** report, carried into main by that
landing. This leg replaces it wholly.

---

## 0. C-1 FIRST — `excluded_term` is NOT in cut-1's enum

**BRIEF-R4 says:** *"a projection that would emit an excluded term is a
composer refusal (`excluded_term`, cut-1's enum — consumed, not
re-minted)"*.

**Measured, at cut-1's landed bytes.** `git show
1bab9547:crates/aion-awl/src/command/cause.rs` — `CommandCause::ALL` is
`[Self; 35]` at line 186 and **none of the 35 is `excluded_term`**. A
whole-tree search at the base for `excluded_term|ExcludedTerm` returns
**zero** hits in `crates/`; at this flight's HEAD before any code was
written it returned exactly one, `.fleet/BRIEF.md:215` — the brief's own
line describing it.

**Disposition, per BRIEF's own refusal-causes wall** (*"every new cause this
cut needs joins cut-1's closed enum BY DESIGN CHANGE recorded in the
report"*): the cut **MINTS** `excluded_term` into `CommandCause` as a
recorded design change, filed at the new **Compose** layer, with its red
specimen (`crates/aion-awl/tests/recipe_wall.rs`
`a_projection_that_would_emit_an_excluded_term_refuses`). BRIEF-R4's
"consumed, not re-minted" wording is reported as **divergent from the
landed bytes** rather than quietly satisfied. No source is fought: the
design's §6 law (new codes are a design change) governs and is followed.

**Waffles and Vesper: this changes what the cut was permitted to mint.** It
is the first item here for that reason.

---

## 1. The rest of the CUT-1 CONFORMANCE READ, with movement named

Read at the landed bytes at BASE `1bab95477cb86d498d07bb8da445f00351b87972`
and re-affirmed at this leg's HEAD. **No file under `crates/` moved between
BASE and the leg's first line of code** (`git diff BASE...HEAD` named 17
files, every one under `.fleet/`), so the bytes read are the bytes cut-1's
own gate went green on.

| consumed shape (BRIEF's name) | landed at | verdict |
|---|---|---|
| command-declaration **types** | `src/command/decl.rs:44` `CommandDecl` and family | **CONFORMS.** Recipe step bodies reference `CommandDecl` **by name**, never by copy. |
| the **admits-`--`** property | `decl.rs:135,149`; emitter places the marker unconditionally (`emit.rs:141`) | **CONFORMS**, and is CONSUMED unchanged: a `runs command` step emits through cut-1's own `check_command` + `emit`, so a step runs exactly the argv `aion awl check` proved sound (pinned by `recipe_fire.rs` `a_runs_command_step_hands_the_emitted_argv_to_the_runner`, which asserts `["git","push","--","fleet/awl-cut2-result"]`). |
| the **refusal cause enum** | `cause.rs:76`, `ALL: [Self; 35]`, `layer()` exhaustive, `error.rs:253 refuse` derives the layer FROM the cause | **CONFORMS** as a closed, layer-named vocabulary — with **C-1** above. |
| **`literal_where_derived`** | cause `cause.rs:128`, wire string `"literal_where_derived"`, `derived.rs:65` three closed shapes | **CONFORMS as a cause; DIVERGES as a trigger — C-2 below.** |
| the **fixture harness** | `tests/command_just_subset.rs` + `src/command/just_subset.rs` tables | **CONFORMS** and was extended in place: `ABSORBED_PRODUCTIONS` 4 → 7, `REFUSED_FORMS` 6 → 8, five new fixtures, all gated by the harness's own coverage and provenance tests. |

### C-2 — `literal_where_derived` diverges as a MECHANISM, not as a cause

Cut-1 fires the cause on the literal's **SHAPE** (`derived.rs:65`, three
closed shapes scanned word by word). BRIEF-R2's acceptance needs a
**DECLARATION-driven** trigger: a re-derive slot fed *any* hand literal.
`"main"` is invisible to `derived_shape_in` and is exactly as wrong in a
re-derive slot as a copied sha.

**Disposition — CONSUMED, not re-minted, and proven both ways.**
`crates/aion-awl/tests/recipe_parameters.rs`
`literal_where_derived_fires_against_a_re_derive_slot_fed_a_literal`
asserts three things in one test: the haem-wall exhibit's hand-copied
control token refuses (cut-1's shape rule would also have caught it),
`"main"` in a re-derive slot **also** refuses (only the declaration trigger
catches it), and `"main"` in a `skeleton` slot is **ordinary text** — so
cut-1's closed shape set is provably not widened. `src/command/derived.rs`
and `src/command/check_fills.rs` are **byte-identical to BASE** (verified by
`diff`). Cause code and wire string unchanged.

### C-3 — the amendment-1 fence is load-bearing and SURVIVED byte-identically

`src/command/words.rs`, `src/command/parse.rs` and
`tests/fixtures/command/just-subset/refusal-recipe-body.awl` are
**byte-identical to BASE** (verified by `diff` against `1bab9547`). The
existing `grammar_parser_gate.rs`
`every_assembly_form_is_refused_by_name_with_its_reason` still passes over
all six `ASSEMBLY_WORDS`, and this cut adds its own copy of that assertion
in `recipe_declarations.rs` so the fence is pinned from the assembly side
too.

### C-4 — one movement, named: the grammar gate's grip on the parser's sentence

`tests/grammar_parser_gate.rs`'s `DECLARATION_REFUSAL` constant **moved**,
because the dispatcher's diagnostic now names `recipe`, `wall` and
`artifact` beside `command`. This is the movement cut-1's own comment
anticipated (*"this constant is the gate's grip on the parser's own
sentence, not a claim about what the parser should admit"*), and it is
recorded here because it is an edit to a gate.

**It was not a silent edit: leaving it behind made the gate go RED first.**
Every probe read as "admitted", and both equality assertions failed loudly —
the fail-safe the constant's comment describes, working. The new comment in
the file records that.

---

## 2. THE CENSUS — has this already happened? (fresh, at this HEAD)

⛔ **Pathspec, stated.** The whole worktree at HEAD, excluding exactly
`.git/`, `target/` and `node_modules/` and nothing else; text files only
(`grep -I`). Narrower pathspecs named per row where the question is a
language-surface one. Run fresh: cut-1's census answered a different
question on a tree that has since moved.

| construct sought (BRIEF's own five) | pathspec | result |
|---|---|---|
| a **recipe / dependency construct** in the AWL word tables | `crates/aion-awl/src/{lexer,parser,ast,command,guide}/**` + whole-tree `-w recipe` | **ZERO as a construct.** `POSITIONAL_WORDS` was `[&str; 19]` and carried no `recipe`, `needs`, `depends`, `steps`; `Keyword::ALL` (58) carried none either; the top-level dispatch admitted exactly `const`/`type`/`worker`/`child`/`subflow`/`step`/`command`. Every in-tree `recipe` hit was cut-1's own refusal machinery (triaged as C-3) or the English word in MIR codegen prose (`src/mir/func.rs`, `src/mir/lower/codec*.rs`, `src/mir/select/shells.rs`) — a template-shape noun, not a construct. |
| any **wall-projection or prompt-assembly machinery** | whole worktree, `-i` on `wall_projection\|project_wall\|WallDecl\|prompt_assembly\|assemble_prompt\|PromptBuilder\|excluded.term\|negative vocabulary` | **ZERO.** The only `excluded_term` was `.fleet/BRIEF.md:215` (C-1). Every `-w wall` hit in `crates/` was wall-*clock* determinism prose (`aion-awl-package/tests/awl_assemble.rs`, `aion-cli/tests/authoring_quality_e2e.rs`) or cut-1's **`haem-wall` fixture NAME**. Nothing derived a prompt or a gate rendering from a declaration. |
| a **sweep-by-claim** mechanism | whole worktree, `-i` on `sweep_by_claim\|sweep-by-claim\|SweepRecord\|sweep(` | **ZERO.** Three hits, all triaged: an author-domain action named `sweep` in a deliberately-invalid rev2 fixture (`tests/fixtures/rev2/loop-outcomes/invalid/pipe_route_in_loop_body.awl:12-16`); `plan_sweep` in the server's worker backpressure (`aion-server/src/worker/backpressure.rs:169,177,492` — a queue sweep, different domain, **outside this cut's wall**); a round-robin test name (`outbox_dispatcher.rs:2060`). None re-projects consumers from a corrected source declaration. |
| a **reference-census / content-hash fire-precondition** surface | `crates/**` on `reference_census\|ReferenceCensus\|fire_precondition`; `crates/aion-awl*/src` on `content_hash\|ContentHash` | **ZERO in the front** — the count was literally 0 across `aion-awl/src` and `aion-awl-lsp/src`. Two adjacent hits triaged: `mir/lower/driver.rs:83 preconditions(...)` is a LOWERING precondition (a different sense), and `aion-package/src/hash.rs:89 content_hash(beams)` hashes a BEAM set for module namespacing — real prior art for the ALGORITHM and already a dependency of the front, but it answers "what is this package's identity", never "do the deployed bytes still match what a recipe declared". |
| any **staging-receipt generator** | whole worktree, `-i receipt` over `crates/` and `scripts/` | **ZERO as a generator.** A tighter scan for `(struct\|enum\|fn\|impl) +[A-Za-z_]*[Rr]eceipt` over `crates/` and `scripts/` returned **0**. Every `receipt` hit is the author-domain NOUN in AWL examples, fixtures and MIR goldens — a user's `type Receipt { ok: Bool }`. Nothing wrote a record naming the tree, head and clean/dirty state it measured. |

**Five constructs sought, five zeros, every hit triaged.**

**One further adjacent surface, triaged as the nearest prior art.**
`crates/aion-fleet-plan` — `src/verify.rs:55 ExactCommit`, `:90 Verdict`,
`:125 verify_conclusion`, `src/error.rs:60 ConclusionCause` (exactly three
codes) — is a SEALED VERIFIER of a stage-conclusion artifact at an exact
commit. It is not a recipe surface, not a receipt generator, and is
**outside this cut's wall**. It is **read as data** in exactly one place
(`recipe_compare_genericity.rs`, cited by digest, re-hashed on the run) and
**modified in none**.

---

## 3. STAGING CONFORMANCE — the rig's claims, measured

1. **The gate's "changed in exactly three places" claim HOLDS.**
   `git diff 1bab9547 731fa74a -- .fleet/gate-entry.sh` is 9 insertions /
   33 deletions, of which exactly **two** are executable lines: `BASE`
   (`5e2fb5a3e…` → `1bab9547…`) and `WALL` (gains `crates/aion-cli`). The
   rest is the cut-1 F-RIG-1 comment block replaced by the cut-2 wall
   rationale. **No amendment to the gate was needed by this cut and none
   was made.**
2. **Seam movement between BASE and HEAD: NONE.** 17 files, all under
   `.fleet/`.
3. **F-STAGE-1 (observation, reported, NOT repaired).** The rig commit
   carried a **second, nested, byte-identical copy** of all four gate
   specimens at `.fleet/fixtures/gate-scripts/gate-scripts/`. Harmless to
   the gate and to `tests/command_gate_census.rs`, which cites the OUTER
   path — but it is a second hand-copy of pinned bytes with no provenance
   row, which is the very class BRIEF-R6's one-truth fence exists to close.
   **Disposition:** reported; **not deleted** (staged bytes are staging's,
   and the estate's never-destroy law binds); every fixture this cut wrote
   cites the OUTER path.
4. **Cosmetic, recorded, not amended:** `.fleet/gate-entry.sh:2` still
   reads "AWL-CUT1 gate" in its header comment. No behaviour depends on it.

---

## 4. PLACEMENT AS TAKEN, and the MANIFEST DEPENDENCY DELTA (fold B)

### D1 — the recipe family is a SUB-FAMILY of the command surface

Taken: `crates/aion-awl/src/command/recipe/` (15 files), not a sibling
`src/recipe/`. The ground is mechanical and measured in this tree:
`CommandRefusal::refuse` — the single door that derives a refusal's LAYER
FROM its CAUSE, so misfiling cannot be written — is `pub(super)`
(`src/command/error.rs:276`). A sibling would have forced either widening
cut-1's landed refusal door or building a second construction path for the
same record. **The door was not widened.** (Where a test needed to build
refusals from outside the surface, it used the PUBLIC per-layer
constructors, which is what they are for — see
`recipe_compare_genericity.rs`'s own note.)

### D2 — the executing verb rides `crates/aion-cli` (the ratified placement)

`aion awl recipe compose` and `aion awl recipe fire`, in a new module family
`crates/aion-cli/src/awl_recipe/` (5 files) beside the existing AWL verbs —
**not** by growing `awl.rs`, which was already 587 lines. The front declares
three ports it does not implement (`ArtifactSource`, `SpecimenFirer`,
`CommandRunner`, bundled as `Ports`); the CLI implements all three over a
real tree and real processes.

### THE DEPENDENCY DELTA, MEASURED

```
crates/aion-awl/Cargo.toml, BASE 1bab9547 → HEAD:
  [dependencies]      + sha2.workspace = true
  [dev-dependencies]  - sha2.workspace = true
crates/aion-cli/Cargo.toml:  NO CHANGE
Cargo.lock:                  NO CHANGE
```

**Required: zero execution-side additions. MET, and measurably so.**

* The **only** delta on `crates/aion-awl` is `sha2` MOVED from
  `[dev-dependencies]` to `[dependencies]`. It is not a new crate in the
  front's graph — it was already in the crate's own dev graph and in the
  workspace lockfile, which is why **`Cargo.lock` is unchanged**. Zero new
  external edges.
* `sha2` is a **digest function**, which is none of the three things the
  fold-B axis names: no process, no runtime, no transport. The reference
  census (BRIEF-R6) *measures deployed bytes against a declared content
  digest*, so the front must hash; SHA-256 is the algorithm this workspace
  already names for content identity (`aion_package::hash`, an `.aion`
  format constraint), and minting a second one would have given the estate
  two answers to "what do these bytes hash to". The manifest comment
  records this argument at the point of the delta.
* `crates/aion-cli` needed **no new dependency at all** — the executing
  verb is built from `std` and what the CLI already links.
* **The absence of execution machinery in the front is pinned by a TEST,
  not by this paragraph.** `recipe_fire.rs`
  `the_recipe_family_opens_no_file_and_spawns_no_process` reads all fifteen
  recipe sources, strips comments, and refuses any reach for `std::fs`,
  `std::process`, `std::net`, `Command::new`, `tokio`, `reqwest`, `tonic`,
  `std::thread` or `std::env` — with a control proving the scan can find one
  and does not read prose as code.

**Verdict: the ratification holds at the bytes.** Reported as measured, not
asserted.

---

## 5. DESIGN CHANGES — new causes and new layers, each with its reasoning

Per BRIEF's refusal-causes wall, every new code is a **design change
recorded here**, layer-named, `observed_bytes` always, forbidden causes
unrepresentable at construction.

### Two new LAYERS: `Compose` and `Fire` (`CommandLayer::ALL` 4 → 6)

The assembly surface acts at two moments cut-1 had no name for. Filing a
fire-time census refusal under `Emit` would say that emitting an argv and
measuring a deployed artifact are the same discipline. They are not, and a
reader who cannot tell **"this was refused before anything ran"** from
**"this was refused at the moment of firing"** cannot tell whether the world
was touched.

* **Compose** — an assembly act was composed and refused BEFORE anything
  ran. Nothing has been fired and nothing will be.
* **Fire** — the moment of firing refused: census, precondition,
  comparison, claim.

`CommandLayer::ALL` is `[Self; 6]`; `error.rs` gains `CommandComposeError`
and `CommandFireError` through the same macro, so the six types stay
structurally identical; `error_tests.rs`'s
`every_cause_constructs_at_its_own_layer_and_nowhere_else` now proves the
misfiling refusal across all six.

### 25 new CAUSES (`CommandCause::ALL` 35 → 60)

**Parse (5)** — forms that are UNREPRESENTABLE, not discouraged:
`recipe_body_empty`, `assembly_clause_unknown`, `shell_string_body`,
`command_declaration_nested`, `parameter_kind_unknown`.

**Check (11)** — the declaration parses and what it says is wrong:
`dependency_cycle`, `reference_undeclared`, `derivation_missing`,
`extraction_unknown`, `double_carry`, `venue_value_restated`,
`artifact_digest_malformed`, `wall_allows_nothing`, `recipe_step_restated`,
`assembly_declaration_restated`, `recipe_states_no_step`.

**Compose (2)**: `slot_unfilled`, `excluded_term` (C-1).

**Fire (7)**: `census_mismatch`, `artifact_unreadable`, `restore_inexact`,
`stage_compare_mismatch`, `object_moved_before_claim`,
`precondition_unmet`, `step_command_failed`.

Two of these need their reasoning stated rather than assumed:

* **`census_mismatch` answers BOTH BRIEF-R4's "the check refuses a digest
  mismatch" and BRIEF-R6's "a census-mismatch specimen refuses the fire".**
  A second code for the same condition — declared digest ≠ observed digest,
  measured at the same moment by the same code — would be the mirror image
  of a catch-all: two names for one thing. So there is one cause, raised per
  census row with both hashes, and BRIEF-R4's separate wording is reported
  here rather than answered with a duplicate code. `artifact_digest_malformed`
  is a genuinely different, check-time ruling: a digest that could never
  match anything.
* **`step_command_failed` was minted late and deliberately.** Before it, the
  receipt printed *"ran declared command `push_branch`"* for a step the CLI
  had not actually run — a receipt asserting something that was not so. The
  cure was not to soften the wording but to add the `CommandRunner` port,
  actually run the emitted argv, and give a failed run a named refusal.
  `recipe_fire.rs` now pins the receipt line to the real argv and exit
  status.

**RED FIRST, mechanically enforced.** `tests/command_refusals.rs`
`every_cause_has_a_refusing_specimen` iterates the ENUM, so a cause minted
without a refusing specimen fails the build. All 16 parse/check causes have
document specimens in `SPECIMENS`; the 9 compose/fire causes are in
`NON_DOCUMENT_CAUSES` and are specimened in their own families
(`recipe_compose.rs`, `recipe_wall.rs`, `recipe_fire.rs`).

### One TAXONOMY that is deliberately NOT a word of the language

The four parameter kinds (`skeleton`, `per_item`, `both`, `re_derive`) are
**enumerated VALUES**, written as bare identifiers in the one position after
a parameter's type. They are **not** added to `POSITIONAL_WORDS`, because
that inventory is the OPENER vocabulary the grammar table and completion
read, and offering `skeleton` as a completion at the top of a recipe body
would describe a language the parser does not accept. They are taught in
AWL-REFERENCE §18 and in the `parameter` entry's checked example, and an
unknown kind is refused BY NAME with the admitted set printed
(`parameter_kind_unknown`). Recorded as a decision, not left implicit.
Measured ground: `both` already stands in **25** `.awl` documents of this
tree.

---

## 6. THE SURFACE, requirement by requirement

### BRIEF-R1 — recipe declarations, structurally fenced

Fifteen new POSITIONAL words, each measured over the discovered `**/*.awl`
corpus before it was chosen (`runs` 36 documents, `parameter` 20,
`requires` 12, `artifact` 8, `derives` 6, `wall`/`digest`/`fires` 3 each,
`recipe` 2, `allows`/`captures`/`extract` 1 each; only `excludes`,
`restores`, `expecting` stand nowhere). Reserving any non-zero row would
break documents that exist.

* `recipe`, `wall`, `artifact` open top-level declarations; `parameter`,
  `allows`, `excludes`, `digest`, `restores`, `runs`, `captures`, `fires`,
  `requires` open clauses; `derives`, `extract`, `expecting` stand mid-line.
* **`after` is REUSED, not re-spelled.** It already means "an ordering
  dependency on another step" everywhere else in this language; minting a
  synonym for the recipe layer would give one meaning two words. Likewise
  `step` (reserved) opens a recipe's steps and `of` (reserved) reads a
  capture's source. Confirmed at S3 as the plan's D2 required.
* **Parse, check, and round-trip** are proven by
  `recipe_declarations.rs::the_assembly_surface_round_trips_through_the_canonical_printer`:
  `print(parse(x)) == x` byte for byte, and `print ∘ parse ∘ print = print`.
* **A cyclic dependency is a check error WITH A SPAN**, and the diagnostic
  renders the cycle (`a -> b -> c -> a`). The graph walk is an
  explicit-stack DFS, pinned by a 2 000-step chain that must produce an
  order rather than a stack overflow.
* **A shell-string body is UNPARSEABLE** — three forms tested, each refused
  at the parse layer with `shell_string_body` and the review property in the
  diagnostic.
* **⛔ Amendment 1, structural, proven in BOTH directions.** Inward: all six
  `ASSEMBLY_WORDS` still refuse inside a gate-leg command declaration with
  `recipe_form_refused` and their reason, over bytes identical to BASE.
  Outward: a `command` declaration inside a recipe body is refused with
  `command_declaration_nested`, so a recipe cannot smuggle a gate-leg
  declaration into a place no reader of the top level meets — which would
  make the recipe layer reachable from a gate leg by a second route.
* **Every absorbed production has a conformance fixture.**
  `ABSORBED_PRODUCTIONS` 4 → 7 (`recipe_declaration`, `recipe_dependency`,
  `recipe_parameter_kind`), `REFUSED_FORMS` 6 → 8 (`shell_string_body`,
  `nested_command_declaration`). Five new fixtures, each carrying suite
  provenance (repo, CC0 licence, commit `b20386ab…`, a suite file digest,
  the quoted `JUST-SPECIMEN`) or `AUTHORED` with its **absence from the
  suite named** — the two authored ones say exactly why Just can carry no
  specimen (untyped parameters; no declaration nesting). The tables' own
  doc comment records that the recipe-side absorbed rows are **not** in
  contradiction with the refused rows: same shapes, two contexts, one fence
  between them.

### BRIEF-R2 (DESIGN-R4b) — parameter kinds with declared derivations

Every acceptance criterion has a test in `recipe_parameters.rs`:

* each kind parses and checks — read off `issue-investigations.README.md`
  at `b1db6ae3…`, whose own table rows are asserted at the bytes;
* a re-derive parameter without a derivation is `derivation_missing` (and a
  derivation on any other kind is refused too — a source nothing would read);
* **a derivation resolves at FIRE time**: the source is MUTATED between
  compose and fire and the fired value follows the SOURCE. The law is the
  corpus's own words, quotable from the pinned README: *"At the dispatch
  minute (not the compose minute — they drift)"*;
* the double-carry specimen refuses with the doubled value as observed
  bytes — measured in this cut's own exhibits (`leg_ids_csv` = `"build"`
  beside `legs[0].id` = `"build"`, in BOTH the KIT and the haem-wall INPUT);
* `literal_where_derived` fires against a re-derive slot fed a literal,
  cause code and wire string **unchanged from cut-1's enum** (C-2).

The extraction grammar is CLOSED to two forms (`line <n>`, `whole`);
anything else is `extraction_unknown` with the admitted set printed, and an
extraction past the end of its source refuses rather than supplying an empty
value that reads like a real one.

### BRIEF-R3 (DESIGN-R4a, L2) — slot refusal and sweep-by-claim

* **An unfilled slot refuses with the slot name in observed bytes.** This is
  `build-input.py`'s exit-2 behaviour absorbed and typed — and the
  normalization is NAMED rather than smoothed over: `sleepreap` guards its
  argument count and exits 2 while `liminal22` has **no** argv guard and
  raises an `IndexError` for the same mistake. Both become one typed
  refusal, which is what expressing them buys.
* A bound value still carrying a skeleton sentinel refuses too — all six
  sentinel shapes the corpus actually carries (`<<ISSUE_NUMBER>>`,
  `<<ISSUE_TITLE>>`, `<<TRUTHFUL_TRIGGER …>>`, `__CTL_TOKEN__`,
  `__RIG_COMMIT__`, `__CUT_COMMIT__`), with a control proving the sentinel
  rule reads a real value as a real value.
* Every unfilled slot is reported at once, not one per run.
* **SWEEP BY CLAIM**: three consumers, one source, one correction, one
  record listing all three. The record carries its **completeness claim's
  derivation** — *"assembly is FROM declarations, so a step reaches a source
  only by NAMING it and the edge set is therefore the set of declared names
  — a consumer that did not name its source would not be a consumer of
  it"* — and the sweep is proven able to reach NOTHING, so a green above is
  not merely "it always answers".

### BRIEF-R4 (DESIGN-R6) — wall projection

* **ONE declaration, TWO renderings, fixture-compared** at the haem-wall
  allowlist's pinned bytes: the prompt sentence and the anchored
  alternation `^(gates\.json|known-carries*|…|\.fleet/)` are both asserted
  in full, and every declared pattern is proven present in both.
* The enforcement rendering escapes what a path carries by ACCIDENT (`.`,
  `+`, `(`…) and leaves `*` alone, because a rig writes `known-carries*`
  meaning "and what follows" — escaping it would enforce a wall narrower
  than the one the prompt describes.
* **An excluded-term specimen refuses at projection** with the term as
  observed bytes, and names WHERE the term stands. The wall's own NAME is
  scanned too. A control proves a clean wall projects.
* **The structured diff** of two declaration states is produced by test and
  names every changed projection (`["prompt","enforcement"]`), with a
  control proving two identical states diff to nothing, and a third case
  proving a diff against an unprojectable state IS the projection refusal
  rather than a swallowed one.
* **The L6 declared-artifact kind**: an artifact carries its digest, and a
  `restores` artifact is BYTE-COMPARED after firing — `restore_inexact` when
  it is not exact, with the before-digest in the refusal. L6's honest bound
  is carried verbatim in AWL-REFERENCE §18's neighbourhood and nowhere is
  the claim widened past *"phrasing was the wrong axis here"*.
* ⛔ **The builder owns projection, never content.** Which paths a lane's
  wall admits stays authored, in the declaration. Said in the module doc and
  in §18.

### BRIEF-R5 (DESIGN-R5 + R7) — specimen firing, generic compare, receipts

* **A specimen-fire step against the known-answer red passes and its
  receipt carries the TYPED comparison** — the exhibit is
  `speech1.STAGING.md` step 4's own words, asserted at the pinned bytes.
* **A mismatch refuses the stage with BOTH SIDES' BYTES**, and the receipt
  records the comparison that stopped it.
* **Receipts are GENERATED, never typed.** `MeasuredTree::new` refuses an
  unnamed head or tree; `Receipt::new` refuses an unnamed recipe. A receipt
  that cannot say what it measured is **unrepresentable**, and the test
  asserts the three refusals by value.
* **RECEIPT-FIRST is preserved as law**: a refused fire STILL yields its
  receipt, asserted on the census-mismatch, precondition and
  stage-compare paths, and proven end to end through the binary.
* **The re-read discipline has a red test**: the object is mutated between
  write and claim and `object_moved_before_claim` fires with both digests;
  a green control proves an object that stayed put is claimable.

### BRIEF-R6 (DESIGN-R8) — declared serviceability, the reference census

* **A census-mismatch specimen refuses the fire with declared vs observed
  hashes**, and the row is in the receipt marked `DISAGREES`.
* **A fire with a clean census proceeds and its receipt lists the census
  measured**, address by address, in the order the recipe's steps reach for
  them.
* An artifact a recipe fires with **no declared digest** cannot pass the
  census — absence is a legal declared state for an artifact nobody fires,
  and is not one for an artifact a fire step reaches.
* **At-fire preconditions**: `speech1.STAGING.md` step 9's three-arm
  re-check, declared. Unanswered **and** answered-not-met both refuse with
  `precondition_unmet`.
* ⛔ **The one-truth fence, mechanical**: all eleven `DISPATCH_DECLARED`
  names refuse when RESTATED and are CONSUMABLE with no default — both
  directions, name by name, in one test.
* ⛔ **No new venue-binding hole.** Cut-1's `cwd_placeholder_unknown` still
  refuses a second placeholder and `{workspace_root}` is still admitted,
  both re-verified — and the recipe surface is proven to carry **no
  brace-placeholder machinery of its own** by reading its own sources. The
  fence is CONSUMED, never re-implemented.

### BRIEF-R7 — the composer corpus, EXPRESSIBLE (see §7 for the claim)

### BRIEF-R8 — teaching, byte-identity, estate discipline

* Fifteen new words, each with a guide sentence, a **checked** example
  (three new whole documents, all passing the real parse+check pipeline and
  pinned CANONICAL against the printer), a grammar-table position, and
  AWL-REFERENCE **§18** in **both** copies (`examples/assistant/resources/`
  and `crates/aion-awl/guide-embed/`, pinned byte-identical by the existing
  gate).
* Four new `GrammarPosition`s (`RecipeBody`, `RecipeStep`, `WallBody`,
  `ArtifactBody`) and one new `GrammarForm` (`AssemblyClause`), each
  reachable through a real cursor probe — the LSP's own reachability pin
  forced a cue to exist rather than letting the table claim a position no
  author can reach.
* **LSP surfacing**: hover was widened from a hard-coded `COMMAND_SECTION =
  17` to a per-opener lookup **read from the entries themselves**, so a
  cluster that moves section takes its clause scoping with it. Twelve of the
  fifteen words hover to the guide's own sentence and citation, DERIVED from
  the guide (a sixteenth word fails the build until the fixture carries it);
  the three that stand MID-LINE (`derives`, `extract`, `expecting`) are
  surfaced by COMPLETION with the same sentence and citation, exactly as
  cut-1's `owner` is. Cross-cluster scoping is pinned too: `digest` in a
  command body and `program` in an artifact body both hover to nothing, with
  a control proving `digest` does hover where it belongs.
* Console asset regenerated by `aion awl guide --json`; its gate is READ,
  not edited.
* **Byte-identity**: the round-trip corpus is DISCOVERED repo-wide at run
  time — **448 documents** at this HEAD, **431 baseline documents re-read
  identically**, 17 additions reported (cut-1's 12 + this cut's 5 new
  just-subset fixtures). `AWL_CORPUS_BLESS` was **never set**.
  `workflows/fleet-dev/fleet_dev.awl` is **byte-identical to BASE**.
* `cargo clippy --workspace --all-targets -- -D warnings` **clean, rc=0**,
  with **no `#[allow]`, `#[expect]`, `#[deny]`, `#[ignore]`, `_var` rename
  or `#[cfg(any())]` anywhere** — every one of the 25+ pedantic findings was
  fixed in the code (patterns nested, functions split, `write!` for string
  building, ports bundled, `panic!` → `unreachable!` in tests, `Result`s
  replaced by report structs where a step must always produce an outcome).
* `cargo fmt --all` **APPLIED** before commit. ⛔ No fmt-check gate.
* **500-line law**: every new file is under 500 code lines. Two PRE-EXISTING
  files were pushed over by this cut's additions and were split rather than
  left over: `parser/document.rs` (482 → 503 → **410**, dispatcher extracted
  to `parser/declarations.rs`) and `printer/document.rs` (482 → 531 →
  **484**, assembly rendering extracted to `printer/assembly.rs`).
  `mod.rs` files carry declarations and re-exports only. `thiserror` for
  every new error type; no `unwrap`/`expect`/`panic` in library code.

---

## 7. BRIEF-R7 — THE FIXTURE SET'S COMPLETENESS CLAIM

Every exhibit is cited by source **REPO**, **PATH** and **DIGEST**, and
**every digest is re-hashed on the run** (`recipe_corpus.rs`), so a fixture
comparing against remembered text fails rather than passes. The exhibits
are the flight tree's pinned copies at `.fleet/fixtures/corpus/`, and the
fixtures cite the OUTER path (never F-STAGE-1's nested duplicate).

**Source repo: `https://github.com/ablative-io/docs.git` at commit `3c7cbb6`.**

| clause | exhibit (flight tree) | source path (docs repo) | sha256 |
|---|---|---|---|
| R7(a) | `sleepreap.build-input.py` | `dispatch-rigs/sleepreap/build-input.py` | `032ee3d7598ac361a731a5f0917b47a6b1ea7c32c3cc3b9670b3c11c32db23b7` |
| R7(a) | `liminal22.build-input.py` | `dispatch-rigs/liminal22/build-input.py` | `b51f60f780e542dffbaa6879db2294ccd3fa050e8138b33e87e1ef3cd0d06351` |
| R7(b) | `skeleton.input.json` | `dispatch-rigs/issue-investigations/skeleton.input.json` | `4e90bc3bf2e3c2f74546c794cc4b87b183fcf60672065668720552edb2eb1218` |
| R7(b) | `issue-investigations.README.md` | `dispatch-rigs/issue-investigations/README.md` | `b1db6ae3dfa3d261a29923ed091d56657c3978fc2bf453d3a3f6ae12d973fc65` |
| R7(c) | `speech1.STAGING.md` | `dispatch-rigs/speech-1-python-wire/STAGING.md` | `1150e466b0c3264da1ac6ce93d9301287d89a492c60615b88b4b9b420f7e5f4a` |
| R7(d) | `haem-wall.INPUT-FINAL.json` | `dispatch-rigs/haem-wall/INPUT-FINAL.json` | `77f0333fcea03ada0c017af9fdea725231c3e2aecce8ade71781571daff9115d` |
| R7(e) | `speech1.KIT-input-template.json` | `dispatch-rigs/speech-1-python-wire/KIT-input-template.json` | `1a95315217b431ae7d1956183da51098e02ddc671d3e7ab737d0593df429f5ed` |

**All seven staged exhibits are expressed. Every clause (a)–(e) is answered
by at least one exhibit** — asserted, not counted by hand.

### Known corpus members EXCLUDED, each with the reason

1. **`speech1.STAGING.md` steps 7–8, the author≠reviewer handoff (lines
   28–29)** — OPERATOR LAW, not mechanism. Design §8 keeps it written words;
   the builder owns projection, never content.
2. **`speech1.STAGING.md`'s fire, verdict and landing authority (lines
   41–43)** — OPERATOR LAW. Fire authority and venue choice are the
   operator's, per Cally's clearance precedent. Absorbing them would make a
   declaration decide who may act.
3. **`speech1.STAGING.md` step 5's token mint (lines 24–25)** —
   SECURITY-ADJACENT. Minting and rotation requirements come FROM Cally
   Ray's lane and are never proposed here. The pointer stands exactly as the
   design brief carries it.
4. **`speech1.STAGING.md`'s sequencing wall** — OPERATOR LAW. A sequencing
   wall names which lane may proceed; that is a scheduling decision an owner
   makes, not a step a recipe fires.
5. **The four cut-1 gate-script specimens at
   `.fleet/fixtures/gate-scripts/`** — OUT OF CLAUSE. They are cut-1's R6
   corpus, already expressed by cut-1's gate-census fixtures. BRIEF-R7 names
   five exhibit groups and none is a gate script; the rig carries them
   because BRIEF-R5/R7 fixtures MAY cite them, and this set cites their
   family only through the known-answer red in `recipe_fire.rs`.

Each exclusion is asserted against the pinned bytes: the excluded lines are
proven to be lines the checklist actually carries, so the list cannot go
stale silently.

### Inexpressibility findings

**None found for clauses (a)–(e).** Every behaviour the five exhibit groups
carry was expressible on the declared surface, and each expression is
asserted against the original's behaviour at the bytes (refusal codes,
projected content, fill results), with normalizations named — the one
normalization being the `sleepreap`/`liminal22` argv divergence, named in §6
under BRIEF-R3.

**F-R6-1 (cut-1's, INHERITED and still OPEN):** hardening is expressible as
a value but not as a presence. This cut does not close it and did not
attempt to; it is a design question carried forward.

---

## 8. THE RATCHET RULING — the affirmative duty's outcome, and FOLD A

⛔ **The ratchet family is UNTOUCHED.** No `liminal22` or `sleepreap` script
was converted or retired; none of the five checks was absorbed. `git status`
carries no path under any ratchet rig. The cut's only contact is the
affirmative duty below.

### The named exemplar — a ratchet-SHAPED record flows through as DATA

`recipe_compare_genericity.rs` reads
`crates/aion-fleet-plan/src/error.rs` — a sealed verifier **outside this
cut's wall**, read and never modified — cited by digest and **re-hashed on
the run**:

```
repo   https://github.com/ablative-io/aion.git
path   crates/aion-fleet-plan/src/error.rs
sha256 a7354955ae389394555199b167340677d45e4217fcffea831a19ac387488e0a0
```

Its **absent-vs-malformed pair as two distinct causes** (`conclusion_missing`
/ `conclusion_malformed`, plus `conclusion_undeclared`) and a **named-hole
verdict** (`deliverable_absent`) both flow through the compare surface as
data. The test first proves each code is **FOREIGN** —
`CommandCause::from_code` answers `None` for every one of them — which is
what makes the exemplar mean something: a compare typed over cut-1's enum
could not have carried them at all.

**The ruling is NOT falsified.** The compare stays generic without absorbing
any check.

### FOLD A — genericity proven at the MECHANISM, two ways the exemplar cannot fake

1. **`the_compare_consumes_every_member_of_the_vocabulary_uniformly`**
   enumerates the full closed vocabulary **PROGRAMMATICALLY from
   `CommandCause::ALL`** — so a cause a later design change adds joins this
   test **without an amendment** — converts each into a record, and asserts
   uniform consumption member by member: every record matches itself in all
   three parts, disagrees with every other, and the layer comparison is
   proven to be a string comparison and nothing cleverer. All **60** members,
   3 540 cross-comparisons.
2. **`the_compare_surface_matches_on_no_cause_identity`** reads the compare
   surface's OWN SOURCE (`aion_awl::command::recipe::source_text()`), strips
   comment lines, and refuses any mention of `CommandCause`, of any of the
   60 wire strings, **or of the exemplar's own three foreign codes**. A
   control proves the comment stripper did not throw the source away.

That second assertion is the one fold A exists for: an implementation that
special-cased exactly the exemplar's members would pass every other test
here and be precisely the enumeration the duty forbids.

**The design consequence, recorded (plan D3):** the compare surface is typed
over a **record shape** (`VerdictRecord`: a non-empty stable code, its layer,
its observed bytes), with `CommandCause` converting **INTO** it and never out
of it. The one-way door lives in its own file (`recipe/record.rs`) precisely
so the genericity assertion above can run — a conversion beside the compare
would have made the mechanism unprovable, and an assertion that cannot run
is not an assertion.

**When line 3 lifts the five checks, they land as new PRODUCERS into an
already-generic consumer, and their per-leg signature files land as
instances of this cut's receipt kind. Line 3 touches no recipe grammar.**

---

## 9. FINDINGS

| id | finding |
|---|---|
| **C-1** | `excluded_term` is **not** in cut-1's landed enum, though BRIEF-R4 says it is. MINTED here as a recorded design change. **Changes what this cut was permitted to mint.** §0. |
| **C-2** | `literal_where_derived` diverges as a MECHANISM: cut-1's trigger is the literal's SHAPE, BRIEF-R2 needs a DECLARATION-driven one. The cause is CONSUMED unchanged and the declaration trigger added; cut-1's closed shape set is provably not widened. §1. |
| **C-4** | `grammar_parser_gate.rs`'s `DECLARATION_REFUSAL` constant MOVED with the parser's sentence. It went RED first — the fail-safe working — and the move is recorded in the file. §1. |
| **F-STAGE-1** | The rig carried a byte-identical nested duplicate of all four gate specimens at `.fleet/fixtures/gate-scripts/gate-scripts/`. Reported, **not deleted**; every fixture cites the OUTER path. §3. |
| **F-R7D-1** | BRIEF-R4 calls the haem-wall exhibit the **"×4-copies"** case. **MEASURED at the pinned bytes** (`77f0333f…`): the allowlist stands **VERBATIM TWICE** (`fix_prompt`, `legs[0].prompt`), is **referred to without restatement twice more** (`fix_prompt` again — *"nothing committed outside the allowlist"*; `judge_prompt` — *"every changed path inside the allowlist"*), and is enforced a **fifth** time by the gate script the rig names. Two verbatim copies, two references, one mechanical rendering. The fixture states what it measured and asserts the counts; the brief's phrase is reported as divergent, never rounded to fit. Two further one-declaration-many-projections pairs sit in the same file and are asserted too (`control_prompt` ≡ `counsel_control_prompt`; `control_expect` ≡ `counsel_control_expect`, both carrying the hand-copied control token). |
| **F-R6-1** | Cut-1's, INHERITED and still OPEN: hardening is expressible as a value but not as a presence. Not closed by this cut. |
| **F-CUT2-1** | `crates/aion-cli/src/main.rs` is **2 011 code lines**, over the 500-line law. **Pre-existing** (2 010 at BASE); this cut added exactly one line, `mod awl_recipe;`. Reported rather than repaired: splitting the CLI's composition root is not this cut's wall or its brief. |
| **F-CUT2-2** | `aion-cli::tail_follow_e2e tail_follow_attributes_replay_only_after_the_selected_generation_starts` **FAILED once** under full-suite parallel load (11.158 s; `tail status signal: 9 (SIGKILL)`) while three 60 s+ cargo-build tests saturated the box, and **PASSES in isolation in 0.102 s**. The test and its fixtures are **untouched by this cut** (`git status` carries no path under `crates/aion-cli/tests/tail_follow_e2e*`). Recorded as a load-sensitive pre-existing flake, not repaired — repairing another lane's e2e is outside this cut's wall. The gate's own suite run is the authority; §11 records what it said. |

---

## 10. WALLS — held

* ⛔ **Amendment 1, structural** — held and proven in both directions; cut-1's
  fence bytes are identical to BASE.
* ⛔ **Ratchet family untouched** — no script converted or retired, no check
  absorbed; the affirmative duty is the only contact and the ruling is not
  falsified.
* ⛔ **Expressibility, NOT deployment** — no composer retired, no rig
  converted, no checklist deleted, no deployed artifact moved.
  `workflows/fleet-dev/fleet_dev.awl` byte-identical to BASE. The strike
  list is the contract of the cutover, staging's to own.
* ⛔ **No shims, no compat** — the surface lands whole; nothing is added
  alongside anything.
* ⛔ **The builder owns projection, never content** — per-lane wall SETS stay
  authored in the declaration.
* ⛔ **Teardown deletion authority not widened** — this cut deletes nothing
  and declares no deletion.
* **Q5 consumed, not implemented** — venue binding stays the deploy
  surface's; no new venue-binding hole beyond `{workspace_root}`, pinned by
  test.
* **Security-adjacent** — nothing here proposes token minting or rotation.
  The mint step is a NAMED EXCLUSION of the R7 fixture set (§7), with the
  pointer to Cally Ray's lane stated.
* ⛔ **No lint/test suppressions in any spelling** — zero `#[allow]`,
  `#[expect]`, `#[deny]`, `#[ignore]`, `_var` rename or `#[cfg(any())]`
  added.
* ⛔ **No blocking sleep sized to an expected duration** — none added. The
  executing verb runs processes to completion with `Command::status()`;
  nothing is spawned in the background, so nothing needs killing, and no
  ceiling this side invented was imposed.
* ⛔ **Nothing outside the wall modified** — every changed and new path
  matches the gate's wall regex; F-STAGE-1's duplicates are left in place.

---

## 11. RECEIPTS

### Housekeeping

**Target directory cleaned before the final commit**, as the housekeeping
rule requires:

```
du -sh target   BEFORE:  20G
cargo clean            → Removed 30746 files, 20.0GiB total
du -sh target   AFTER:   0 (directory removed)
```

`target/` is gitignored (`.gitignore:2`), so the clean does not touch the
tree the gate attests. The gate run recorded below rebuilds it from cold —
that is the honest cost of cleaning before the commit rather than after —
and the flight cleans it again once the gate has spoken.

### Suite

```
cargo nextest run --workspace
Summary [211.830s] 5472 tests run: 5472 passed (6 slow, 3 leaky), 0 skipped
```

An earlier full run of the same tree reported one failure —
`aion-cli::tail_follow_e2e tail_follow_attributes_replay_only_after_the_selected_generation_starts`,
recorded as **F-CUT2-2** in §9. It passed on this run and passes in
isolation in 0.102 s. Both runs are reported; neither is hidden.

### Clippy

```
cargo clippy --workspace --all-targets -- -D warnings   →  rc=0
```

### `git ls-tree HEAD` — every deliverable path

LS_TREE_PLACEHOLDER

The report you are reading cannot quote its own blob hash. Its receipt is
`git ls-tree HEAD .fleet/reports/build.md` at the landing commit.

### Gate

GATE_PLACEHOLDER

---

## 12. REQUIREMENTS NOT COMPLETED

**None.** BRIEF-R1 through BRIEF-R8 are complete, each with the acceptance
criteria its clause names, and every divergence from the brief's own wording
is reported above as a finding rather than absorbed. The two open questions
this cut carries forward without closing — **F-R6-1** (cut-1's, inherited)
and **F-CUT2-1** (a pre-existing 500-line violation in another crate's
composition root) — are named as findings, not as thinning: neither is a
BRIEF-Rn acceptance criterion of this cut.

— the build leg, rocketfish.
