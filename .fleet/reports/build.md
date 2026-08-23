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
| **F-CUT2-3** | `crates/aion-cli/tests/embedded_assistant_launchable_e2e.rs` spawned the real `aion` binary with **no `AION_HOME`**, so the shipped document's `cwd "{workspace_root}"` resolved through the operator's ambient `$HOME`. Two consequences, both defects: the test's verdict was a property of the BOX (green on a desk, red under any runner whose child environment carries no `HOME`), and passing it CREATED `clones/` inside the operator's real `~/.aion` as a side effect. **This is the red the gate fired**, and it is IN THE WALL. Fixed at its cause in fix round 1: every `aion` spawn in the file now states a test-owned `AION_HOME` (a `tempfile::TempDir` thrown away with the test), which is the idiom the other twelve `AION_HOME`-stating e2e files in this crate already use. §13. |
| **F-RIG-2** | The fleet's `gates` action runs `gate-entry.sh` as an AWL **declared body**, whose environment aion clears to `PATH` alone — so `HOME` never reaches the gate under the fleet, though it does at every desk. 69 workspace tests refuse without it; 67 of them are OUTSIDE this cut's wall and byte-identical to BASE. **F-RIG-class: the staged wall cannot contain the fix.** Amended by the minimal exact delta — a step 0 that states the home the rest of the toolchain already resolves through passwd, and only when the caller supplied none. **The WALL regex is NOT touched and no out-of-wall byte is modified.** §13. |
| **F-SRV-1** | `aion_server::config::aion_home` reads `$HOME` directly with **no passwd fallback** (`crates/aion-server/src/config/home.rs:62`), while `cargo` and `rustup` on the same box resolve their homes through the passwd entry and are unaffected by an absent `HOME`. That asymmetry is what made round 1's red look like a tree defect. Reported with bytes, **NOT patched**: `crates/aion-server` is outside this cut's wall and the question — whether an aion home should fall back to the passwd entry — is aion-server's lane to rule, not this cut's to decide in passing. §13. |
| **F-CUT2-4** | Three PRE-EXISTING load-sensitive failures were observed across four whole-suite runs of the fixed tree, each in a crate this cut does not touch, each ONCE or TWICE, none reproducible in isolation, and none present in the run that ended `5472 passed`. Same class as F-CUT2-2 and recorded the same way — not repaired, because repairing another lane's timing-sensitive tests is outside this cut's wall. §13.5 names all four runs and every failure verbatim. |
| **F-SRV-2** | **A real product race, found while diagnosing F-CUT2-4 and reported rather than patched.** `ConnectedWorkerRegistry::wait_for_worker` (`crates/aion-server/src/worker/registry.rs:908`) is `self.worker_arrived.notified().await`, and its doc contract at `:903–907` promises *"Returns immediately if a registration occurred since the last call."* The notifying side is `self.worker_arrived.notify_waiters();` (`:890`). **`notify_waiters()` stores no permit** — it wakes only tasks ALREADY parked in `notified()` — so the contract is false: a registration that lands between a caller's registry re-check and its `.notified().await` is LOST, and the dispatch parks unbounded until some *other* worker registers. `dispatch.rs:321` and `:473` are the two dispatch-path callers. That is what `aion-server::unserved_park_cluster_event the_grpc_outbox_leg_announces_its_park` catches intermittently: it passes in **0.005 s** in isolation (3/3) and hits its 30 s `EVENT_DEADLINE` only when it happens to be scheduled beside the three 40–95 s outbox e2e tests. **NOT patched**: `crates/aion-server` is outside this cut's wall, and a wake-primitive change in a durable engine's dispatch path is not a drive-by from a language-surface cut — `notify_one` wakes exactly one waiter where `notify_waiters` wakes all, so the correct repair (a generation counter, or `watch`) is aion-server's lane to design. §13.5. |

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
tree the gate attests. The gate run recorded below rebuilt it from cold —
that is the honest cost of cleaning before the commit rather than after —
and the flight cleans it again once the gate has spoken for the last time.

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

