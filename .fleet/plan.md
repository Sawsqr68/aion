# PLAN — awl-cut2 flight1, build leg (recipes / staging assembly)

Authored by the flight before any implementation, against `.fleet/BRIEF.md`
(Calliope, owner-grilled by Vesper, folds A/B/C in place) and `.fleet/RIG.md`
(staged by Waffles). Every requirement below is grounded at `file:line` in
**this** tree — the tree whose HEAD is cut-1's landing plus the cut-2 rig, not
the tree the design brief's seams were derived on.

⛔ **Citation discipline (BRIEF fold C), binding on every line below.**
`BRIEF-Rn` is this cut's brief (`.fleet/BRIEF.md`, R1–R8). `DESIGN-Rn` is the
design brief's own numbering (R1–R8 there too). A bare `Rn` is two different
requirements and is never written. Cut-1's brief is cited as `CUT1-Rn`
(retrievable at `git show 1bab9547:.fleet/BRIEF.md`).

**Tree receipt at planning time.** HEAD `731fa74a04afafb2b2190483a6b2edba96769b5f`,
working tree CLEAN. BASE `1bab95477cb86d498d07bb8da445f00351b87972` verified an
ancestor of HEAD; `git diff --no-ext-diff --name-only BASE...HEAD` names exactly
**15 files, every one under `.fleet/`** (brief, rig, control token, seven corpus
exhibits, four nested gate-script copies, the gate). No source file has moved
since the landing. The base contains cut-1's landing commit `1bab9547`
(`land: awl-cut1`), which is what BRIEF's cut-1 relationship clause requires.

**Toolchain receipt.** `rust-toolchain.toml:2` pins `channel = "1.97.1"`;
`rustc --version` = `1.97.1 (8bab26f4f 2026-07-14)`; `rustup show active-toolchain`
= `1.97.1-x86_64-unknown-linux-gnu (overridden by '…/awl-cut2-flight1/repo/rust-toolchain.toml')`
— pin HONORED from the pin file, not default-coincidence, which is the
discrimination `.fleet/gate-entry.sh:54–63` requires on this venue.

**Exhibit receipt.** All **seven** BRIEF-R7 corpus exhibits under
`.fleet/fixtures/corpus/` re-hashed and matched against the RIG.md table byte
for byte (`032ee3d7…`, `b51f60f7…`, `4e90bc3b…`, `b1db6ae3…`, `1150e466…`,
`77f0333f…`, `1a953152…`). All **four** cut-1 gate specimens under
`.fleet/fixtures/gate-scripts/` re-hashed and matched (`1e1e3ad2…`,
`ae66bb0b…`, `ef5b82a8…`, `e1544910…`). Fixtures cite these bytes, never
remembered text.

---

## §0 — FIRST BINDING ACT: CONFORMANCE READ of the landed cut-1 surface

The brief's own words: *"The flight's first act is a CONFORMANCE READ of the
landed cut-1 surface against `9ec0a8f`."* Read here against the landed bytes at
HEAD and against cut-1's brief as it landed in the tree
(`git show 1bab9547:.fleet/BRIEF.md`, 282 lines — CUT1-R1…R8 whole). Every
shape BRIEF cites from cut 1, one row per shape, verdict at the bytes:

| consumed shape (BRIEF's name) | landed at | verdict |
|---|---|---|
| command-declaration **types** | `crates/aion-awl/src/command/decl.rs:44` `CommandDecl` (+`CommandParam:100`, `ProgramDecl:118`, `EndOfOptionsClause:149`, `LeadingDashSpecimen:159`, `TypedFill:197`, `CwdDecl:231`, `HardeningDecl:246`, `CommandTimeout:293`), exported `src/command/mod.rs:21–25` | **CONFORMS.** Typed throughout; no field is a string a shell re-parses. Recipe bodies (BRIEF-R1) can be typed over `CommandDecl` by name reference. |
| the **admits-`--`** property | `decl.rs:135` `END_OF_OPTIONS_MARKER = "--"`, `decl.rs:149` `EndOfOptionsClause { admits: bool }`; unstated middle refused by `CommandCause::EndOfOptionsUnstated` (`cause.rs:113`); emitter places the marker unconditionally (`emit.rs:141`) | **CONFORMS**, and the declared unit is the INVOCATION (program + subcommand), per cut-1's F-UNIT-1 (`.fleet/reports/build.md:660`) and `decl.rs:113–116`. BRIEF cites no contrary shape. |
| the **refusal cause enum** | `cause.rs:76` `CommandCause`, `cause.rs:186` `ALL: [Self; 35]`, `cause.rs:234` `layer()` exhaustive, `cause.rs:275` `as_str()` wire strings, `cause.rs:322` `from_code()` (unknown string ⇒ `None`, never a nearest match); layers `cause.rs:36` `CommandLayer` with `ALL: [Self; 4]` at `:49`; misfiling unrepresentable — `CommandRefusal::refuse` derives the layer FROM the cause (`error.rs:253`) and `CauseMisfiled` (`error.rs:36`) refuses a foreign layer at construction | **CONFORMS** as a closed, layer-named vocabulary — **with one DIVERGENCE, below (C-1).** |
| **`literal_where_derived`** | cause `cause.rs:128`, wire string `"literal_where_derived"` (`cause.rs:294`), raised at `check_fills.rs:243` over `derived.rs:65 derived_shape_in`, three closed shapes `derived.rs:28` (40-hex git object, 64-hex digest, control token) | **CONFORMS as a cause; DIVERGES as a trigger — C-2 below.** |
| the **fixture harness** | `crates/aion-awl/tests/command_just_subset.rs` — corpus DISCOVERED not enumerated (`:55`), header claims parsed from `//!` lines (`:41`), provenance mandatory (`:150`), expectation honoured (`:201`), refused table and fixtures name the same causes (`:255`); tables `src/command/just_subset.rs:59 ABSORBED_PRODUCTIONS[4]` and `:106 REFUSED_FORMS[6]` | **CONFORMS.** BRIEF-R1's "conformance fixture per cut-1 R5's discipline" has a live harness to extend: adding a recipe-side production to `ABSORBED_PRODUCTIONS` mechanically demands its fixture (`command_just_subset.rs:123`). |

**C-1 — DIVERGENCE (reportable finding, first line of the report).**
BRIEF-R4 says the excluded-term refusal is *"`excluded_term`, cut-1's enum —
consumed, not re-minted."* **`excluded_term` is not in cut-1's enum.** Measured:
a whole-tree search for `excluded_term|ExcludedTerm` (excluding `.git/`,
`target/`, `node_modules/`) returns exactly **one** hit — `.fleet/BRIEF.md:215`,
the brief describing it. `CommandCause::ALL` (`cause.rs:186`) carries 35
members and none is it. **Disposition, per the brief's own wall** (*"every new
cause this cut needs joins cut-1's closed enum BY DESIGN CHANGE recorded in the
report"*): the cut MINTS `excluded_term` into `CommandCause` as a recorded
design change, layer-named, with its red specimen — and reports the divergence
against BRIEF-R4's "consumed" wording rather than silently absorbing it. No
source is fought: the design's §6 law (new codes are a design change) governs
and is followed.

**C-2 — DIVERGENCE OF MECHANISM, not of cause (reportable).** Cut-1's
`literal_where_derived` fires on the literal's SHAPE (`derived.rs:65`, scanned
word-by-word over three closed shapes). BRIEF-R2's acceptance requires it to
fire *"against a re-derive slot fed a literal"* — a DECLARATION-driven trigger:
a slot whose parameter declares a derivation refuses ANY hand literal, whether
or not it looks derived. The two are not the same instrument; `"main"` in a
re-derive slot is invisible to `derived_shape_in` and must refuse under
BRIEF-R2. **Disposition:** this cut adds the declaration-side trigger and
**consumes the cause code unchanged** (`CommandCause::LiteralWhereDerived`,
wire string `literal_where_derived`, `cause.rs:294`) — no re-mint, no second
code, no widening of `derived.rs`'s closed shape set. Recorded because BRIEF-R2
reads as though the trigger already exists; it does not, and saying so is the
conformance read's job.

