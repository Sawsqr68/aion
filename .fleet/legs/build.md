# LEG — `build` (the only leg of this flight)

One leg. RIG.md: *"Single leg `build` — the surface lands whole (no-shims wall),
same reasoning as cut-1."* No other leg exists, and none is to be invented. The
ordered work is `.fleet/plan.md` §4 / S1–S13; this document states the leg's
duties, surface, walls, evidence duties and deliverables.

⛔ **Citation discipline (BRIEF fold C), binding on every line of this leg's
output.** `BRIEF-Rn` = `.fleet/BRIEF.md` R1–R8. `DESIGN-Rn` = the design
brief's own R1–R8. `CUT1-Rn` = cut-1's brief, retrievable in this tree at
`git show 1bab9547:.fleet/BRIEF.md`. A bare `Rn` is two different requirements
and is never written.

---

## 1. FIRST DUTIES, BEFORE ANY WORK — in this order

### 1a. The CONFORMANCE READ of the landed cut-1 surface (BRIEF's first act)

*"The flight's first act is a CONFORMANCE READ of the landed cut-1 surface
against `9ec0a8f`."* Every shape this brief consumes from cut 1 — the
command-declaration types, the admits-`--` property, the refusal cause enum,
`literal_where_derived`, the fixture harness — is re-verified **at the landed
bytes**, not at the brief's description of them. Divergence is a reportable
finding, and the brief AMENDS before the build proceeds on the divergent
point: never silently absorbed, never silently fought.

Executed and recorded at `.fleet/plan.md` §0. Five shapes read; **three
conform outright; two diverge and both are carried to the report**:

* **C-1 — `excluded_term` is NOT in cut-1's enum.** BRIEF-R4 instructs
  "consumed, not re-minted"; a whole-tree search returns exactly one hit — the
  brief's own line describing it. `CommandCause::ALL`
  (`crates/aion-awl/src/command/cause.rs:186`) carries 35 members and none is
  it. The cause is MINTED here as a recorded design change under BRIEF's own
  refusal-causes wall, and the divergence is the **first item of the report**,
  because it changes what this cut is permitted to mint.
* **C-2 — `literal_where_derived` diverges as a MECHANISM, not as a cause.**
  Cut-1's trigger is the literal's SHAPE (`src/command/derived.rs:65`, three
  closed shapes). BRIEF-R2's acceptance needs a DECLARATION-driven trigger (a
  re-derive slot fed any hand literal, `"main"` included). The cut adds the
  declaration side and consumes the cause code unchanged.
* **C-3 — the amendment-1 fence is load-bearing** and is preserved
  byte-identically (`src/command/words.rs:46`, `src/command/parse.rs:229`,
  fixture `refusal-recipe-body.awl`).

The leg **re-affirms** this read against its own HEAD before S1 and reports
drift. It does not re-litigate it.

### 1b. The has-this-already-happened CENSUS, fresh, over THIS cut's constructs

Cut-1's census answered a different question on a tree that has since moved,
so this one is run fresh. Three parts, all mandatory:

1. **State the pathspec.** An undeclared corpus is not a census. Stated at
   `.fleet/plan.md` §1: the whole worktree at HEAD, excluding exactly `.git/`,
   `target/`, `node_modules/` and nothing else; text files only; narrower
   pathspecs named per row where the question is a language-surface one.
2. **State the zero, or triage every hit.** A hit is not a failure; an
   untriaged hit is.
3. **Carry it into `.fleet/reports/build.md` verbatim.** The census is a report
   section, not a private step.