```
$ git ls-tree HEAD -- <every deliverable path>
100644 blob a47367839cd6d9842eb92d36cf691dc734d4468f	apps/aion-ops-console/src/features/authoring/assets/awl-guide.json
100644 blob 98a077f1d18fcfedf9f88b4cbdb85632441e4bf1	crates/aion-awl-lsp/src/completion.rs
100644 blob 4e6292c621bdf8c615e04f9872a5bfa780feea2b	crates/aion-awl-lsp/src/glossary.rs
100644 blob c513da93b6086fe21d65340f308c11a945b64253	crates/aion-awl/Cargo.toml
100644 blob 566345c5481472510ae2429272b5635a9320a141	crates/aion-awl/guide-embed/AWL-REFERENCE.md
100644 blob 8d63731488de3f1e821c1de467b60ad4c4d3d533	crates/aion-awl/src/command/cause.rs
100644 blob 97e8db2370d4a1306fb583ce0552f663e4784aad	crates/aion-awl/src/command/error.rs
100644 blob 673cedb46e96aea0ac53708c5eccffdd6fb71925	crates/aion-awl/src/command/just_subset.rs
100644 blob caab78990ecf8cce2a339616c909bd29086df483	crates/aion-awl/src/command/mod.rs
040000 tree 80b9d39955974c4e04888e7512ad31ea34a71488	crates/aion-awl/src/command/recipe
100644 blob b3a65a654482c31d0c4e70bbdc05cdc3c0fd57e2	crates/aion-awl/src/guide/entry.rs
100644 blob 3bcff46dce004b3db5b26233f24573821bea98d9	crates/aion-awl/src/guide/examples/decl_recipes.rs
100644 blob dbda87db299de8d73a01e863ff8ca3a42e2ed25d	crates/aion-awl/src/guide/grammar.rs
100644 blob 1d998931f937c984a64957ffdbda869cd0b94de2	crates/aion-awl/src/guide/lookup.rs
100644 blob b6a5505fe202407db605395bab4b42209c8d0015	crates/aion-awl/src/guide/recipes.rs
100644 blob 74f1083870da47c5ae4d187cbd245f2c09ab3faf	crates/aion-awl/src/lexer/tokens.rs
100644 blob 3a168f80c5d463d9e97fbbc70b54323c92d8685e	crates/aion-awl/src/parser/declarations.rs
100644 blob 95ec54bb352c873d5f08244c613e669282135e98	crates/aion-awl/src/printer/assembly.rs
100644 blob 39b38ede89555870e9e9aa50c965f54dd8456204	crates/aion-awl/tests/command_refusals.rs
040000 tree f4a8fe3238ca6392e450d4623afd2579442e51b3	crates/aion-awl/tests/recipe
100644 blob c38623fb5266b07cddb24a50770e9c158123a4d5	crates/aion-awl/tests/recipe_compare_genericity.rs
100644 blob fb2826c6c8ab83af19539e77860867e0b372a1ec	crates/aion-awl/tests/recipe_compose.rs
100644 blob cb8bffec60e710f295ce2324afd724212345f035	crates/aion-awl/tests/recipe_corpus.rs
100644 blob d54248ecfcb18a2674a32e818b12e25a1ef19ca3	crates/aion-awl/tests/recipe_declarations.rs
100644 blob 430520cfdd780972a19fe57bccb3f2c4189f9d56	crates/aion-awl/tests/recipe_fire.rs
100644 blob c426213637b89461d94239f03bf67cf50a7c0786	crates/aion-awl/tests/recipe_parameters.rs
100644 blob a6292152930cb4f93bf1ea530d97d30ea66968fa	crates/aion-awl/tests/recipe_wall.rs
100644 blob 2d3015734b093ffe99a36b62a84a6fcffcb9bcb3	crates/aion-cli/src/awl.rs
040000 tree 757a4e7029f3955ea59cc88484d403b475c2e0ea	crates/aion-cli/src/awl_recipe
100644 blob 0429340a33852acd2e7aaa715797393a8a1b061d	crates/aion-cli/tests/awl_recipe_verb_e2e.rs
100644 blob 566345c5481472510ae2429272b5635a9320a141	examples/assistant/resources/AWL-REFERENCE.md

$ git ls-tree HEAD -- crates/aion-awl/src/command/recipe/ (the surface, file by file)
100644 blob ee8f412dd2764a3d309f8ec1d76e2d79bb967e1c	crates/aion-awl/src/command/recipe/census.rs
100644 blob 615e37f985b201ebb5b376d8e4c66c9d9557587d	crates/aion-awl/src/command/recipe/check.rs
100644 blob 5cb8d0687ca258b6b16d10dde8a0a2d7d2615060	crates/aion-awl/src/command/recipe/compare.rs
100644 blob bb96201c60bfb8c5487bc13cca8af893e574f802	crates/aion-awl/src/command/recipe/compose.rs
100644 blob 7f1701bbcacea4437f5f80b93bbda59af0af946f	crates/aion-awl/src/command/recipe/decl.rs
100644 blob 0eaac792448ad3b85ab4e8a4cd62c372c3bd4139	crates/aion-awl/src/command/recipe/derivation.rs
100644 blob 75af787531bfaf101ea49fddd22be43a77211f18	crates/aion-awl/src/command/recipe/fire.rs
100644 blob 3765fb672432c3bba952ccb5e0a06cbf257ce8ec	crates/aion-awl/src/command/recipe/graph.rs
100644 blob 25fc1e35ee546d0c3db2a3e5a7c88e123eeab03f	crates/aion-awl/src/command/recipe/mod.rs
100644 blob 0968f76959a9ed44df8bb432e14dd8e3c15b53a7	crates/aion-awl/src/command/recipe/parse.rs
100644 blob 6bb59aa491d83b9f70e3a506c4ca3a655c61177a	crates/aion-awl/src/command/recipe/print.rs
100644 blob 1d795d51e27f21a4d339f24c11b98a437f8e81b0	crates/aion-awl/src/command/recipe/project.rs
100644 blob ef264c64fdc4944740cc96146cac2dd9ca0c8df6	crates/aion-awl/src/command/recipe/receipt.rs
100644 blob 0031c915b30eb8ab39ae5844826bfd5f637539e3	crates/aion-awl/src/command/recipe/record.rs
100644 blob 5cb4d5248ad4b312d46b35c7918cc894314a56eb	crates/aion-awl/src/command/recipe/sweep.rs
100644 blob 7d026d5a8be948433909d24c1bbe9194eb5be958	crates/aion-awl/src/command/recipe/words.rs

$ git ls-tree HEAD -- crates/aion-cli/src/awl_recipe/ (the executing verb)
100644 blob 0334dda4740f17b3af35e51eabc73aa170ac0a6f	crates/aion-cli/src/awl_recipe/args.rs
100644 blob 25fc9c7dab24540df90b4243410c1b2de5b32099	crates/aion-cli/src/awl_recipe/firer.rs
100644 blob d2af18a94f4c87935a2d59dca0dcdfc47391634c	crates/aion-cli/src/awl_recipe/mod.rs
100644 blob 07c8481be3d16ca40c01976f868f7b3732034dda	crates/aion-cli/src/awl_recipe/run.rs
100644 blob f24b9063f72bd245c601c37a62e6e9284356b11c	crates/aion-cli/src/awl_recipe/tree.rs

$ git ls-tree HEAD -- crates/aion-awl/tests/fixtures/command/just-subset/ (recipe-side conformance fixtures)
100644 blob c92d573de56063365500f522bf182de402590df8	crates/aion-awl/tests/fixtures/command/just-subset/absorbed-recipe-declaration.awl
100644 blob c3c54dc31b2da4763a9e2479f40c475616c6a7f2	crates/aion-awl/tests/fixtures/command/just-subset/absorbed-recipe-dependency.awl
100644 blob 8ab879006809c4797a0ed70f0784abb19a406832	crates/aion-awl/tests/fixtures/command/just-subset/absorbed-recipe-parameter-kind.awl
100644 blob 09124d5529503cc10a8b9500d524a09272cf49f5	crates/aion-awl/tests/fixtures/command/just-subset/refusal-nested-command-declaration.awl
100644 blob b3e6e9af64cd995483f7916d0aac38e7b9ed924c	crates/aion-awl/tests/fixtures/command/just-subset/refusal-recipe-body.awl
100644 blob ec78b861f9bc70a25f3a09277c7b7ff23598a8b7	crates/aion-awl/tests/fixtures/command/just-subset/refusal-shell-string-body.awl
```

The report you are reading cannot quote its own blob hash. Its receipt is
`git ls-tree HEAD .fleet/reports/build.md` at the landing commit.

### Gate

`.fleet/gate-entry.sh`, run by this leg on the CLEAN COMMITTED tree at
`dc25d7607ae597d924ac4c82e0b28626f8b310cf` — the commit carrying the whole
surface. Compilation lines elided; nothing else is:

```
== 1. toolchain ==
  pinned: 1.97.1
  active: rustc 1.97.1 (8bab26f4f 2026-07-14)
  source: 1.97.1-x86_64-unknown-linux-gnu (overridden by '/home/rocketfish/fleet/runs/awl-cut2-flight1/legs/build/rust-toolchain.toml')
  pin HONORED: active toolchain is sourced from the pin file
== 2. tree receipt ==
  HEAD: dc25d7607ae597d924ac4c82e0b28626f8b310cf
  TREE: CLEAN
== 3. paths census ==
  census OK (91 files within the wall, 59 in the deliverable crate family)
== 4a. clippy ==
  clippy clean
== 4b. new-test control ==
  new test annotations in diff: 71
== 4c. suite ==
  suite names ~5472 test(s)
     Summary [ 209.619s] 5472 tests run: 5472 passed (7 slow, 3 leaky), 0 skipped
GATE-GREEN: toolchain 1.97.1 pin-honored from rust-toolchain.toml; tree CLEAN at dc25d7607ae597d924ac4c82e0b28626f8b310cf; census within wall with deliverable present; clippy -D warnings clean; 71 new test annotations; 5472 tests named and green against base 1bab95477cb86d498d07bb8da445f00351b87972
```

`GATE_RC=0`.