**C-3 — the amendment-1 fence is LOAD-BEARING and must survive this cut.**
Cut-1 landed the structural refusal BRIEF-R1 must preserve: `ASSEMBLY_WORDS`
(`src/command/words.rs:46` — `recipe`, `depends`, `needs`, `after`, `steps`,
`then`) refused inside a command body by name with its reason
(`parse.rs:229 assembly_refusal`), cause `RecipeFormRefused` (`cause.rs:86`),
red-pinned by fixture `tests/fixtures/command/just-subset/refusal-recipe-body.awl:17`
and by `REFUSED_FORMS[0..2]` (`just_subset.rs:106–126`). BRIEF-R1 makes
recipes representable at the TOP LEVEL while these must go on refusing INSIDE a
gate-leg command declaration, byte-identically. That is the whole of amendment
1 and it is a red test that already exists — the cut must not move it.

**C-4 — no other cut-1 shape this brief cites has moved.** `git diff BASE...HEAD`
touches no file under `crates/`, so the landed bytes read in this section are
the bytes cut-1's own gate went green on.

---

## §1 — SECOND BINDING ACT: HAS THIS ALREADY HAPPENED? (the census, fresh)

⛔ **Pathspec, stated (an undeclared corpus is not a census).** The whole
worktree at HEAD `731fa74a`, excluding exactly three classes and nothing else:
`.git/`, `target/`, `node_modules/`. Text files only (`grep -I`). Where a
construct is a language-surface question, the narrower pathspec is named in the
row. This census is over **THIS** cut's constructs and was run fresh: cut-1's
census answered a different question on a tree that has since moved.

| construct sought (BRIEF's own five) | pathspec | result |
|---|---|---|
| a **recipe / dependency construct** in the AWL word tables | `crates/aion-awl/src/{lexer,parser,ast,command,guide}/**` + whole-tree `-w recipe` | **ZERO as a construct.** `POSITIONAL_WORDS` (`lexer/tokens.rs:108`) is `[&str; 19]` and carries no `recipe`, `needs`, `depends`, `steps`; `Keyword::ALL` (`tokens.rs:283`) is `[Self; 58]` and carries none either; the top-level dispatch (`parser/document.rs:437–516`) admits exactly `const`/`type`/`worker`/`child`/`subflow`/`step`/`command`. Every in-tree `recipe` hit is either **cut-1's refusal machinery** (triaged as C-3: `words.rs:47`, `cause.rs:86`, `just_subset.rs:65,110`, `parse.rs:229`, the refusal fixture) or the English word "recipe" used in MIR codegen prose (`src/mir/func.rs:36,51`, `src/mir/lower/codec*.rs`, `src/mir/select/shells.rs:164,464`) — a template-shape noun, not a language construct. Neither satisfies nor blocks BRIEF-R1. |
| any **wall-projection or prompt-assembly machinery** | whole worktree, `-i` on `wall_projection\|project_wall\|WallDecl\|prompt_assembly\|assemble_prompt\|PromptBuilder\|excluded.term\|negative vocabulary` | **ZERO.** The only `excluded_term` in the tree is `.fleet/BRIEF.md:215` (see C-1). Every `-w wall` hit in `crates/` is unrelated: `wall`-*clock* determinism prose (`aion-awl-package/tests/awl_assemble.rs:108,130`, `aion-cli/tests/authoring_quality_e2e.rs:8,130`) or cut-1's **`haem-wall` fixture NAME** (`tests/command_gate_census.rs:37`, `tests/fixtures/command/gate-census/haem-wall*.params`) — a rig's name, not projection machinery. Nothing in the tree derives a prompt or a gate rendering from a declaration. |
| a **sweep-by-claim** mechanism | whole worktree, `-i` on `sweep_by_claim\|sweep-by-claim\|SweepRecord\|sweep(` | **ZERO.** Three hits, all triaged: an author-domain action named `sweep` in a deliberately-invalid rev2 fixture (`crates/aion-awl/tests/fixtures/rev2/loop-outcomes/invalid/pipe_route_in_loop_body.awl:12–16`); `plan_sweep` in the server's worker backpressure (`crates/aion-server/src/worker/backpressure.rs:169,177,492` — a queue sweep, different domain, **outside this cut's wall**); a round-robin test name (`outbox_dispatcher.rs:2060`). None re-projects consumers from a corrected source declaration. |
| a **reference-census / content-hash fire-precondition** surface | `crates/**` on `reference_census\|ReferenceCensus\|fire_precondition\|precondition`; `crates/aion-awl*/src` on `content_hash\|ContentHash` | **ZERO in the front.** `crates/aion-awl/src` and `crates/aion-awl-lsp/src` contain **no** content-hash surface at all. Two adjacent hits triaged: `mir/lower/driver.rs:83 preconditions(...)` is a LOWERING precondition (document shape before codegen), a different sense; and `crates/aion-package/src/hash.rs:89 content_hash(beams)` hashes a BEAM set for module namespacing (CLAUDE.md invariant 5) — real prior art for the ALGORITHM, and already a dependency of the front (`crates/aion-awl/Cargo.toml:19`), but it answers "what is this package's identity", never "do the deployed bytes still match what a recipe declared". Neither satisfies BRIEF-R6. |
| any **staging-receipt generator** | whole worktree, `-i receipt` over `crates/` and `scripts/` | **ZERO as a generator.** All 30+ `receipt` hits in `crates/` are the author-domain NOUN in AWL examples and fixtures (`src/guide/examples/values.rs:94–144`, `examples/flow_graph.rs:149–160`, `tests/printer_roundtrip.rs:123`, `src/parser/send.rs:33`, `mir/deferred_tests.rs:226`) — a user's `type Receipt { ok: Bool }`, not a record of a run. Nothing in the tree writes a receipt that names the tree, head and clean/dirty state it measured. |