Result: **five constructs sought, five zeros, every hit triaged** — a
recipe/dependency construct in the AWL word tables (zero; every `recipe` hit is
cut-1's own refusal machinery or MIR codegen prose); wall-projection or
prompt-assembly machinery (zero; every `wall` hit is wall-*clock* prose or the
`haem-wall` fixture NAME); a sweep-by-claim mechanism (zero; the hits are an
author-domain action name and the server's queue `plan_sweep`, out of wall); a
reference-census / content-hash fire-precondition surface (zero in the front;
`aion-package`'s `content_hash` answers package identity, not artifact
serviceability); a staging-receipt generator (zero; every `receipt` hit is a
user's `type Receipt` in an example). One further adjacent surface —
`crates/aion-fleet-plan`'s sealed conclusion verifier — is triaged as prior
art, **outside this cut's wall**, read and never modified.

Adjacent and equally mandatory: **staging conformance**, `.fleet/plan.md` §2 —
the rig's own claims measured rather than trusted. The gate's "changed in
exactly three places" claim HOLDS at the bytes (BASE, WALL, comment block; two
executable lines). Seam movement between BASE and HEAD: **NONE** — the 15
changed files are all under `.fleet/`. One staging observation is reported and
**not repaired**: `F-STAGE-1`, a byte-identical nested duplicate of the four
gate specimens at `.fleet/fixtures/gate-scripts/gate-scripts/`.

---

## 2. SURFACE — what this leg builds

AWL gains a **declared recipe surface** — assembly acts with typed
dependencies, parameter kinds with declared derivations, mechanical specimen
firing, walls projected from one declaration, receipts generated rather than
typed, and referenced artifacts census-checked at fire time — proven
expressible over the live composer corpus as fixtures, structurally unreachable
from any gate-leg declaration, with every pre-existing document byte-identical
and every new word carrying its structural teaching row.

Requirements, in the leg's execution order (`.fleet/plan.md` §4):

| step | requirement | one-line surface |
|---|---|---|
| S1 | BRIEF-R8a | corpus + exhibit baselines taken **before** any grammar change; ⛔ no `AWL_CORPUS_BLESS` |
| S2 | BRIEF-R4/R5/R6 spine | the cause and layer vocabulary extended, red-first; `excluded_term` minted (C-1) |
| S3 | BRIEF-R1 | recipe declarations: grammar, AST, parser, printer, checked DAG, amendment-1 fence both ways |
| S4 | BRIEF-R2 (DESIGN-R4b) | parameter kinds + declared derivations; fire-time resolution; double-carry refusal; `literal_where_derived` consumed |
| S5 | BRIEF-R3 (DESIGN-R4a, L2) | slot refusal typed; sweep-by-claim as a builder function with its own record |
| S6 | BRIEF-R4 (DESIGN-R6) | one wall declaration, two renderings; excluded terms; structured declaration diff; the L6 declared-artifact kind |
| S7 | BRIEF-R5 (DESIGN-R5+R7) | specimen firing, generic compare, generated receipts, re-read discipline |
| S8 | BRIEF-R6 (DESIGN-R8) | the reference census; at-fire preconditions; the one-truth fence |
| S9 | placement, fold B | the executing verb in `crates/aion-cli`; the front's manifest delta MEASURED |
| S10 | BRIEF-R7 | the composer corpus expressed as fixtures (a)–(e), with a completeness claim |
| S11 | BRIEF-R8a | teaching rows: guide sentence, checked example, AWL-REFERENCE §18, LSP, console asset |
| S12 | BRIEF-R8b | corpus close-out, clippy `-D warnings`, `fmt --all` applied, 500-line law |
| S13 | — | committed report, clean tree, gate |

**Placement, as taken** (`.fleet/plan.md` §3, D1): declaration, check and
projection stay front in `crates/aion-awl`, per the item-5 seam and the
ratified fold B; the recipe family lands as a **sub-family**
`crates/aion-awl/src/command/recipe/` rather than a sibling module, on a
mechanical ground measured in this tree — `CommandRefusal::refuse`, the single
door that derives a refusal's layer FROM its cause, is `pub(super)`
(`src/command/error.rs:253`), so a sibling would force either widening cut-1's
landed door or building a second construction path for the same record. The
**executing verb rides `crates/aion-cli`** as a new module family beside the
existing AWL verbs (`src/awl.rs:33`), never by growing `awl.rs` (already 587
lines). Both decisions are reported with their reasoning, and the item-5
pricing decision remains the owner's at landing.

---

## 3. WALLS — binding on every step

**Path wall, exactly as staged** (`.fleet/gate-entry.sh:33`). No amendment is
needed by this cut and none is made — cut-1's F-RIG-1 fix (the authored
reference twin) is already carried, and `crates/aion-cli` was admitted at
staging rather than discovered mid-flight:

```
^(crates/aion-awl|crates/aion-cli|apps/aion-ops-console/src/features/authoring/assets/awl-guide\.json|examples/assistant/resources/AWL-REFERENCE\.md|Cargo\.toml|Cargo\.lock|\.fleet/)
```

**Structural walls, from the brief:**

* ⛔ **Amendment 1, structural** (BRIEF-R1): no recipe, dependency or assembly
  form is *reachable* from a gate-leg declaration — the GRAMMAR refuses it
  (unrepresentable, not discouraged), a test proves the refusal in **both**
  directions, and the diagnostic carries the reason (a gate leg's review
  property: a reader sees the exact command that ran).
* ⛔ **The ratchet family is UNTOUCHED** — per this brief's own fresh ruling,
  on its own four grounds. `liminal22` and `sleepreap`: no script converted or
  retired, none of the five checks absorbed. The cut's ONLY contact is the
  affirmative duty: a compare surface typed over the ONE vocabulary's record
  shape, proven by a ratchet-SHAPED fixture flowing through as DATA. If the
  fixture reveals the compare cannot stay generic without absorbing a check,
  **that is a reportable finding to Waffles and Vesper against the ruling** —
  the ruling is falsifiable at the bytes, as a ruling should be.
* ⛔ **Expressibility, NOT deployment** (BRIEF-R7): no composer script retires,
  no rig converts, no checklist is deleted, no deployed document moves;
  `workflows/fleet-dev/fleet_dev.awl` byte-identity stands; the sealed
  fleet-ops scripts remain line 4's. The strike list becomes the CONTRACT OF
  THE CUTOVER — a staged act after landing, owned by staging.
* ⛔ **No shims, no compat**: the surface lands whole or not at all.
* ⛔ **The builder owns projection, never content**: per-lane wall sets,
  briefs, judge duties and FINDINGS stay authored. Operator law stays law —
  author ≠ reviewer handoff, fire authority, venue choice, receipt-first
  ordering, red-first evidence ordering.
* ⛔ **Teardown deletion authority is not widened** by any declarative
  replacement (design §8 verbatim: root pinned, shape-checked, the check gating
  the `rm` in the same unit, leftover trees remain refusals).
* **Q5 consumed, not implemented**: venue binding stays the deploy surface's
  (Q5 §2a) and lands with the ops-script retirement, LAST. No venue-binding
  hole beyond the existing `{workspace_root}`
  (`crates/aion-awl/src/workspace_root.rs:47`), mechanically fenced as in
  CUT1-R3 (`CommandCause::CwdPlaceholderUnknown`, raised at
  `src/command/check.rs:403,417`) — **conformance-read and consumed, never
  re-implemented**.
* **Security-adjacent surfaces** — token minting inside recipes (BRIEF-R3/R5
  touch the mint step's neighbourhood, e.g. `speech1.STAGING.md:24–25`),
  receipts must not widen token exposure, allowlist containment: pointers to
  Cally Ray stand exactly as the design brief carries them. Minting and
  rotation requirements come FROM her lane and are never proposed here.
* **Refusal causes**: every new cause joins cut-1's closed enum BY DESIGN
  CHANGE recorded in the report (the §6 law), layer-named per the
  carrier-contract law, `observed_bytes` always, forbidden causes
  unrepresentable at construction.

**Instrument walls, measured in this tree:**

* ⛔ **No `fmt --check` gate.** `cargo fmt --all` is APPLIED before commit —
  Tom's ruling, docs `2ab9689`, carried at `.fleet/gate-entry.sh:10–11`. The
  `seqlegs` specimen still runs `fmt --all --check` as a leg; it is a SPECIMEN
  of the pre-ruling shape, never a model.
* ⛔ **`AWL_CORPUS_BLESS` is never set.** The manifest
  (`crates/aion-awl/tests/fixtures/corpus/manifest.txt`, 431 records) was
  generated at the pre-command-declaration grammar; re-blessing it would erase
  cut-1's byte-identity proof along with this cut's.
* ⛔ **Do not change `builtins::BUILTIN_TYPES`.** `tests/editor_assets.rs` pins
  it to `tools/tree-sitter-awl/grammar.js`, outside the wall and outside this
  cut's acceptance (tree-sitter rides Tom's pending word). New words are
  POSITIONAL words — measured, not preferred: `receipt` already stands in 30
  documents of this tree, `parameter` in 20, `needs` in 17, `recipe` in 2.
  Reserving any would break documents that exist.
* ⛔ **Do not edit any existing gate to make it pass.** `#[allow]`,
  `#[expect]`, `#[deny]`, `#[ignore]`, `_var` renames and `#[cfg(any())]` are
  bypasses, not fixes. `crates/aion-cli/tests/console_guide_asset.rs` and
  `crates/aion-awl/tests/guide_reference_gate.rs` are **read, not edited** —
  the asset and the reference move to satisfy them.
* ⛔ **No arbitrary limits, no assumed defaults.** Absence stays a legal
  declared state wherever cut-1 made it one; nothing here invents a number.
* ⛔ **Never one blocking sleep sized to an expected duration.** Any spawned
  process is killed only by its own recorded pid.
* ⛔ **Nothing outside the wall is modified**, and nothing staged inside
  `.fleet/` is deleted — including F-STAGE-1's duplicate specimens, which are
  reported and left in place (the estate's never-destroy law).

---

## 4. EVIDENCE DUTIES

**Red-first throughout.** Every refusal cause, every refused grammar form, the
cycle refusal, the gate-leg-context refusal, the census mismatch and the
excluded-term family land with **refusing specimens BEFORE** the passing paths.
This is mechanical here, not a review note: `tests/command_refusals.rs:285
every_cause_has_a_refusing_specimen` iterates the enum, so a cause minted
without a red document fails its own step.

**Bytes, not memory.** Every fixture cites source **REPO**, **PATH** and commit
**DIGEST**, and re-hashes what it cites on the run. Verified at planning time
and re-verified by the leg: all seven BRIEF-R7 corpus exhibits match the RIG
table (`032ee3d7…`, `b51f60f7…`, `4e90bc3b…`, `b1db6ae3…`, `1150e466…`,
`77f0333f…`, `1a953152…`) and all four cut-1 gate specimens match cut-1's
digests (`1e1e3ad2…`, `ae66bb0b…`, `ef5b82a8…`, `e1544910…`). Fixtures cite the
OUTER specimen paths, never F-STAGE-1's duplicate copies.

**Measured, not remembered — including the brief's own figures.** BRIEF-R4
calls the haem-wall exhibit the "×4-copies" case; at the pinned bytes the
allowlist appears **verbatim twice** (`fix_prompt`, `legs[0].prompt`) and is
referred to without restatement twice more (`fix_prompt` again, `judge_prompt`),
with the gate script as the mechanical rendering. The fixture states what it
measured; a divergence from the brief's phrase is reported (candidate
**F-R7D-1**), never rounded to fit.

**Genericity proven at the mechanism, not just the exemplar** (grill fold A).
The ratchet-shaped fixture is the named exemplar and cannot stand alone: the
compare's genericity is additionally proven by an enum-derived property test
that enumerates the full closed-cause vocabulary **programmatically from the
enum** (`src/command/cause.rs:186`), asserts uniform consumption of every
member, and asserts the compare surface itself never matches on cause identity.

**The dependency-clean claim is measured** (grill fold B). The report carries
`crates/aion-awl/Cargo.toml`'s dependency DELTA for this cut. Required: zero
execution-side additions — no process, runtime or transport machinery entering
the front. A non-zero delta on that axis is a reportable finding against the
ratification, not a thing to absorb.

**Discovery, not enumeration.** The round-trip corpus is discovered repo-wide
by extension at run time and the discovered list IS the completeness claim —
443 documents discovered at this HEAD against a 431-record baseline, with a
discovery floor of 400 guarding a walk that stopped looking.

**Findings are reported, never patched silently.** Spec-vs-implementation
discrepancies are reported WITH BYTES. Four are already open before the first
line of code and must reach the report whatever their resolution: **C-1**
(`excluded_term` absent from the enum the brief says carries it), **C-2**
(`literal_where_derived`'s trigger), **F-STAGE-1** (duplicated staged
specimens), and cut-1's still-open **F-R6-1** (hardening expressible as a value
but not as a presence — a design question this cut inherits and does not
close). Where this leg and the ratified sources disagree, **the sources
govern** and the disagreement is the finding.