⚠️ **Stated plainly, because a receipt that overstates itself is worse than
none.** The run above attests the commit that carries the SURFACE. This
report is a second commit on top of it, and the exit-condition run — the
gate against the final tree, this file included — is the flight's terminal
green and is reported in the leg's reply rather than quoted here: a document
cannot contain the output of a run that measures it. Every `git ls-tree`
receipt above is valid at the final HEAD as well, because none of the paths
it names changed between the two commits.

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

---

## 13. FIX ROUND 1 — the gate's red, its cause, and the amendment

Appended by the fix round on top of the build leg's report. The build leg's
sections above are unchanged; this section is the round's own record.

### 13.1 The red, verbatim

`.fleet/gates/RED-LATEST.md`, committed by the runner at `6f451330`
("gates: red round 1 — findings verbatim"). Steps 0–4b were GREEN in that
same run — pin HONORED from `rust-toolchain.toml`, TREE CLEAN at
`1689708d2033aa66180a76012a1dff274780fcfb`, `census OK (93 files within the
wall, 59 in the deliverable crate family)`, `clippy clean`, `new test
annotations in diff: 71`. The suite is the only step that fired:

```
        FAIL [   0.081s] ( 889/5472) aion-cli::embedded_assistant_launchable_e2e the_embedded_document_is_launchable_by_the_recommended_verb
        FAIL [   0.113s] ( 890/5472) aion-cli::embedded_assistant_launchable_e2e the_recommended_serve_invocation_reaches_the_dial_not_a_refusal
warning: 4568/5472 tests were not run due to test failure (run with --no-fail-fast to run all tests, or run with --max-fail)
error: test run failed
GATE-RED: cargo nextest run --workspace rc=100 (named cause: suite_red)
```

Both failures carry the SAME stderr — one cause, two arms:

```
error: the `harness` section of /home/rocketfish/fleet/runs/awl-cut2-flight1/repo/crates/aion-cli/../aion-server/assistant-embed/assistant.awl sets `cwd` to `{workspace_root}`, which asks this box for its workspace root: the aion home cannot be resolved, so there is no workspace root: configuration error: cannot resolve Aion home: set AION_HOME or HOME
```

### 13.2 The cause, measured — not inferred from the message alone

`cannot resolve Aion home: set AION_HOME or HOME` has exactly ONE source and
exactly one branch that produces it — `crates/aion-server/src/config/home.rs:62`,
reached only when `AION_HOME` is absent **and** `HOME` is absent-or-empty:

```rust
None => {
    let Some(home) = std::env::var_os("HOME").filter(|value| !value.is_empty()) else {
        return Err(ServerError::Config {
            message: "cannot resolve Aion home: set AION_HOME or HOME".to_owned(),
        });
    };
```

The test spawns the real binary with no `.env_clear()` and no `.env(...)`, so
the child's environment is the gate's own. **The gate ran without a `HOME`.**

Reproduced byte-for-byte on the flight tree at the redded commit:

```
$ env -u HOME -u AION_HOME CARGO_HOME=… RUSTUP_HOME=… \
    cargo nextest run --workspace -E 'test(the_embedded_document_is_launchable_by_the_recommended_verb)'
        FAIL [   0.039s] (1/1) aion-cli::embedded_assistant_launchable_e2e the_embedded_document_is_launchable_by_the_recommended_verb
    …  error: the `harness` section of …/assistant.awl sets `cwd` to `{workspace_root}`, … cannot resolve Aion home: set AION_HOME or HOME
```