**One further adjacent surface, triaged because it is the nearest prior art and
a reader must not think this cut duplicates it.** `crates/aion-fleet-plan` —
`src/verify.rs:55 ExactCommit`, `:90 Verdict`, `:125 verify_conclusion`, and
`src/error.rs:60 ConclusionCause` (exactly three codes: `conclusion_missing`,
`conclusion_malformed`, `conclusion_undeclared`, `:75 as_str`) — is a SEALED
VERIFIER of a stage-conclusion artifact at an exact commit. It is **not** a
recipe surface, **not** a receipt generator, and it is **outside this cut's
wall** (`.fleet/gate-entry.sh:33` admits `crates/aion-awl`, `crates/aion-cli`,
two assets, the two manifests and `.fleet/` — not `crates/aion-fleet-plan`).
It is read as prior art in two places and modified in none: its
absent-vs-malformed pair is the ratchet-SHAPED record BRIEF-R5's affirmative
duty names, available as DATA without absorbing any check.

**The census answer: five constructs sought, five zeros, every hit triaged.**
This section is carried into `.fleet/reports/build.md` verbatim.

---

## §2 — STAGING CONFORMANCE (measured, not assumed)

A rig's claims are claims about bytes. Three measured, before any work:

1. **The gate's three-place delta claim HOLDS EXACTLY.**
   `git diff 1bab9547 731fa74a -- .fleet/gate-entry.sh` = 9 insertions,
   33 deletions, of which exactly **two** are executable lines: `BASE`
   (`5e2fb5a3e…` → `1bab9547…`) and `WALL` (gains `crates/aion-cli`); the rest
   is the cut-1 F-RIG-1 comment block replaced by the cut-2 wall rationale
   (`.fleet/gate-entry.sh:25–32`). Cut-1's F-RIG-1 amendment
   (`examples/assistant/resources/AWL-REFERENCE\.md`) is **carried** in the
   staged wall, so this cut inherits a consistent wall and needs no amendment
   of its own for the reference twin. Positive control unchanged
   (`gate-entry.sh:91` still requires ≥1 changed file under `crates/aion-awl/`).
2. **Seam movement: NONE.** `git diff BASE...HEAD` names 15 files, all under
   `.fleet/`. No AWL-family source, no teaching asset, no manifest has moved
   between the base and this HEAD, so every cut-1 citation re-verified in §0 is
   a citation against the bytes cut-1's gate measured.
3. **F-STAGE-1 (observation, reportable, NOT to be silently repaired).** The
   rig commit `731fa74a` carried a **second, nested copy** of all four gate
   specimens at `.fleet/fixtures/gate-scripts/gate-scripts/*.gate-entry.sh`.
   Measured: byte-identical to the outer four (same four digests). Harmless to
   the gate and to cut-1's `tests/command_gate_census.rs:37`, which cites the
   OUTER path — but it is a second hand-copy of pinned bytes with no provenance
   row, which is the very class BRIEF-R6's one-truth fence exists to close.
   **Disposition:** reported; **not deleted** (staged bytes are staging's, and
   the estate's never-destroy law binds); every fixture this cut writes cites
   the OUTER path so no fixture can ever pin the accidental copy. A second
   cosmetic: `.fleet/gate-entry.sh:2` still reads "AWL-CUT1 gate" in its header
   comment. Recorded; no behaviour depends on it; not amended.

---

## §3 — DECISIONS TAKEN (each reportable, each the owner's to overturn)

### D1 — the recipe surface lands INSIDE `crates/aion-awl/src/command/`, as a sub-family

The brief ratifies "front in `crates/aion-awl`" (fold B) and leaves the module
shape to the flight, as cut-1's placement clause did. Taken as a **sub-family**
`crates/aion-awl/src/command/recipe/` rather than a sibling
`crates/aion-awl/src/recipe/`, on a mechanical ground measured in this tree:

* `CommandRefusal::refuse` — *the door every refusal inside this surface comes
  through*, which derives the layer FROM the cause so misfiling cannot be
  written — is `pub(super)` (`src/command/error.rs:253`). Rust visibility
  descends: a module under `command/` may call it; a sibling `src/recipe/`
  may not. A sibling would therefore force either (a) widening cut-1's landed
  refusal door to `pub(crate)`, or (b) a second construction path for the same
  record — which is the catch-all the carrier-contract law forbids.
* BRIEF's wall says new causes join **cut-1's** closed enum. The enum lives at
  `src/command/cause.rs:76`, and its inventory gate
  (`src/command/cause_tests.rs:12`) is `#[path]`-mounted inside that module.
  One vocabulary, one home.
* The 500-line law is per FILE and is satisfiable: the plan lands the recipe
  family as ~10 files, each budgeted well under the limit, with
  `src/command/mod.rs` (40 lines today) gaining only `mod recipe;` and
  re-exports.

The sibling shape stays live only if S3–S8 cannot hold a file under 500 lines.
Either outcome is reported as a finding with its reasoning, exactly as
cut-1's D2 was.

### D2 — every new word is POSITIONAL, and that is measured, not preferred

Cut-1's stated discipline (`src/command/words.rs:1–21`: `command` stands in 54
documents of this tree as a field name, so reserving it would break documents
that exist). Re-measured across the discovered `**/*.awl` corpus at this HEAD,
whole-word, for this cut's candidate vocabulary:

| candidate | documents already using it | candidate | documents |
|---|---|---|---|
| `receipt` | **30** | `needs` | **17** |
| `parameter` | **20** | `artifact` | **8** |
| `derives` | **6** | `fire` | **5** |
| `slot` | **5** | `depends` | **4** |
| `compare` | **4** | `census` / `digest` / `wall` | **3** each |
| `recipe` / `sweep` / `assemble` | **2** each | `derive` / `skeleton` / `fill` | **0** |

Reserving any of the non-zero rows would break documents that exist today.
**Every word this cut lands is recognised positionally, in the one grammar
position where nothing else may stand**, and is added to `POSITIONAL_WORDS`
(`lexer/tokens.rs:108`, `[&str; 19]` today) — which is what the glossary, the
grammar table and the language server read, so one edit reaches all three.
Note `after` is ALREADY a reserved `Keyword` meaning *an ordering dependency on
another step* (`guide/grammar.rs:46` `GrammarPosition::StepReference`); the
dependency edge between recipe steps SHOULD reuse it rather than mint a synonym
— one word, one meaning — and the leg confirms or reports that at S3.

### D3 — the compare surface types over a RECORD SHAPE, never over a cause enum

BRIEF-R5's affirmative duty plus grill fold A force a design answer, and it is
better taken now than discovered at S7. Cut-1's `CommandCause::from_code`
(`cause.rs:322`) returns `None` for any string outside its 35 — deliberately,
so an unknown code is never a nearest match. It follows mechanically that **if
the compare surface consumed `CommandCause`, no foreign record could flow
through it at all**, and the ratchet-shaped fixture (a `conclusion_missing` /
`conclusion_malformed` pair, a named-hole verdict) would be inexpressible —
falsifying the ruling at the first fixture.

Taken: the compare surface consumes a **record shape** — a non-empty stable
code string, its layer, and its observed bytes — with `CommandCause` converting
INTO it and never out of it. The builder's own refusals keep their
unrepresentable-forbidden-cause discipline (`error.rs:36 CauseMisfiled`,
`:253 refuse`); foreign verdict records flow as data. Fold A's property test
then enumerates `CommandCause::ALL` (`cause.rs:186`) **programmatically from the
enum** — so a cause added by a later design change joins the test without an
amendment — and asserts uniform consumption while the compare surface's own
code contains no match on cause identity. If that cannot hold, it is the
reportable finding to Waffles and Vesper the ruling itself invites.

### D4 — new LAYERS are proposed, not assumed, and are a recorded design change

`CommandLayer::ALL` is `[Self; 4]` (`cause.rs:49`): Parse, Check, Emit,
Specimen. This cut acts at two moments cut-1 had no name for — **compose /
project** (a fill or a projection is refused before anything runs) and
**fire** (a census or precondition is refused at the moment of firing). The
carrier-contract law says different layers get different causes and never one
catch-all, so the leg extends `CommandLayer` rather than filing fire-time
refusals under `Emit`. Both the layer additions and every new cause are
recorded in the report as design changes with their reasoning, per BRIEF's
refusal-causes wall. Provisional set, red-first, each with its specimen —
`dependency_cycle`, `shell_string_body`, `derivation_missing`, `double_carry`,
`slot_unfilled`, `excluded_term` (C-1), `artifact_digest_mismatch`,
`restore_inexact`, `census_mismatch`, `stage_compare_mismatch`,
`object_moved_before_claim`, `venue_value_restated` — final list and final
layer assignment settled at S2 and reported as taken.

---

## §4 — ORDERED WORK (foundation-first, red-first; S-numbers are the leg's order)

### S1 — baselines taken BEFORE any grammar change (BRIEF-R8, first half)

Ordering is load-bearing: a "before and after" claim needs the *before*
recorded while the grammar is still the base grammar.

* Re-run `crates/aion-awl/tests/corpus_roundtrip.rs` at base grammar and record
  its printed census. Measured now: **443** `.awl` documents discovered
  repo-wide against a **431**-record baseline manifest
  (`tests/fixtures/corpus/manifest.txt`), the 12 additions being cut-1's own
  fixtures — which the instrument reports rather than fails on
  (`corpus_roundtrip.rs:229 report_additions`), while every baseline document
  must still read identically (`:216–240`) and `DISCOVERY_FLOOR = 400`
  (`:65`) guards a walk that stopped looking.
* ⛔ **`AWL_CORPUS_BLESS` is NOT set by this leg, in any step.** The manifest
  was generated at the pre-command-declaration grammar
  (`corpus_roundtrip.rs:41–46`), which is the only state in which the pin means
  anything. Re-blessing it would erase cut-1's byte-identity proof as well as
  this cut's.
* Re-affirm §0's conformance read and §1's census against the leg's own HEAD
  and report any drift. Confirm, do not re-litigate.

### S2 — the cause and layer vocabulary (D4), red-first, before anything refuses through it

* Extend `CommandCause` (`src/command/cause.rs:76`) and `CommandLayer`
  (`:36`) per D4. `ALL` arrays (`:49`, `:186`) grow with them —
  `cause_tests.rs:12 the_inventory_carries_every_variant_exactly_once` and
  `:61 every_layer_files_at_least_one_cause_and_every_cause_has_a_layer` are
  the mechanical pins that make a forgotten row a red test.
* `layer()` (`:234`) is an exhaustive match with no wildcard — a new cause does
  not compile until someone decides which layer it refuses at. Keep it that way.
* `cause_tests.rs:90 the_vocabulary_carries_no_catch_all_code` must go on
  passing over the widened set.
* **RED FIRST, mechanically enforced:** `tests/command_refusals.rs:285
  every_cause_has_a_refusing_specimen` iterates the enum; a new code with no
  refusing document fails that test rather than a review. Every S3–S8 refusal
  below lands its red specimen before its passing path.
* ⛔ `excluded_term` is minted HERE as C-1's recorded design change, with its
  red specimen, and BRIEF-R4's "consumed, not re-minted" wording is reported as
  divergent rather than quietly satisfied.

### S3 — recipe declarations: grammar, AST, parser, printer (BRIEF-R1)

* New positional words per D2, added to `lexer/tokens.rs:108` and grouped for
  the parser as cut-1 grouped its own (`src/command/words.rs:30 CLAUSE_WORDS`).