**No silent thinning.** For any requirement this leg cannot complete: name the
requirement (BRIEF-Rn, disambiguated), report the stop. A partial landing is
not a landing.

---

## 5. DELIVERABLES

1. **`.fleet/reports/build.md` — a COMMITTED FILE.** The payload copy is a
   courtesy; the file is the deliverable. ⚠️ That path currently holds **cut-1's**
   report, carried into main by the landing; this leg replaces it wholly.
   Contents, in order: C-1 first; the rest of the conformance read with
   movement named; the census verbatim (pathspec + zeros + triage); staging
   conformance including F-STAGE-1; the placement decision as taken with its
   reasoning; the front crate's manifest dependency delta; every design change
   (new causes, new layers) with its reasoning; the BRIEF-R7 fixture set's
   completeness claim; the ratchet-ruling fixture's outcome PLUS the
   enum-derived genericity property test's result; every inexpressibility
   finding; and any requirement not completed, named, with its stop.
2. **The recipe surface** under `crates/aion-awl/src/command/recipe/` — recipe
   and dependency declarations, parameter kinds with declared derivations, slot
   refusal and sweep-by-claim, wall projection, specimen firing and receipts,
   the reference census — with its extended cause and layer vocabulary.
3. **The executing verb** under `crates/aion-cli`, consuming the front's
   checked output, with the front's dependency budget intact and measured.