— the same two tests, the same stderr, and among tests 1..904 (all the redded
run reached before nextest's fail-fast cancelled it) **exactly those two and
no others**. With `HOME` present, both pass in isolation and the build leg's
own gate run of the identical script reported `5472 tests run: 5472 passed`
(§11). The discriminator is the environment, and nothing else.

**Why the gate has no `HOME`, named at its bytes.** The fleet's `gates` action
is a declared AWL body:

```
  action gates(workspace_root: String, run_name: String, gate_entry: String) -> OpProbe
    run json "sh /home/rocketfish/fleet/bin/fleet-gates.sh $workspace_root $run_name $gate_entry"
```

and aion clears a declared body's environment to `PATH` alone —
`crates/aion-worker/src/shell/action.rs:178`:

```rust
// The host's environment does not cross into a declared command. See
// `INHERITED_VARIABLE`: only PATH does, and only so a bare program
// name resolves.
command.env_clear();
if let Some(path) = std::env::var_os(INHERITED_VARIABLE) {
```

with `const INHERITED_VARIABLE: &str = "PATH";` (line 63). The `fleet_ops_rf`
worker declares no `env` for that action, and `fleet-gates.sh` adds none —
it runs `sh "$gate_entry"` with what it was given. So the gate's whole
environment is `PATH`. This is aion's determinism law working exactly as
designed; the rig simply never stated the environment its suite needs.

**Why steps 1–4a survived it.** `cargo` and `rustup` resolve their homes
through the passwd entry when `HOME` is absent, so the pin was honored and
400+ registry crates compiled — measured directly:

```
$ env -i PATH=/home/rocketfish/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/bin sh -c \
    'echo "HOME=[${HOME:-UNSET}]"; rustc --version; rustup show active-toolchain; cargo --version'
HOME=[UNSET]
rustc 1.97.1 (8bab26f4f 2026-07-14)
1.97.1-x86_64-unknown-linux-gnu (overridden by '/home/rocketfish/fleet/runs/awl-cut2-flight1/repo/rust-toolchain.toml')
cargo 1.97.1 (c980f4866 2026-06-30)
```

`aion_home` does NOT fall back that way (F-SRV-1). That asymmetry is the whole
of it: the toolchain never noticed, the suite did.

### 13.3 The full exposure, measured — why the wall cannot contain the fix

nextest's fail-fast cancelled round 1 at test 890, so the red named 2 of the
failures and left 4 568 tests unrun. Measured with `--no-fail-fast` in the
reproduced environment:

```
$ env -u HOME -u AION_HOME CARGO_HOME=… RUSTUP_HOME=… cargo nextest run --workspace --no-fail-fast
     Summary [ 210.203s] 5472 tests run: 5403 passed (6 slow), 69 failed, 0 skipped
```

All 69 carry the same `cannot resolve Aion home` refusal. By crate:

| crate | failures | in this cut's wall? |
|---|---|---|
| `crates/aion-server` (`config::load::tests` ×54, `::haematite_backend` ×5, `::run_server_outbox_e2e` ×2) | **61** | NO |
| `crates/aion-integration-norn` (`claim_area::tests`, `harness::tests`, `::declared_environment_e2e`) | **6** | NO |
| `crates/aion-cli` (`::embedded_assistant_launchable_e2e`) | **2** | YES |

And the 67 out-of-wall failures are **BASE's own bytes**, not this cut's:

```
$ git diff --no-ext-diff --stat 1bab95477cb86d498d07bb8da445f00351b87972...HEAD \
    -- crates/aion-server crates/aion-integration-norn
$ echo $?
0
```

— empty. Neither crate is touched by this cut, so those 67 tests fail
identically on the staged BASE. **Round 1's red measured the rig's
environment, never this cut's tree.** That is the F-RIG-class condition
named in the fix-round instruction: a pre-existing test forces an edit
outside the staged wall.

### 13.4 What was fixed, and what was amended

**(a) The cause INSIDE the wall — fixed, not suppressed.** F-CUT2-3.
`crates/aion-cli/tests/embedded_assistant_launchable_e2e.rs` now states the
home it means, in all four `aion` spawns across its three launching arms:

```rust
fn test_home() -> Result<tempfile::TempDir, Box<dyn std::error::Error>> {
    Ok(tempfile::Builder::new()
        .prefix("aion-209-home-")
        .tempdir()?)
}
…
        .env("AION_HOME", home)
```

This is the idiom twelve other e2e files in the same crate already use
(`launcher_command.rs:61`, `dev_hot_reload_e2e.rs:191`, `one_motion_live_e2e.rs:122`,
…). It fixes a real defect independent of the gate: the test previously
created `clones/` inside the operator's own `~/.aion` whenever it passed.
Verified in the reproduced environment:

```
$ env -u HOME -u AION_HOME … cargo nextest run --workspace -E 'binary(embedded_assistant_launchable_e2e)' --no-fail-fast
        PASS [   0.003s] (1/4) … the_spike_carries_the_embedded_prompt_constants_verbatim
        PASS [   0.038s] (2/4) … the_embedded_document_is_launchable_by_the_recommended_verb
        PASS [   0.072s] (3/4) … the_same_document_without_its_harness_section_is_refused
        PASS [   0.105s] (4/4) … the_recommended_serve_invocation_reaches_the_dial_not_a_refusal
     Summary [   0.106s] 4 tests run: 4 passed, 0 skipped
```

**(b) The rig — AMENDED by the minimal exact delta.** F-RIG-2. The other 67
failures cannot be fixed inside the wall and are not this cut's to repair, so
the amendment is to `.fleet/gate-entry.sh` (which IS in the wall) and to
nothing else. It adds a step **0. environment** that supplies the home the
rest of the toolchain already resolves through passwd, **only when the caller
supplied none**:

```sh
echo "== 0. environment =="
if [ -n "${HOME:-}" ]; then
  echo "  HOME supplied by the caller: ${HOME}"
else
  HOME=$(getent passwd "$(id -u)" | cut -d: -f6)
  [ -n "${HOME}" ] || red "the caller passed no HOME and the passwd entry for uid $(id -u) names no home directory, so the workspace suite cannot resolve an aion home (named cause: environment_homeless)"
  [ -d "${HOME}" ] || red "the passwd home ${HOME} is not a directory, so the workspace suite cannot resolve an aion home (named cause: environment_homeless)"
  export HOME
  echo "  HOME absent from the caller's environment (aion clears a declared body's env to PATH); supplied from the passwd entry: ${HOME}"
fi
```

plus the rationale block above it and one line in the header's order list
(`0 environment`). **The exact delta is that and nothing else** — measured:

```
$ git diff --no-ext-diff --stat 731fa74a -- .fleet/gate-entry.sh
 .fleet/gate-entry.sh | 35 +++++++++++++++++++++++++++++++++++
```

What the amendment does NOT do, stated so a re-gate can check it:

* The **`WALL` regex is byte-identical** to the staged one. No path is
  admitted that was not admitted at staging.
* `BASE`, `PKG_HINT`, and steps 1–4 are byte-identical to the staged script.
  The census, the positive control, the pin-honored assertion, the new-test
  control and the suite invocation are untouched.
* Nothing outside the wall is modified: the fix round's whole diff is
  `.fleet/gate-entry.sh`, `.fleet/reports/build.md`, and
  `crates/aion-cli/tests/embedded_assistant_launchable_e2e.rs`.
* It is **not a suppression in any spelling** — no `#[ignore]`, no `--skip`,
  no filter, no `allow`/`expect`, no retry. Every one of the 5 472 tests still
  runs and must still pass. The environment is *stated* rather than
  *inherited*, which is strictly stronger than the status quo: before the
  amendment the same script returned GREEN at the build leg's desk and RED
  under the fleet **on identical bytes**, and a gate whose verdict depends on
  who invoked it is not a gate.

**(c) Deliberately NOT done.** The wall was not widened, `crates/aion-server`
and `crates/aion-integration-norn` were not touched, and `aion_home`'s missing
passwd fallback was not patched. F-SRV-1 records that question with its bytes
for aion-server's own lane; adopting 67 of another lane's tests to answer an
environment problem is not a minimal delta and is not this cut's brief.

### 13.5 Verification after the fix — four whole-suite runs, all four reported

Every run below is `cargo nextest run --workspace` on the fixed tree
(`904ae176`) in the REPRODUCED gate environment — `env -i PATH=…`, exactly
what aion hands a declared body — with `HOME` reaching it only through the
step-0 amendment. Reported in full, including the two that went red, because
a verification that quotes only its green runs is not a verification.

| # | invocation | result |
|---|---|---|
| A | `--no-fail-fast`, HOME UNSET (the round-1 environment, pre-fix tree) | `5472 tests run: 5403 passed (6 slow), 69 failed` — the 69 aion-home refusals of §13.3 |
| B | the gate itself (fail-fast) | RED at test 3262: `aion-rs::awl_bc4_differential adversarial::adversarial_differential_is_byte_identical` |
| C | `--no-fail-fast` | `5472 tests run: 5470 passed (7 slow, 3 leaky), 2 failed` |
| D | the gate itself (fail-fast) | RED at test 5468: `aion-server::unserved_park_cluster_event the_grpc_outbox_leg_announces_its_park` |
| E | `--no-fail-fast` | **`5472 tests run: 5472 passed (6 slow, 3 leaky), 0 skipped`, RC=0** |

**What the amendment proved.** Across runs C, D and E — 16 416 test
executions — there is **not one** `cannot resolve Aion home` refusal
anywhere, including the 61 `crates/aion-server` tests and the 6
`crates/aion-integration-norn` tests that run A refused. Run A died at test
890 under the gate's fail-fast; runs B–E reach 3 262, 5 472, 5 468 and 5 472.
The step-0 environment delta does exactly what it claims and nothing else.

**The failures that remain, named verbatim.** Four distinct tests, in three
crates, none of them this cut's (F-CUT2-4):

```
B: FAIL (3262/5472) aion-rs::awl_bc4_differential adversarial::adversarial_differential_is_byte_identical
     DIVERGENCE header-types/valid/max_arity_record event[0] : reference="disposition=Completed" direct="disposition=Parked"
C: FAIL (3324/5472) aion-rs::engine_query query_racing_completion_yields_payload_or_typed_error_without_events
C: FAIL (5468/5472) aion-server::unserved_park_cluster_event the_grpc_outbox_leg_announces_its_park
D: FAIL (5468/5472) aion-server::unserved_park_cluster_event the_grpc_outbox_leg_announces_its_park
     Error: "released outbox-leg dispatch did not resolve"
```

Each is measured, not waved at:

* **`adversarial_differential_is_byte_identical`** — passes in **4.755 s** in
  isolation and passed in runs A (21.337 s), C and E. Its divergence is
  `disposition=Parked` on a fixture whose lowered MIR carries **no timer at
  all** (`grep -ci 'timer\|sleep\|timeout'
  crates/aion-awl/tests/mir-goldens/header-types/valid/max_arity_record.mir`
  → `0`), while `run.rs`'s only `Parked` producer (`:164`) requires a
  `TimerStarted` with no `TimerFired`. The fixture's own bytes therefore do
  NOT explain the classification, and this report does not invent a mechanism
  it did not measure — the observation is handed to aion-rs's lane as it
  stands. The same file already documents one load-sensitive
  `Parked`-misclassification of this family (`covered.rs:114`).
* **`query_racing_completion_yields_payload_or_typed_error_without_events`** —
  seen once, in C, in 0.250 s. The test's own name states the race it drives.
* **`the_grpc_outbox_leg_announces_its_park`** — the one that repeated (C and
  D, both at exactly 30.01 s, both at scheduling position 5468, beside the
  three 40 s/90 s/95 s `outbox_delivery_contract_e2e` tests). It passes in
  **0.005 s** in isolation (3 runs of 3) and passed at position 5029 in runs
  A and E. Diagnosed to its bytes as **F-SRV-2**, a genuine lost-wakeup in
  `ConnectedWorkerRegistry` — a real defect this round found and reported
  rather than papered over.

**None of the three is this cut's.** All three crates are byte-identical to
BASE:

```
$ git diff --no-ext-diff --stat 1bab95477cb86d498d07bb8da445f00351b87972...HEAD \
    -- crates/aion crates/aion-server crates/aion-integration-norn
$ echo $?
0
```

and the build leg's own two runs of this identical suite were `5472 passed`
and `5472 passed` (§11). Run E reproduces that on the fixed tree. Repairing
another lane's timing-sensitive tests — or its dispatch-path wake primitive —
is not a minimal delta and is not this cut's brief; F-CUT2-4 and F-SRV-2
carry them to the lanes that own them, with the bytes to act on.

⚠️ **Stated plainly.** Runs A–E attest commit `904ae176`. This section is a
second commit on top of it, so the terminal gate run — the one that measures
the tree containing THIS paragraph — is reported in the round's reply, not
quoted here: a document cannot contain the output of a run that measures it.

## 14. FIX ROUND 2 — the differential's park classifier, and F-RIG-3

Appended by the second fix round. §§1–13 are unchanged; this section is round
2's own record.

### 14.1 The red, verbatim

`.fleet/gates/RED-LATEST.md` (= `red-2.md`, byte-identical, md5
`978a15ccb8a78f681c7b667a3cf25a8b`), committed by the runner at `35808d94`
("gates: red round 2 — findings verbatim"). Steps 0–4b were GREEN in that same
run — `HOME absent from the caller's environment … supplied from the passwd
entry: /home/rocketfish` (round 1's F-RIG-2 amendment doing exactly its job),
`pin HONORED`, `TREE: CLEAN` at `31f616a864f58d890734f831c97a3b2b43735281`,
`census OK (97 files within the wall, 59 in the deliverable crate family)`,
`clippy clean`, `new test annotations in diff: 71`. Round 1's 69 aion-home
refusals did not recur. One test fired:

```
        FAIL [   6.669s] (3244/5472) aion-rs::awl_bc4_differential covered::flagship_awl_hello_matches

    thread 'covered::flagship_awl_hello_matches' (3344632) panicked at crates/aion/tests/awl_bc4_differential/covered.rs:431:5:
    flagship not clean:
    differential report: 0 succeeded, 0 failed-path, 0 cancelled, 0 timer-parked, 0 excluded, 0 refused, 0 AGREED-REFUSAL, 1 DIVERGENCES, 0 INFRA
      DIVERGENCE flagship/valid/awl_hello event[0] : reference="disposition=Parked" direct="disposition=Completed"

     Summary [ 143.404s] 3259/5472 tests run: 3258 passed (3 slow, 3 leaky), 1 failed, 0 skipped
error: test run failed
GATE-RED: cargo nextest run --workspace rc=100 (named cause: suite_red)
```

This is the family §13.5 reported as F-CUT2-4 and handed on: run B there was
`DIVERGENCE header-types/valid/max_arity_record event[0] :
reference="disposition=Completed" direct="disposition=Parked"` — the same
divergence, the other way round, on a different fixture. §13.5 recorded that
the fixture's own bytes did not explain the classification and declined to
invent a mechanism. **That was the right call on the evidence then held and the
wrong disposition of the finding**: the mechanism is real, it is in the
oracle's own classifier, and this round measured it. Both rounds are the same
defect.

### 14.2 The cause, measured at the bytes

`Disposition::Parked` has exactly one producer,
`crates/aion/tests/awl_bc4_differential/run.rs`, and it requires three things
at once — no terminal event, `is_running` true, and a non-empty
`pending_timers`:

```rust
        if !pending.is_empty()
            && stable >= STABLE_READS
            && is_running(&engine, workflow_type, handle.workflow_id()).await?
        {
            break outcome(history, Disposition::Parked);
        }
```

`flagship/valid/awl_hello` is `name |> greet |> .greeting |> shout |> route
shouted`. It declares no `sleep`, no `wait`, no timeout;
`grep -ci 'timer\|sleep\|timeout'
crates/aion-awl/tests/mir-goldens/flagship/valid/awl_hello.mir` → `0`. So the
first question is where a pending timer came from at all. **Dumped from the
reference run's actual trail** (temporary instrumentation, since removed):

```
REF[0] WorkflowStarted { envelope: { seq: 1, recorded_at: …49.438423740Z, … }, workflow_type: "awl_hello", … run_id: RunId(4377b823-1c7a-4dcd-b4a1-d3c99ac25d5f) … }
REF[1] TimerStarted   { envelope: { seq: 2, recorded_at: …49.438423740Z, … }, timer_id: TimerId(Named("deadline:4377b823-1c7a-4dcd-b4a1-d3c99ac25d5f")), fire_at: …50:49.438423740Z }
REF[2] ActivityScheduled … greet
…
REF[8] WorkflowCompleted { seq: 9 … }
REF[9] TimerCancelled  { seq: 10, timer_id: TimerId(Named("deadline:4377b823-…")), cause: WorkflowIntent }
```

`ref_timers=["timer:named:deadline:4377b823-…"]` — **a run that COMPLETED
reports an outstanding timer**. Two distinct defects meet there:

1. **Every fixture in this corpus arms a run deadline.** The differential's
   reference project declares a workflow timeout on *every* entry —
   `crates/aion/tests/awl_bc4_differential/harness.rs:338`, inside
   `write_workflow_toml`:

   ```
   "[[workflow]]\nentry_module = \"{module}\"\nentry_function = \"run\"\n\
    timeout_seconds = 60\ninput_schema = …
   ```

   so `Package::declared_timeout()` is `Some(60s)` for all 112, and
   `record_declared_deadline` (`crates/aion/src/lifecycle/start.rs:413-430`)
   appends the reserved `deadline:{run_id}` `TimerStarted` at **seq 2** of
   every single run, 58 s beyond the oracle's own 15 s `RUN_DEADLINE` — so it
   never fires and is outstanding for the entire life of every run.

2. **`pending_timers` was blind to cancellation.** It read only `TimerFired`:

   ```rust
   let fired: Vec<&TimerId> = history.iter().filter_map(|event| match event {
       Event::TimerFired { timer_id, .. } => Some(timer_id), _ => None }).collect();
   ```

   but a terminal transition retires the deadline by **cancelling** it
   (`aion::time::retire_run_deadline`; `TimerCancelled` at REF[9] above), never
   by firing it. Hence a completed run reporting a live timer.

Put together: the timer half of "positive park evidence" was satisfied by
**every fixture, from its second event to its terminal**, and `is_running` is
true for any non-terminal run by definition. The whole discrimination between
*parked* and *merely slow* therefore rested on `stable >= STABLE_READS` —
three consecutive unchanged reads at `POLL = 20ms`, i.e. **60 ms of history
silence**. A box running 16 nextest processes, several of them booting BEAM
VMs and running `gleam build`, hands 60 ms of silence to any run at all. The
gate's own numbers say so: this test took **6.669 s** in the red run against
**0.912 s** in isolation at this desk.

That is the full mechanism, and it is symmetric — whichever backend loses the
CPU is the one that reads as `Parked`, which is exactly why round 1 saw
`direct=Parked` and round 2 saw `reference=Parked`.

**The deductive step is closed, not assumed.** `awl_hello`'s trail carries
exactly ONE `TimerStarted` in its entire history, the deadline. When the gate
classified that run `Parked`, the pending set it matched on could not have been
anything else. Nothing here is inferred from the message alone.

### 14.3 The mechanism, demonstrated deterministically

Reproducing the race on demand is a coin toss (25 flagship runs under 28 CPU
spinners: 0 failures; 12 runs pinned to one contended core: 0 failures — the
box has to be loaded the way a whole-workspace run loads it). So it was
demonstrated by **removing the timing term and changing nothing else** —
`STABLE_READS: 3 → 0`, one constant, on the otherwise-unmodified pre-fix tree:

```
$ sed -i 's/const STABLE_READS: u32 = 3;/const STABLE_READS: u32 = 0;/' …/run.rs
$ cargo nextest run -p aion-rs --test awl_bc4_differential -E 'test(flagship_awl_hello_matches)'
    panicked at crates/aion/tests/awl_bc4_differential/covered.rs:436:5:
    differential report: 0 succeeded, … 1 timer-parked, … 0 DIVERGENCES, 0 INFRA
    test result: FAILED. 0 passed; 1 failed
```

A fixture with no timer in its lowering, classified **timer-parked**, with the
timing guard as the only thing that had been holding it back. The same control
on the FIXED tree:

```
$ # STABLE_READS = 0 again, post-fix
$ cargo nextest run -p aion-rs --test awl_bc4_differential -E 'test(flagship_awl_hello_matches)'
    test result: ok. 1 passed; 0 failed
```

The fix does not make the flake rarer. With the timing guard **entirely
removed** the misclassification is unreachable, because no fixture without a
timer of its own has any park evidence left to match.

Also measured, and reported because it is the honest bound on the above: under
28 spinners, a probe that logged the park predicate instead of taking it fired
4 158 times across the covered set, on exactly **three** workflow types —
`confirm_gate`, `wait_and_sleep`, `wait_timeout_optional`, every hit carrying
`pending=["timer:named:deadline:<run>", "timer:anonymous:0"]`. Those are the
three genuinely parked fixtures, parking on their own `timer:anonymous:0`. CPU
contention alone did not starve a completing fixture for 60 ms at this desk;
the fleet's mixed load did, twice.

### 14.4 The fix

`crates/aion/tests/awl_bc4_differential/run.rs`, and nothing else in the tree.

* **`pending_timers` now retires last-event-wins over BOTH retirement events**,
  `TimerFired` *and* `TimerCancelled` — the same law
  `aion::time::outstanding_deadline_timer` already applies to the deadline it
  recovers (`crates/aion/src/time/deadline.rs:79-98`). A completed run no
  longer reports a live timer.
* **`park_evidence` is new and is what the park decision now consumes**: the
  outstanding timers *minus* the engine's run deadline, tested with
  `aion::time::is_deadline_timer`. The `deadline:` prefix is engine-minted
  only — the author NIF choke point `decode_timer_id_arg` refuses any
  author-supplied timer name under it (`crates/aion/src/time/deadline.rs:24-28`)
  — so a `deadline:` timer in history is *always* the per-run deadline and
  never a workflow timer. Excluding it cannot hide a park a workflow authored.
  That is a guarantee of the engine's own identifier discipline, not a
  heuristic.
* **The RECORD is deliberately unchanged.** `RunOutcome::pending_timers` still
  carries the full outstanding set, deadline included, so the two backends
  still compare their whole timer sets and `EXPECTED_TIMER_PARKED`'s pinned
  evidence — `["timer:anonymous:0", "timer:named:deadline:<run>"]` for all
  three parked fixtures — is byte-identical to what it was. The park DECISION
  narrowed; the park EVIDENCE did not.

**Red-first, as the brief requires.** Five tests land in
`run::park_evidence_tests`, and against the pre-fix rules four of them fail:

```
        PASS (1/5) … a_fired_timer_is_not_outstanding
        FAIL (2/5) … a_cancelled_timer_is_not_outstanding
        FAIL (3/5) … a_workflow_started_timer_is_park_evidence_beside_the_deadline
             left: [TimerId(Named("deadline:00000000-0000-0000-0000-000000002222")), TimerId(Anonymous(0))]
        FAIL (4/5) … the_engine_run_deadline_is_never_park_evidence
        FAIL (5/5) … a_timer_restarted_after_retirement_is_outstanding_again
     Summary 5 tests run: 1 passed, 4 failed
```

The one that passes pre-fix is the rule that was already right (a fired timer
is retired), kept under test so the retirement rewrite cannot silently lose it.
On the fixed tree all five pass, and so do all 42 tests in the binary,
including `covered_set_is_byte_identical` over the full 112-fixture corpus with
its bucket inventory and pinned park evidence:

```
        PASS [   1.220s] (36/42) aion-rs::awl_bc4_differential covered::flagship_awl_hello_matches
        PASS [  80.471s] (42/42) aion-rs::awl_bc4_differential covered::covered_set_is_byte_identical
     Summary [  80.471s] 42 tests run: 42 passed (1 slow), 0 skipped
```

**It is not a suppression in any spelling.** No `#[ignore]`, no `--skip`, no
filter, no `allow`/`expect`, no retry, no fixture removed from the oracle, no
`EXCLUDED_OUT_OF_ORACLE` entry, no widened tolerance, no sleep. Every one of
the 5 472 tests still runs and must still pass, and the corpus's park bucket
still asserts its exact three names with their exact timer identities. What
changed is that a run must now present evidence of a park to be called parked.

### 14.5 F-RIG-3 — the wall amendment, by the minimal exact delta

**The finding.** The staged wall cannot contain this fix:

```
^(crates/aion-awl|crates/aion-cli|apps/aion-ops-console/…/awl-guide\.json|examples/assistant/resources/AWL-REFERENCE\.md|Cargo\.toml|Cargo\.lock|\.fleet/)
```

`crates/aion/tests/awl_bc4_differential/run.rs` matches no alternative
(`crates/aion-awl` and `crates/aion-cli` are prefixes of *hyphenated* siblings;
neither is a prefix of `crates/aion/`). And no in-wall byte can fix the defect:
the cause is the oracle's classifier, `crates/aion-awl` holds only the fixture
and the compiler, and neither of those is wrong — `awl_hello` lowers correctly
on both backends, which is precisely why the *trails* never diverged and only
the *disposition label* did. Editing the fixture to dodge a harness defect, or
adding it to `EXCLUDED_OUT_OF_ORACLE`, would be suppression. This is the
F-RIG-class condition named in the fix-round instruction: **a pre-existing test
forces an edit outside the staged wall.**

**The amendment.** One alternative added to `WALL` in `.fleet/gate-entry.sh`,
spelled out as a full path — not the crate, not the test directory, not a
prefix:

```
+crates/aion/tests/awl_bc4_differential/run\.rs
```

plus the rationale block above it. Measured:

```
$ git diff --no-ext-diff --stat -- .fleet/gate-entry.sh
 .fleet/gate-entry.sh | 22 +++++++++++++++++++++-
```

(21 of those lines are the comment block; one is the `WALL` line.)

What the amendment does NOT do, stated so a re-gate can check it:

* `BASE`, `PKG_HINT`, step 0 (round 1's F-RIG-2 block), and steps 1–4 are
  **byte-identical** to the round-1 script. The census logic, the positive
  control, the pin-honored assertion, the new-test control and the suite
  invocation are untouched.
* No other path is admitted. `crates/aion` remains out of bounds everywhere
  else — `crates/aion/src/**` and every other file under
  `crates/aion/tests/**` still red-fire `wall_breach`.
* The escapee set is **exactly** the one file admitted, measured against the
  staged wall over the whole round (committed diff + working tree):

  ```
  $ { git diff --no-ext-diff --name-only ${BASE}...HEAD; git status --porcelain | awk '{print $2}'; } \
      | sort -u | grep -Ev "<STAGED WALL>"
  crates/aion/tests/awl_bc4_differential/run.rs
  ```

### 14.6 A SECOND red, found by this round's verification and fixed at its cause

The first whole-workspace run of the fixed tree did **not** go green, and the
test that fired was not the one round 2 named:

```
        FAIL [  11.142s] (2990/5477) aion-cli::tail_follow_e2e tail_follow_attributes_replay_only_after_the_selected_generation_starts

    thread '…' panicked at crates/aion-cli/tests/tail_follow_e2e/fixtures.rs:206:5:
    selected-run attempt 2 was suppressed after prior replay poisoned current attempts; selected-generation child workflow socket was not followed; selected-run attempt 2 live transcript was not printed; stdout: ["[parent 00000000 activity:activity:7 attempt:1] retained-backfill-parent: {\"line\":\"visible through retained HTTP backfill\"}"]; tail status signal: 9 (SIGKILL); stderr:
```

It passed in **0.144 s** at position 1003 in round 2's own gate run, passes in
**0.101 s** in isolation, and is byte-identical to BASE
(`git diff --no-ext-diff --stat ${BASE}...HEAD -- crates/aion-cli/tests/tail_follow_e2e*`
is empty). It is IN the wall — `crates/aion-cli` is a staged alternative — so
it needed no amendment, only a diagnosis.

**The cause: a lost wakeup, not a tight timeout.** The fixture's handshakes ran
on `tokio::sync::Notify::notify_waiters`, which stores **no permit**: a
notification that arrives before the other side has registered is discarded and
the waiter blocks to its timeout. The parent socket handler announces its own
attach and only THEN starts waiting for the test's release:

```rust
        state.parent_attached.notify_waiters();
        state.emit_fanout.notified().await;      // crates/aion-cli/tests/tail_follow_e2e.rs:418-419
```

while the test, woken by that announcement, immediately calls
`emit_fanout.notify_waiters()`. On a 16-way run the handler can be preempted
between those two lines, the release lands with nobody registered, and the
handler waits forever — so it never sends the replayed parent events, the tail
never opens the selected child socket or the selected parent attempt's
transcript socket, and the test's two 5 s timeouts both expire. 5 + 5 + the
1 s line drain is the 11.142 s in the receipt above; the SIGKILL is the test's
own teardown, not a crash.

**Reproduced deterministically, both ways.** Rather than re-rolling the dice,
the adverse ordering was forced — one `sleep(400ms)` inserted between the
handler's announce and its wait, nothing else changed:

```
PRE-FIX handshake, adverse ordering forced:
        FAIL [  11.060s] (1/1) aion-cli::tail_follow_e2e tail_follow_attributes_replay_only_after_the_selected_generation_starts
    selected-run attempt 2 was suppressed after prior replay poisoned current attempts; selected-generation child workflow socket was not followed; selected-run attempt 2 live transcript was not printed; stdout: ["[parent 00000000 activity:activity:7 attempt:1] retained-backfill-parent: …"]; tail status signal: 9 (SIGKILL); stderr:

POST-FIX handshake, SAME forced ordering:
        PASS [   0.502s] (1/1) aion-cli::tail_follow_e2e tail_follow_attributes_replay_only_after_the_selected_generation_starts
```

The pre-fix reproduction is the observed failure **byte-for-byte** — same three
violations, same captured stdout, same `signal: 9 (SIGKILL)`, 11.060 s against
the suite's 11.142 s. That closes it as diagnosis rather than conjecture.

**The fix, at the primitive.** No timeout was lengthened, no retry added, no
sleep left behind. Each handshake is now the primitive its waiter count
demands:

| signal | direction | waiters | before | after |
|---|---|---|---|---|
| `parent_attached` | server → test | 1 | `notify_waiters` | `notify_one` (stores a permit) |
| `emit_fanout` | test → server | 1 | `notify_waiters` | `notify_one` (stores a permit) |
| `emit_live` | test → server | **2** | `notify_waiters` | `watch::channel` (latched) |

`emit_live` is the one that could not become `notify_one`: two transcript
sockets wait on it (the child's and the selected parent attempt's), and
`notify_one` would wake exactly one of them. A `watch` latches its value, so
every receiver observes the emit whenever it asks — early or late, one waiter
or ten. The send is now also checked (`send(true)?`) instead of being a
fire-and-forget that cannot fail loudly, and the receiver's dropped-sender case
is handled explicitly rather than swallowed.

Verified: 8 consecutive runs of the binary, `2 tests run: 2 passed` each,
0.101–0.109 s; `cargo clippy -p aion-cli --all-targets -- -D warnings` clean.

**F-CLI-1.** Recorded as a finding against a pre-existing surface this cut did
not author: three `notify_waiters` rendezvous in `tail_follow_e2e` were lost
wakeups, latent since the file was written and reachable by any sufficiently
loaded run. It is the same defect CLASS as F-SRV-2 from §13.5 (a lost wakeup in
`ConnectedWorkerRegistry`) — different crate, same primitive misuse. Worth the
owning lane's sweep for other `notify_waiters` rendezvous.

### 14.7 Verification — both whole-suite runs, both reported

Both runs are `cargo nextest run --workspace` in the REPRODUCED gate
environment — `env -i PATH=…`, exactly what aion hands a declared body, with
`HOME` reaching it the way step 0's F-RIG-2 amendment supplies it. The red one
is quoted because a verification that quotes only its green run is not a
verification.

| # | tree | result |
|---|---|---|
| F | the `run.rs` park-evidence fix alone | `3005/5477 tests run: 3004 passed (4 slow, 3 leaky), 1 failed` — the F-CLI-1 lost wakeup of §14.6, at test 2990, `2472/5477 tests were not run due to test failure` |
| G | both fixes | **`5477 tests run: 5477 passed (6 slow, 3 leaky), 0 skipped`, RC=0** |

Run F is the one that found the second defect; it is reported as it landed,
including the 2 472 tests its fail-fast never reached. Run G is the whole
workspace green end to end, 5 477 of 5 477, on the tree this commit records.

The count moved 5 472 → **5 477**: the five `run::park_evidence_tests` this
round adds. No test was removed, skipped, filtered or ignored — `0 skipped` on
both runs, and the round-2 gate's own `0 skipped` for comparison.

Named individually, because they are the tests the two rounds were about — and
note the timing on the first line: **6.785 s**, against the **6.669 s** it took
while going red in round 2's gate. The run that answers the red was as loaded
as the run that produced it. The fix is not the box being quieter.

```
        PASS [   6.785s] (3251/5477) aion-rs::awl_bc4_differential covered::flagship_awl_hello_matches
        PASS [ 113.726s] (5475/5477) aion-rs::awl_bc4_differential covered::covered_set_is_byte_identical
        PASS [  14.000s] (3249/5477) aion-rs::awl_bc4_differential adversarial::adversarial_differential_is_byte_identical
        PASS [   0.150s] (1005/5477) aion-cli::tail_follow_e2e tail_follow_attributes_replay_only_after_the_selected_generation_starts
        PASS [   0.049s] (3194/5477) … run::park_evidence_tests::a_cancelled_timer_is_not_outstanding
        PASS [   0.028s] (3195/5477) … run::park_evidence_tests::a_fired_timer_is_not_outstanding
        PASS [   0.033s] (3196/5477) … run::park_evidence_tests::a_timer_restarted_after_retirement_is_outstanding_again
        PASS [   0.046s] (3197/5477) … run::park_evidence_tests::a_workflow_started_timer_is_park_evidence_beside_the_deadline
        PASS [   0.047s] (3200/5477) … run::park_evidence_tests::the_engine_run_deadline_is_never_park_evidence
```

The other two tests §13.5's F-CUT2-4 carried forward also passed in run G, in a
run that reached every one of them —
`engine_query::query_racing_completion_yields_payload_or_typed_error_without_events`
at `0.935s` (3333/5477) and
`aion-server::unserved_park_cluster_event the_grpc_outbox_leg_announces_its_park`
at `0.017s` (5034/5477). The latter is F-SRV-2's lost wakeup: it did not fire
in this round's runs, nothing here fixes it, and it stands reported to
aion-server's lane exactly as §13.5 left it. Passing is not evidence that a
race is absent, and this report does not claim otherwise.

### 14.8 Findings

* **F-CUT2-4 — re-dispositioned, not re-filed.** §13.5 carried the
  `awl_bc4_differential` divergence to aion-rs's lane as an unexplained
  load-sensitive observation. It is now diagnosed and fixed at its cause here.
  The two other tests named in F-CUT2-4 —
  `engine_query::query_racing_completion_yields_payload_or_typed_error_without_events`
  (seen once, in one run of five) and
  `aion-server::unserved_park_cluster_event::the_grpc_outbox_leg_announces_its_park`
  (F-SRV-2, diagnosed to a lost wakeup in `ConnectedWorkerRegistry`) are
  **untouched and still stand as reported**: both are out of wall, in crates
  byte-identical to BASE, and neither appeared in round 2's gate.
* **F-RIG-3 — the staged wall could not contain a pre-existing test's own
  defect.** Recorded above with its bytes and its exact delta. The wall's
  cut-1 lesson (F-RIG-1: admit byte-pinned twins up front) generalises: a wall
  drawn around a deliverable also has to admit whatever *pre-existing test of
  another crate* the deliverable's own suite runs, because the gate runs the
  whole workspace. Cut 2 has now paid that twice — F-RIG-2 for the
  environment, F-RIG-3 for the classifier.
* **F-CLI-1 — three lost-wakeup rendezvous in `tail_follow_e2e`, fixed here.**
  In wall, so no amendment; diagnosed and reproduced deterministically in
  §14.6. Latent since the file was written, and the same primitive misuse as
  F-SRV-2 — worth the owning lane sweeping its other `notify_waiters`
  rendezvous.
* **F-AION-1 (out of wall, to aion-rs's lane, not fixed here).** The
  differential's reference project declares `timeout_seconds = 60` on every
  entry (`harness.rs:338`) whether or not the fixture authored a timeout. That
  is what makes `header-types/valid/workflow_timeout`'s *own* declared timeout
  invisible to the differential: the manifest the reference project writes
  overrides it, so the fixture proves its lowering and not its timeout
  semantics. Not this cut's to change — moving it would move the corpus's
  recorded trails — but it is named here with its bytes so the owning lane can
  decide.

### 14.9 Requirements — no movement

No BRIEF-R1..R8 requirement changed in this round. **Not one byte of the
deliverable moved**: `crates/aion-awl` is byte-identical to the tree round 2
gated, as is every `crates/aion-cli` SOURCE file — the whole of
`crates/aion-cli/src/awl_recipe/`, `src/awl.rs`, `src/main.rs`, and the
`awl_recipe_verb_e2e` suite. §§6, 7, 8 and 12 stand exactly as written. This
round touched three files, two of them pre-existing tests of surfaces this cut
does not author, and the gate script:

```
 .fleet/gate-entry.sh                          |  22 +-
 crates/aion-cli/tests/tail_follow_e2e.rs      |  95 ++++++--
 crates/aion/tests/awl_bc4_differential/run.rs | 297 +++++++++++++++++++++++---
```

Of those, only `crates/aion/tests/awl_bc4_differential/run.rs` needed the
F-RIG-3 wall amendment; `crates/aion-cli/tests/tail_follow_e2e.rs` was already
inside the staged wall.

`cargo fmt --all` applied before commit (autoformat discipline; ⛔ no fmt
gate). `cargo clippy --workspace --all-targets -- -D warnings` clean.

⚠️ **Stated plainly, as in §13.5.** This section is part of the commit that
answers round 2's red, so the terminal gate run — the one measuring the tree
that contains THIS paragraph — is reported in the round's reply, not quoted
here: a document cannot contain the output of a run that measures it.