* Top-level dispatch: a `recipe` arm beside the `command` arm at
  `parser/document.rs:492–495`, with the refusal strings at `:500` and `:510`
  extended in BOTH places — `tests/grammar_parser_gate.rs:78 DECLARATION_REFUSAL`
  pins them and moves together or reds.
* AST: `document.recipes` beside `document.commands`
  (`src/ast/document.rs:88`); canonical printing beside `print_command`
  (`src/printer/document.rs:144,181,197`) so recipes round-trip through
  `aion awl fmt` — the round-trip is asserted by
  `tests/printer_roundtrip.rs` and by S1's manifest.
* **A recipe body is a declared command or a declared typed capture — NEVER a
  shell string.** The shell-string form is refused at the PARSE layer
  (unparseable, not linted) with its own cause and a test per named form,
  extending `just_subset.rs:106 REFUSED_FORMS` so the refusal list cannot
  quietly shorten (`command_just_subset.rs:255`).
* **Dependencies form a checked DAG**; a cycle is a check error WITH A SPAN
  (`dependency_cycle`). Prior art for the graph pass and its span discipline:
  `src/checker/graph.rs` (wired at `src/checker/entry.rs:11`).
* ⛔ **Amendment 1, structural, and it must be measured both ways.** A recipe
  form stays unrepresentable INSIDE a gate-leg command declaration: `words.rs:46
  ASSEMBLY_WORDS` and `parse.rs:229 assembly_refusal` are untouched, and
  `tests/fixtures/command/just-subset/refusal-recipe-body.awl` goes on refusing
  with `recipe_form_refused` byte-identically (C-3). The new red is the
  converse: a gate-leg command declaration nested in, or reachable from, a
  recipe context does not make the recipe reachable from a gate leg — the
  fence is proven by test in both directions, and the diagnostic carries the
  reason (a reader sees the exact command that ran).
* Every absorbed recipe-side production gets its row in
  `just_subset.rs:59 ABSORBED_PRODUCTIONS` and therefore its conformance
  fixture (`command_just_subset.rs:123`), Just-suite-derived where the CC0
  suite (`SUITE_REPO/SUITE_COMMIT`, `just_subset.rs:39–41`) carries a
  specimen and authored-with-absence-named where it does not.

### S4 — parameter kinds with declared derivations (BRIEF-R2 / DESIGN-R4b)

* The four kinds — skeleton / per-item / both / RE-DERIVE — typed, parsed,
  checked, printed. They are read off the corpus, not invented:
  `.fleet/fixtures/corpus/issue-investigations.README.md:24–30` is the
  field-kind table verbatim (`node_label`/`repo_url`/`workspace_root`/
  `control_prompt` = skeleton; `control_expect` = ⚠️ neither, **re-derive**;
  `issue`/`threshold` = per-issue; `investigate_prompt` = **both**).
* A **re-derive parameter carries a declared derivation — source address +
  extraction — never a copied literal**, and the corpus supplies the exact
  shape: README `:40–46` declares source (`origin/main`, path
  `workflows/investigate/investigate_worker.awl`) and extraction (line 22),
  with the law that it is read **at the dispatch minute, not the compose
  minute — they drift**. A re-derive parameter without a derivation is a check
  error (`derivation_missing`).
* **Derived values resolve at FIRE time against the named source** — proven by
  a test that mutates the source between compose and fire and asserts the
  fired value follows the source. Compose/fire drift becomes impossible rather
  than checked-for.
* **One source, one declared projection.** The double-carry disease is
  measured in this cut's own exhibits and both are fixtures: haem-wall carries
  `leg_ids_csv = "build"` beside `legs[0].id = "build"`
  (`.fleet/fixtures/corpus/haem-wall.INPUT-FINAL.json`), and the speech-1 KIT
  carries the same pair at `speech1.KIT-input-template.json:8` vs `:18`, plus
  `repo` (`:7`) restating `workspace_root` (`:2`) + `run_name` (`:3`). A
  declaration carrying the same truth twice is a check error (`double_carry`).
* **`literal_where_derived` is CONSUMED, per C-2**: the declaration-side
  trigger is added, the cause code and its wire string are unchanged
  (`cause.rs:128`, `:294`), `derived.rs`'s three closed shapes are NOT widened,
  and the raise site's message discipline (`check_fills.rs:242–253`, observed
  bytes always) is reused. Red specimen: a re-derive slot fed the literal
  `wall-ctl-b508cd84-waffles-20260823` — which the haem-wall exhibit carries
  TWICE by hand (`control_expect` and `counsel_control_expect`) and which
  cut-1's `DerivedShape::ControlToken` (`derived.rs:33`) already knows by shape.

### S5 — slot refusal and sweep-by-claim (BRIEF-R3 / DESIGN-R4a, L2)

* **Refuse-on-unfilled-slot**, typed, carrying the slot name and observed
  bytes. `build-input.py`'s exit-2 behaviour, absorbed and typed — at the
  bytes: `.fleet/fixtures/corpus/sleepreap.build-input.py:28–33` (refuses while
  `OPERATING-GROUND.md` still carries `RECORD-RUN-PENDING`) and
  `liminal22.build-input.py:20–25` (the same shape over `TBD-LINUX-TESTS`).
  The unfilled-slot specimens are the corpus's own:
  `skeleton.input.json:6,7,9,14` (`<<ISSUE_NUMBER>>`, `<<ISSUE_TITLE>>`,
  `<<TRUTHFUL_TRIGGER…>>`, `<<RE-DERIVE AT THE DISPATCH MINUTE…>>`) and the
  KIT's sed slots (`__CTL_TOKEN__` ×2, `__RIG_COMMIT__` ×2, `__CUT_COMMIT__`
  ×1, `speech1.KIT-input-template.json:11,13,14,23`).
* **SWEEP-BY-CLAIM as a builder function**: a corrected source declaration
  re-projects and re-checks every input assembled from it, mechanically. The
  sweep's OUTPUT IS ITSELF A RECORD — which inputs, from which declaration,
  re-checked against what — and it carries a completeness claim naming its own
  derivation (assembly is FROM declarations, so the edge set is closed).
* Fixture: three consumers, one source, one correction, one sweep record
  listing all three.

### S6 — wall projection, one wall one home (BRIEF-R4 / DESIGN-R6, assigned by ruling)