4. **Fixtures**: the recipe-side absorbed-production conformance corpus with
   provenance headers; the BRIEF-R7 composer-corpus exhibits (a)–(e) with their
   completeness claim; the ratchet-shaped record fixture and the enum-derived
   genericity property test.
5. **Teaching rows** for every new word: guide entry + checked examples,
   grammar-table position, AWL-REFERENCE **§18** (both copies), LSP
   hover/completion tests, regenerated
   `apps/aion-ops-console/src/features/authoring/assets/awl-guide.json`.
6. **A CLEAN working tree.** The gate reds on dirty by design — a gate that
   runs on the working tree attests the working tree.

## 6. EXIT

`.fleet/gate-entry.sh` GREEN on the clean committed tree: toolchain 1.97.1
pin-honored from `rust-toolchain.toml` (source-asserted, not merely
version-matched — verified available and honored on this venue today); tree
CLEAN; census within the wall with the deliverable present under
`crates/aion-awl/`; clippy `-D warnings` clean; a nonzero count of new test
annotations under `crates/aion-awl*`; the workspace suite listed and green.

Then, per RIG.md's acceptance path: Waffles' landing re-gate at his desk on the
pushed result branch — **who must be told about C-1 in the first section of the
report, because it changes what this cut was permitted to mint** — then
Vesper's owner review and landing, then Calliope's conformance read of the
landed surface against her brief, and finally the strike list as the contract
of the cutover, owned by staging.
