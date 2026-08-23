# LEG — `build` (the only leg of this flight)

One leg. RIG.md: *"Single leg `build` — the surface lands whole (no-shims wall);
leg boundaries inside one crate family would manufacture merge conflicts."* No
other leg exists, and none is to be invented. The ordered work is `.fleet/plan.md`
S1–S10; this document states the leg's duties, surface, walls, and deliverables.

---

## 1. FIRST DUTY, BEFORE ANY WORK — the has-this-already-happened census

**Answer it before touching a file.** The brief's first question: on this clone
of current main, prove by search that no part of this contract already exists —
a command-declaration construct in the AWL grammar (search the lexer/parser word
tables), an argv emitter, an `admits_dash`/end-of-options property in any
spelling, `{{`-interpolation handling outside string display, any Just-grammar
conformance fixtures.

The duty has three parts and all three are mandatory:

1. **State the pathspec.** An undeclared corpus is not a census. Name what was
   searched and what was excluded.
2. **State the zero, or triage every hit.** A hit is not a failure; an untriaged
   hit is. For each, say what it is, cite it at `file:line`, and rule whether it
   satisfies, blocks, or merely neighbours the contract.
3. **Carry it into `.fleet/reports/build.md` verbatim.** The census is a report
   section, not a private step.

This leg's census is executed and recorded at `.fleet/plan.md` §0: five
constructs sought, **five zeros**, one adjacent hit triaged
(`crates/aion-awl/src/ast/command.rs:1–272` — a string-command **scanner** for
`run "…"` bodies that never emits an argv, has no options/operands distinction,
no `--` property, and interpolates `$name`, not `{{name}}`). The leg **re-affirms**
that census against its own HEAD before S1 and reports any drift. It does not
re-litigate it; it confirms it or reports movement.

Adjacent and equally mandatory: the **seam re-derivation**
(`.fleet/plan.md` §1). A brief's citations are claims about the commit they were
derived on. Measured: zero movement in the AWL family between `c590712f5` (design
derivation) and `5e2fb5a3e` (this base) across five intervening commits. Report
the fact, not the assumption.

---

## 2. SURFACE — what this leg builds

AWL gains a **typed command-declaration surface** — structural argv, typed
check-time interpolation, declared env/cwd/timeout — whose emitter and checker
make the wall-flight2 defect family **unwritable**, proven expressible over the
diff-census gate-entry family as fixtures, with every pre-existing AWL document
byte-identical and every new word carrying its structural teaching row.

Requirements, in the leg's execution order (`.fleet/plan.md` §3):

| step | requirement | one-line surface |
|---|---|---|
| S1 | R8a | repo-wide `**/*.awl` corpus baseline, taken **before** any grammar change |
| S2 | R4 | the closed cause-code vocabulary, four layers, red-first |
| S3 | R1a | structural argv: grammar, AST, parser; `admits_dash` declared, never inferred |
| S4 | R2 | `{{parameter}}` typed, check-time, closed; backticks refused; `literal_where_derived` |
| S5 | R3 | env/cwd/timeout as declarations; absence of a timeout is a legal state |
| S6 | R5 | the absorbed Just subset + CC0 conformance fixtures with provenance headers |
| S7 | R1b, R6 | the emitter (`--` unconditional for admitting programs) + one shape, four parameter sets |
| S8 | R7 | teaching rows: guide sentence, checked example, §17, LSP hover, console asset |
| S9 | R8b | corpus close-out, clippy `-D warnings`, `fmt --all` applied, 500-line law |
| S10 | — | committed report, clean tree, gate |

Placement (D2, `.fleet/plan.md` §2): a module family
`crates/aion-awl/src/command/` — eight source files, `mod.rs` declarations-only,
every file under 500 lines. The sibling-crate fallback `crates/aion-awl-command`
stays live **only** if that budget cannot hold; the wall admits either, and the
outcome is reported as a finding whichever way it lands. The item-5 pricing
decision remains the owner's at landing, not this leg's.

---

## 3. WALLS — binding on every step

**Path wall.** Changes only under `crates/aion-awl*` (family prefix), the
ops-console guide asset if regenerated, `Cargo.toml` / `Cargo.lock`, and
`.fleet/` — plus, per **D1** and by exactly one path,
`examples/assistant/resources/AWL-REFERENCE.md`, because the wall as staged
admits `crates/aion-awl/guide-embed/AWL-REFERENCE.md` while
`crates/aion-awl/tests/guide_reference_gate.rs:221` pins that embed
byte-identical to the unadmitted twin — making the staged cut ungreenable for
**any** new AWL word. D1 is FINDING F-RIG-1 and is the **first line** of the
report. No second path is added under its cover.

**Structural walls, from the brief:**

* ⛔ **Amendment 1** — no recipe, dependency, or assembly form is *reachable*
  from a gate-leg command declaration. The grammar refuses it; a test proves the
  refusal. The reason (a reader sees the exact command that ran) rides in the
  diagnostic text.
* ⛔ **Amendment 2** — the ratchet family (`liminal22`, `sleepreap`) is untouched:
  not converted, not retired, no check absorbed. Its five checks lift to
  requirements in a later flight.
* ⛔ **No deployed artifact moves** — `fleet_dev.awl` byte-identity stands, no
  `.fleet/` script retires, the sealed scripts wait on item-2 typed capture.
  R6 is **expressibility proof, NOT deployment**.
* ⛔ **No shims, no compat** — the surface lands whole or not at all; nothing
  dual-paths.