* One wall DECLARATION projects into (a) a prompt rendering and (b) a
  mechanical gate-enforcement rendering, from the same source — the L5 family
  closes because both renderings share one declaration. Fixture-compared, both
  directions.
* ⛔ **The builder owns projection, never content.** Per-lane wall SETS,
  briefs, judge duties and FINDINGS stay authored (design §8, RESULT §A4).
* **Negative vocabulary (L1)**: excluded terms are declared beside the walls; a
  projection that would emit one refuses with `excluded_term` and the term in
  observed bytes (minted at S2 per C-1).
* **Structured diff of two declaration states** produced by test, naming every
  changed projection — the hand-built refire artifact, mechanized.
* **The L6 rider as a projection kind**: a hazard-adjacent step is a declared,
  digest-pinned, in-tree artifact the leg EXECUTES and reports; the check
  refuses a digest mismatch (`artifact_digest_mismatch`); restore steps carry
  byte-compare restore verification and refuse if inexact (`restore_inexact`).
  L6's honest bound is carried **verbatim** — the measured claim is *"phrasing
  was the wrong axis here"*, never *"mutation vocabulary tripped it."*
* **The haem-wall exhibit as ONE declaration + N projections — and the count is
  MEASURED, not remembered.** At the pinned bytes of
  `.fleet/fixtures/corpus/haem-wall.INPUT-FINAL.json`, the allowlist
  `gates.json, known-carries*, scripts/release/, docs/evidence/, gate-logs/*.md,
  CONTRIBUTING.md, README.md, docs/DOCUMENTATION.md, .fleet/` appears **verbatim
  twice** — in `fix_prompt` and in `legs[0].prompt` — and is referred to without
  restatement **twice more** (`fix_prompt` again: *"nothing committed outside the
  allowlist"*; `judge_prompt`: *"every changed path inside the allowlist"*),
  with the gate script enforcing it mechanically as the fourth rendering. Two
  further one-declaration-many-projections pairs sit in the same file and are
  fixture material: `control_prompt` is byte-identical to
  `counsel_control_prompt`, and `control_expect` to `counsel_control_expect`.
  BRIEF-R4 names this the "×4-copies exhibit"; the fixture states what it
  measured, and if the measured shape differs from the brief's phrase that
  divergence is reported (candidate **F-R7D-1**), never rounded to fit.

### S7 — mechanical specimen firing and generated receipts (BRIEF-R5 / DESIGN-R5 + R7)

* **A recipe step KIND that fires a declared demonstration artifact** (the
  known-answer red), compares TYPED, refuses the stage on mismatch, and writes
  the receipt. The corpus supplies the canonical exhibit at the bytes:
  `.fleet/fixtures/corpus/speech1.STAGING.md:17–23` — *"TEST THE GATE'S OWN
  CONTROLS … it must go RED with `deliverable sdks/liminal-py/pyproject.toml
  absent from the diff` … Record the output and compare against THAT string."*
  Cut-1's `fire_specimen` (`src/command/specimen.rs:31`) is the prior art for
  the LAYER discipline — a claim the document made about itself that turned
  out false gets its own causes — and is consumed, not re-minted.
* **The compare surface is generic per D3**, and its genericity is proven two
  ways, both required (grill fold A): the named ratchet-SHAPED exemplar (a
  named-hole verdict; an absent-vs-malformed pair as two distinct causes,
  modelled on `crates/aion-fleet-plan/src/error.rs:60–70`, read as data, never
  absorbed) **and** an enum-derived property test that enumerates
  `CommandCause::ALL` (`cause.rs:186`) programmatically, asserts uniform
  consumption of every member, and asserts the compare's own source matches on
  no cause identity. The exemplar alone cannot discriminate a generic compare
  from an enumeration that special-cases the exemplar's members — which is
  exactly why fold A exists.
* **Receipts are GENERATED records, never typed prose.** A receipt asserts what
  IS and names what it measured — the tree, the head, clean/dirty — and those
  fields are **mandatory at construction**: a receipt that cannot say what it
  measured is unrepresentable (the constructor returns a typed error; no
  `unwrap`, no `expect`, no panic). The in-tree model for "name the tree you
  measured" is the gate itself (`.fleet/gate-entry.sh:65–75`, `:125`).
* **Re-read before claiming**: every builder commit re-reads its object from
  the committed state before claiming it (`fleet-fanin.sh:42–48` generalized).
  Red test: the object is mutated between write and claim ⇒ refusal
  (`object_moved_before_claim`).
* **Receipt-FIRST stream ordering is preserved as law** — the corpus states it
  as step 1 of staging (`speech1.STAGING.md:6–8`: *"RECEIPT FIRST … No receipt
  → no staging; ask, don't infer"*). Commit trailers name the venue's actual
  generator or are omitted.

### S8 — declared serviceability: the reference census (BRIEF-R6 / DESIGN-R8, builder's half)

* Every artifact a recipe references outside itself — deployed documents,
  scripts while they exist, kits, declared demonstration artifacts — is
  declared **with its content hash**; the FIRE step measures the deployed bytes
  against the census and refuses on mismatch (`census_mismatch`), with declared
  and observed hashes both in the diagnostic.
* At-fire preconditions the checklists carry by hand become declared
  preconditions of the fire step — the exhibit is
  `speech1.STAGING.md:37–39`'s three-arm re-check (idle-enough, workers serving
  the queue, disk ≥ 40G), plus `:6–8`'s receipt precondition.
* ⛔ **The one-truth fence, mechanical**: a recipe declaring a queue or venue
  value the dispatch already declares is a check error
  (`venue_value_restated`).
* ⛔ **No new venue-binding holes.** `{workspace_root}` remains the ONE
  admitted placeholder, consumed from `src/workspace_root.rs:47` and its
  accept-set (`:110 harness_path_form`) — **not extended**. Cut-1's
  brace-placeholder fence is CONFORMANCE-READ and consumed, never
  re-implemented: `CommandCause::CwdPlaceholderUnknown` (`cause.rs:135`) raised
  at `src/command/check.rs:403,417`. Venue binding stays the deploy surface's
  (Q5 §2a) and lands LAST, with the ops-script retirement.
* Hashing reuses the workspace's existing algorithm home rather than minting a
  second one — `crates/aion-package/src/hash.rs` is already a dependency of the
  front (`crates/aion-awl/Cargo.toml:19`); if the front needs `sha2` directly,
  that manifest delta is measured and reported under S9's fold-B duty and
  argued on its merits (a digest function is not execution machinery), never
  slipped in.

### S9 — the executing verb in `aion-cli`, and the manifest delta MEASURED (fold B)

* Declaration, check and projection stay front (`crates/aion-awl`); the
  executing verb rides `crates/aion-cli` per the ratified placement, as a new
  module family beside the existing AWL verbs (`src/awl.rs:33 AwlCommand`
  carries `Check`/`Fmt`/`Emit`/… today) — **not** by growing `awl.rs`, which is
  already 587 lines.
* ⛔ **The dependency-clean claim is MEASURED, not trusted.** The report carries
  `crates/aion-awl/Cargo.toml`'s dependency DELTA for this cut against its state
  at BASE (today: `aion-package`, `beamr`, `gleam-types`, `serde_json`,
  `thiserror`; dev-only `aion-worker`, `flate2`, `sha2`). **Required: zero
  execution-side additions** — no process, runtime, or transport machinery
  entering the front. A non-zero delta on that axis is a reportable finding
  against the ratification, not a thing to absorb.
* `anyhow` stays confined to the binary; the front keeps `thiserror` only.

### S10 — the composer corpus proven EXPRESSIBLE (BRIEF-R7)

Five exhibit groups, each a fixture asserting the expressed declaration
reproduces the original's behaviour AT THE BYTES (refusal codes, projected
content, fill results), with every normalization named in the fixture header —
cut-1's provenance discipline extended: source **REPO**, **PATH**, and commit
**DIGEST** (`command_gate_census.rs` and the `.params` headers are the model,
and each header's digest is re-hashed on the run so a specimen edited after the
fixture was written fails rather than passing against a stale copy).

| exhibit | BRIEF clause | pinned bytes (this tree) | expressed as |
|---|---|---|---|
| (a) `build-input.py` ×2 | R7(a) | `.fleet/fixtures/corpus/sleepreap.build-input.py`, `liminal22.build-input.py` | BRIEF-R3 refusals + BRIEF-R2 kinds |
| (b) field-kind rows | R7(b) | `skeleton.input.json`, `issue-investigations.README.md:24–30` | BRIEF-R2 parameter-kind declarations |
| (c) STAGING steps | R7(c) | `speech1.STAGING.md` | BRIEF-R5 step kinds |
| (d) haem-wall allowlist | R7(d) | `haem-wall.INPUT-FINAL.json` | BRIEF-R4 one-declaration exhibit |
| (e) speech-1 kit slots | R7(e) | `speech1.KIT-input-template.json` | BRIEF-R2/R3 declarations |

* **The two `build-input.py` copies DIFFER, and the completeness claim names
  both** (RIG.md records the measurement; re-measured here): `sleepreap`
  guards its argument count (`:19–21`, `exit 2`) and composes across TWO repos
  (`:23–24,37–38`), while `liminal22` has NO argv guard (`:15`, an IndexError
  where the other refuses) and carries a `--round2` variant selection
  (`:17,29–40`). Two behaviours, not one; both fixtures, or the claim is false.
* **(c) names what was deliberately NOT absorbed and why**, in the fixture
  header — the operator-law steps stay written words per design §8: author ≠
  reviewer handoff (`speech1.STAGING.md:28–29`), fire/verdict/landing authority
  (`:41–43`), the sequencing wall, and the security-adjacent mint step (`:24–25`)
  whose requirements come FROM Cally Ray's lane and are never proposed here.
* ⛔ **Expressibility, NOT deployment.** No composer script retires, no rig
  converts, no checklist is deleted, no deployed document moves.
  `workflows/fleet-dev/fleet_dev.awl` byte-identity stands and is asserted by
  S1's manifest. The strike list is the CONTRACT OF THE CUTOVER, a staged act
  after landing, owned by staging.
* **Any behaviour the surface CANNOT express is a reportable finding**, never
  papered over — cut-1's F-R6-1 (hardening expressible as a value but not as a
  presence, `.fleet/reports/build.md:402`) is the shape of an honest one,
  and remains OPEN as a design question this cut inherits rather than closes.
* The fixture set carries its own completeness claim: which exhibits, from
  which repos, at which digests, and which known corpus members were excluded
  with the reason.

### S11 — structural teaching rows for every new word (BRIEF-R8, first half)

A missing row is a **BUILD FAILURE**, and in this tree that is already
mechanical. For each new word:

* a guide sentence + complete checked example (`src/guide/declarations.rs`,
  entry shape `src/guide/entry.rs:145`), run through the real parse+check
  pipeline by the guide's own example gate;
* an arm in the exhaustive match `src/guide/lookup.rs:28 for_keyword` (keywords)
  or the positional arm pinned against `POSITIONAL_WORDS` (`lookup.rs:9–11`);
* the inventory equality `guide::all().len() == Keyword::ALL.len() +
  POSITIONAL_WORDS.len()` (`tests/guide_reference_gate.rs:204–210`) holds by
  construction, not by counting;
* a grammar-table position (`src/guide/grammar.rs:33 GrammarPosition`, today
  `ALL: [Self; 16]` at `:71`) — a `RecipeBody` position joins `CommandBody`
  (`:65`), and `words_offered_at` (`:316`) then feeds completion for free;
* an **AWL-REFERENCE section** — §17 is cut-1's "Command declarations"
  (`examples/assistant/resources/AWL-REFERENCE.md:955`, `src/guide/entry.rs:130`);
  this cut writes **§18** and a `Section::new(18, …)` const beside it. Each new
  word's spelling MUST occur in its cited section's body or
  `guide_reference_gate.rs:126` reds (whole-word matched, `:151`). Written once
  into `examples/assistant/resources/AWL-REFERENCE.md` and copied to
  `crates/aion-awl/guide-embed/AWL-REFERENCE.md`, which `:221` pins
  byte-identical (both files hash `336dcb4f…` today; the wall admits both);
* **LSP surfacing**: hover and completion already DERIVE from the one table
  (`crates/aion-awl-lsp/src/glossary.rs:55,86,112`,
  `completion.rs:274`), so the rows appear by construction — but
  `glossary.rs:101` hard-codes `const COMMAND_SECTION: u8 = 17` to scope
  command-cluster hover, and a §18 cluster needs that machinery widened
  deliberately, with a test asserting hover returns the guide sentence for each
  new word **against the table, not a copy**;