* **Wall projection (design R6) is OUT** — ruled to the recipes cut, staging
  line 2.
* **Q5 consumed, not implemented** — no venue-binding holes beyond the existing
  `{workspace_root}` (`crates/aion-awl/src/workspace_root.rs:47`); the
  closed-delta-set fence is mechanical in R3.
* Security-adjacent surfaces (token minting, allowlist containment) are pointers
  to Cally Ray; nothing here analyzes them.

**Instrument walls, measured in this tree:**

* ⛔ **No `fmt --check` gate.** `cargo fmt --all` is APPLIED before commit. Tom's
  ruling, docs `2ab9689`, canonical in `GATE-EXTRACTION-STANDARD.md`. The
  `seqlegs` specimen still runs `fmt --check` as a leg — it is carried as a
  specimen of the pre-ruling shape, never as a model.
* ⛔ **Do not change `builtins::BUILTIN_TYPES`.** `crates/aion-awl/tests/editor_assets.rs:171`
  pins it to `tools/tree-sitter-awl/grammar.js`, which is outside the wall and
  outside this cut's acceptance (tree-sitter rides Tom's pending confirmation).
* ⛔ **Do not edit any existing gate to make it pass.** `#[allow]`, `#[expect]`,
  `#[deny]`, `#[ignore]`, `_var` renames, and `#[cfg(any())]` are bypasses, not
  fixes. `crates/aion-cli/tests/console_guide_asset.rs` and
  `crates/aion-awl/tests/guide_reference_gate.rs` are **read, not edited** — the
  asset and the reference move to satisfy them.
* ⛔ **No arbitrary limits, no assumed defaults.** An unset timeout defers to the
  engine's own behaviour, never to a number this leg invents.

---

## 4. EVIDENCE DUTIES

**Red-first throughout.** Every R4 cause code, every refused grammar form, and
R1's leading-dash family land with **refusing specimens BEFORE** the passing
paths. A cause code with no red specimen fails its own step's test.

**Bytes, not memory.** R6's fixtures assert at the bytes against the four pinned
specimens under `.fleet/fixtures/gate-scripts/` — digests re-verified against the
RIG table and matching (`1e1e3ad2…`, `ae66bb0b…`, `ef5b82a8…`, `e1544910…`). Each
fixture names its source **repo**, **path**, and **commit digest**
(repo-qualified per the grill fold: gate-entry specimens live in the docs repo's
dispatch-rigs *and* in aion rig branches' `.fleet/`, so cite whichever repo holds
the bytes). Normalizations are declared in the fixture, one by one.

**Discovery, not enumeration.** R8's corpus is discovered repo-wide by extension
at run time and **listed** by the test — which documents, how many, from which
roots. Measured today: **431** across five top-level roots (`crates/` 345,
`docs/` 47, `workflows/` 19, `examples/` 16, `gate-logs/` 4). `examples/` and
`workflows/` together are 35 — **8%** — so the hand-enumerated claim the brief
was suspicious of would have missed 92% of its own corpus.

**Findings are reported, never patched silently.** Spec-vs-implementation
discrepancies are reported WITH BYTES. Two are already open and must be carried
to the report whatever their resolution: **F-RIG-1** (D1, the wall) and the
`admits_dash` declared-unit question — `git tag -- <name> <sha>` admits while
`git diff --name-only <BASE>..HEAD` must not (a `--` there reinterprets the
revision range as a pathspec and silently changes the census's meaning), so the
declared unit is probably program+subcommand, not the bare program. Where this
leg and the ratified sources disagree, **the sources govern** and the
disagreement is the finding.

**No silent thinning.** For any requirement this leg cannot complete: name the
requirement, report the stop. A partial landing is not a landing.

---

## 5. DELIVERABLES

1. **`.fleet/reports/build.md` — a COMMITTED FILE.** The payload copy is a
   courtesy; the file is the deliverable. Contents, in order: F-RIG-1 first; the
   census (pathspec + zero or triage); the re-derived seam cites with movement
   named; the placement decision as taken with its reasoning; the corpus
   round-trip completeness claim; every R6 inexpressibility finding; and any
   requirement not completed, named, with its stop.
2. **The surface** under `crates/aion-awl/src/command/` with its refusal
   vocabulary, parser, checker rules, and emitter.
3. **Fixtures**: the Just-subset conformance corpus with provenance headers, and
   the diff-census family — one shape, four parameter sets — under
   `crates/aion-awl/tests/fixtures/command/`.
4. **Teaching rows** for every new word: guide entry + checked examples, grammar
   table position, AWL-REFERENCE §17 (both copies), LSP hover/completion tests,
   regenerated `apps/aion-ops-console/src/features/authoring/assets/awl-guide.json`.
5. **The corpus round-trip test** with its committed base manifest.
6. **A CLEAN working tree.** The gate reds on dirty by design — a gate that runs
   on the working tree attests the working tree.

## 6. EXIT

`.fleet/gate-entry.sh` GREEN on the clean committed tree: toolchain 1.97.1
pin-honored from `rust-toolchain.toml` (verified available on this venue today);
tree CLEAN; census within the wall with the deliverable present under
`crates/aion-awl/`; clippy `-D warnings` clean; nonzero new test annotations under
`crates/aion-awl*`; the workspace suite listed and green.

Then, per RIG.md's acceptance path: Waffles' landing re-gate — **who must be told
about D1 in the first line of the report, because he will be running the amended
script** — then Vesper's owner review and landing, then Calliope's conformance
read against her design brief.