* the ops-console asset regenerated via `aion awl guide --json >
  apps/aion-ops-console/src/features/authoring/assets/awl-guide.json`, which
  `crates/aion-cli/tests/console_guide_asset.rs` compares — that test is READ,
  not edited.

⛔ Tree-sitter is **not** in this cut's acceptance (it rides Tom's pending
word). The constraint that follows is cut-1's and is carried: do not change
`builtins::BUILTIN_TYPES`, or `tests/editor_assets.rs` reds and drags
`tools/tree-sitter-awl/**` — outside the wall — in with it. New words are
positional words, never builtin types (D2).

### S12 — corpus close-out and estate discipline (BRIEF-R8, second half)

* Re-run S1's corpus instrument against the NEW grammar: all 431 baseline
  documents must read identically; the discovered list is the completeness
  claim and is printed, not assumed. One divergence is a grammar regression and
  stops the leg.
* `cargo clippy --workspace --all-targets -- -D warnings` clean. If clippy
  fires, **fix the code** — `#[allow]`, `#[expect]`, `#[deny]`, `#[ignore]`,
  `_var` renames and `#[cfg(any())]` are bypasses, not fixes.
* `cargo fmt --all` **APPLIED** before commit. ⛔ **NO `fmt --check` gate**
  (Tom's ruling, docs `2ab9689`; the gate carries the same ⛔ at
  `.fleet/gate-entry.sh:10–11`). The `seqlegs` specimen still runs
  `fmt --all --check` as a leg; it is carried as a SPECIMEN of the pre-ruling
  shape, never as a model.
* No file over 500 lines; `mod.rs` declarations-only (`src/command/mod.rs` and
  any new `recipe/mod.rs` carry `pub mod` and re-exports and nothing else);
  `thiserror` for new error types; no `unwrap`/`expect`/`panic` in library code.

### S13 — report, clean tree, gate

* `.fleet/reports/build.md` is **a COMMITTED FILE** — the payload copy is a
  courtesy. ⚠️ It currently holds **cut-1's** report (carried into main by the
  landing); this leg REPLACES it wholly, and until it does, no reader should
  mistake those bytes for this cut's.
  Contents, in this order: **C-1 first** (the `excluded_term` divergence, since
  it changes what the cut is allowed to mint), then C-2 and the rest of the §0
  conformance read with movement named; the §1 census verbatim (pathspec +
  five zeros + every triage); §2's staging conformance including **F-STAGE-1**;
  the executing-verb placement decision as taken with reasoning (D1) and the
  front crate's **manifest dependency delta** (fold B, zero execution-side
  additions or the finding); the D3/D4 design changes (new causes, new layers)
  each with its reasoning; the BRIEF-R7 fixture set's completeness claim; the
  ratchet-ruling fixture's outcome **plus** the enum-derived genericity property
  test's result; every inexpressibility finding; and — for any requirement this
  leg cannot complete — **the requirement named and the stop reported**, never
  a silent thinning. Every citation disambiguates BRIEF-Rn from DESIGN-Rn.
* `cargo fmt --all`, commit everything, verify `git status --porcelain` is
  EMPTY, then run `.fleet/gate-entry.sh` on the committed tree. The gate reds
  on a dirty tree by design (`:69–74`): a gate that runs on the working tree
  attests the working tree.

---

## §5 — WALLS CARRIED INTO EVERY STEP

* ⛔ **Amendment 1, structural** (BRIEF-R1): the recipe layer is unrepresentable
  in gate-leg context — grammar-refused, test-proven, in both directions (C-3).
* ⛔ **Ratchet family untouched** (`liminal22`, `sleepreap`): no script
  converted or retired, no check absorbed. The cut's ONLY contact is the
  affirmative duty — a vocabulary-generic compare (D3) and the ratchet-shaped
  fixture. If that fixture reveals the compare CANNOT stay generic without
  absorbing a check, it is a reportable finding to Waffles and Vesper against
  the ruling, which is falsifiable at the bytes by design.
* ⛔ **Expressibility, not deployment**: no composer retires, no rig converts,
  no deployed artifact moves; `fleet_dev.awl` byte-identity stands; the sealed
  fleet-ops scripts remain line 4's.
* ⛔ **No shims, no compat**: the surface lands whole or not at all.
* ⛔ **The builder owns projection, never content**: per-lane wall sets, briefs,
  judge duties and FINDINGS stay authored. Operator law stays law — author ≠
  reviewer handoff, fire authority, venue choice, receipt-first ordering,
  red-first evidence ordering.
* ⛔ **Teardown deletion authority is not widened** by any declarative
  replacement (design §8, verbatim — root pinned, shape-checked, check gating
  the rm in the same unit, leftover trees remain refusals).
* **Q5 consumed, not implemented**: venue binding stays the deploy surface's;
  harness-profile blocks are the deploy lane's later work; no venue-binding
  hole beyond `{workspace_root}` (`src/workspace_root.rs:47`).
* **Security-adjacent surfaces** (token minting inside recipes; receipts must
  not widen token exposure; allowlist containment): pointers to Cally Ray stand
  exactly as the design brief carries them. Nothing here proposes minting or
  rotation requirements.
* **Refusal causes**: every new cause joins cut-1's closed enum BY DESIGN
  CHANGE recorded in the report; layer-named; `observed_bytes` always;
  forbidden causes unrepresentable at construction.
* **Path wall, exactly as staged** (`.fleet/gate-entry.sh:33`) — no amendment
  is needed by this cut and none is made:
  `^(crates/aion-awl|crates/aion-cli|apps/aion-ops-console/src/features/authoring/assets/awl-guide\.json|examples/assistant/resources/AWL-REFERENCE\.md|Cargo\.toml|Cargo\.lock|\.fleet/)`.
* ⛔ **Never one blocking sleep sized to an expected duration**; any spawned
  process is killed only by its own recorded pid.

## §6 — EXIT CONDITION

`.fleet/gate-entry.sh` GREEN on a CLEAN committed tree: toolchain 1.97.1
pin-honored from `rust-toolchain.toml` (source-asserted, not version-matched);
tree CLEAN; census within the wall with the deliverable present under
`crates/aion-awl/`; clippy `-D warnings` clean; a nonzero count of new test
annotations under `crates/aion-awl*`; the workspace suite listed and green.
Plus `.fleet/reports/build.md` committed, carrying every duty §4/S13 names.
