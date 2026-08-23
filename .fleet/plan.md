# PLAN — awl-cut1 flight1, build leg

Authored by the flight before any implementation, against `.fleet/BRIEF.md`
(Calliope, owner-grilled by Vesper) and `.fleet/RIG.md` (staged by Waffles).
Every requirement below is grounded at `file:line` in **this** tree, not in
the tree the design brief's seams were derived on.

**Tree receipt at planning time.** HEAD `8eaa456faaf603863993d26dbabc565293e3da9d`,
working tree CLEAN. BASE `5e2fb5a3ed32b53d48dbc585aa914acd056458c6` verified an
ancestor of HEAD; `git diff --no-ext-diff --name-only BASE...HEAD` names exactly
the eight rig files and nothing else.

**Toolchain receipt.** `rust-toolchain.toml:2` pins `channel = "1.97.1"`;
`rustc --version` = `1.97.1 (8bab26f4f 2026-07-14)`; `rustup show active-toolchain`
= `1.97.1-x86_64-unknown-linux-gnu (overridden by
'/home/rocketfish/fleet/runs/awl-cut1-flight1/repo/rust-toolchain.toml')` — the
pin is HONORED from the pin file, not by default-coincidence. `cargo-nextest`
present. Gate leg 1 is satisfiable on this venue today.

**Specimen digests.** All four gate scripts under `.fleet/fixtures/gate-scripts/`
re-hashed and matched against the RIG table byte for byte
(`1e1e3ad2…`, `ae66bb0b…`, `ef5b82a8…`, `e1544910…`). Fixtures cite these bytes.

---

## §0 — HAS THIS ALREADY HAPPENED? (the census, answered before any work)

Pathspec for every search below: the whole worktree, excluding `.git/`,
`target/`, and `node_modules/`. Each of the brief's five named constructs:

| construct sought | pathspec | result |
|---|---|---|
| command-declaration construct in the AWL grammar | `crates/aion-awl/src/lexer/**`, `crates/aion-awl/src/parser/**` | **ZERO.** `Keyword` (`crates/aion-awl/src/lexer/tokens.rs:101`) carries 58 variants (`:232`); none is `command`, `program`, `option`, `operand`, `env`, `cwd`, or `admits`. `POSITIONAL_WORDS` (`:70`) is `[&str; 6]` and carries none of them. The top-level declaration dispatch (`crates/aion-awl/src/parser/document.rs:450–495`) admits exactly `const`/`type`/`worker`/`child`/`subflow`/`step`. |
| an argv emitter | `crates/`, `sdks/`, `gleam/` | **ZERO in the AWL family.** Every `argv` hit is downstream plumbing outside `crates/aion-awl*`: `crates/aion-integration-norn/src/harness.rs:111`, `crates/aion-cli/src/main.rs:626`, `crates/aion-fleet-plan/src/verify.rs:284`, `crates/aion-integration-acp/src/harness.rs:904`. None derives an argv from an AWL declaration. |
| `admits_dash` / end-of-options property, any spelling | whole worktree, case-insensitive, incl. `admits-dash`, `end_of_options`, `end-of-options`, `double_dash`, `dash_dash` | **ZERO.** The only hits in the tree are `.fleet/BRIEF.md:34` and `:64` — the brief describing what does not exist. |
| `{{`-interpolation handling outside string display | `crates/aion-awl*` | **ZERO as a language construct.** Every `{{` in the crate is a Rust `format!` brace escape emitting literal Gleam braces (`crates/aion-awl/src/emitter/codecs.rs`, `forks.rs`, `flows.rs`, …) or a doc-comment escape (`crates/aion-awl/src/ast/command.rs:190`, `:245`). No `{{name}}` is ever lexed, parsed, or resolved. |
| Just-grammar conformance fixtures | whole worktree | **ZERO.** The only `justfile` in the tree is `.meridian/workflows/onatopp-dev-norn/justfile`, a Meridian dispatch script, not a fixture; `CONTRIBUTING.md:32` mentions justfiles only to say the repo has none. No CC0 suite is vendored anywhere. |

**Triage of the one adjacent hit that is NOT the contract.**
`crates/aion-awl/src/ast/command.rs:1–272` (+ `command_tests.rs`) is a
pre-existing **string**-command surface for declared `run "…"` action bodies. It
is prior art and it is *not* this cut:

* it **scans**, it never emits — `command.rs:24–27` states it outright
  ("Nothing here renders a command back into a string");
* its interpolation is `$name` / `${name}` (`command.rs:169–210`), not `{{…}}`,
  and it mirrors the worker SDK's `CommandTemplate::parse` (`command.rs:10–14`);
* it has no options/operands distinction, no types, no `--` property, no env,
  cwd, or timeout — it carries only `references` and `program_end`
  (`command.rs:44–51`).

It therefore neither satisfies nor blocks R1–R3. **The wall-flight2 defect family
is live in this tree**: nothing here can place `--`, because nothing here
constructs an argv at all. The corpus is declared; the zero is stated; the one
hit is triaged.

---

## §1 — SEAM RE-DERIVATION (a citation is a claim about a commit)

The design brief's seams were derived at aion `c590712f5` (v0.20.0). This
flight's base is `5e2fb5a3e`. Measured:

```
git diff --no-ext-diff --stat c590712f5 5e2fb5a3e -- crates/aion-awl \
    crates/aion-awl-lsp crates/aion-awl-package \
    examples/assistant/resources/AWL-REFERENCE.md \
    apps/aion-ops-console/src/features/authoring/assets/awl-guide.json
```

→ **empty.** Five commits separate the two shas; **none touches the AWL crate
family or either teaching asset.** MOVEMENT: NONE. Every design-brief seam
citation in the AWL family stands unchanged at this flight's base. This is
reported as a fact, not assumed.

---

## §2 — DECISIONS TAKEN (both reportable, both the owner's to overturn)

### D1 — the staged wall makes the staged cut ungreenable; it is amended by exactly one path

**The defect, at the bytes.** The rig's wall admits `crates/aion-awl` as a
prefix, which includes the packaged reference copy
`crates/aion-awl/guide-embed/AWL-REFERENCE.md`. But
`crates/aion-awl/tests/guide_reference_gate.rs:221` — a **pre-existing** test in
the crate the wall admits — pins that embed byte-identical to the authored twin
at `examples/assistant/resources/AWL-REFERENCE.md` (`guide/entry.rs:75`), which
the wall does **not** admit. Editing the admitted file therefore mechanically
forces an edit to an unadmitted one. The wall as written is internally
inconsistent.

**Why this is fatal rather than inconvenient — R7 cannot be thinned around it.**
Three pre-existing gates compose into a hard chain:

1. `guide_reference_gate.rs:204` — `every_cited_section_actually_discusses_…`'s
   sibling `the_inventory_covers_the_whole_vocabulary` asserts
   `guide::all().len() == Keyword::ALL.len() + POSITIONAL_WORDS.len()` by
   **equality**. Any new word forces a glossary entry.
2. `guide/lookup.rs:25` — `for_keyword` is an exhaustive `match`; a new
   `Keyword` fails to compile until its entry is written.
3. `guide_reference_gate.rs:126` — `every_cited_section_actually_discusses_the_word`
   requires the cited section's **body text** to use the word, whole-word matched
   (`:151`).

Measured against the reference's sixteen sections: `command`, `program`, `argv`,
`shell` occur in §4/§16; `cwd` in §16; `operands` in §6 — but **`option`,
`operand`, `env`, `admits`, `variadic`, and `hardening` occur in NO section**.
So a `command` surface that declares options, operands, env bindings, and the
admits-`--` property cannot cite any existing section, cannot be filed anywhere,
and cannot compile. Within the wall as staged, **no new AWL word can land at
all** — which takes R1, R2, R3 and R7 with it. This is not a partial-landing
question; it is the whole cut.

**Disposition.** RIG.md states its own precedence: *"Sources govern the brief;
the brief governs this rig."* R7 is BINDING and the rig's wall is subordinate to
it. The build leg therefore adds **exactly one alternative** to the wall regex in
`.fleet/gate-entry.sh` — `examples/assistant/resources/AWL-REFERENCE\.md` — and
**nothing else**, with the amendment documented in a comment block inside the
gate script itself (so no one can read that script without seeing it) and carried
as **FINDING F-RIG-1, first line of `.fleet/reports/build.md`**, with the forcing
bytes cited. The alternative — leaving the wall intact and reporting a total
non-landing of a five-requirement cut over one markdown twin — serves nobody.
The amendment is one line to revert and the ruling is Waffles' at re-gate and
Vesper's at landing.

**What D1 does NOT license.** No other path is added. The flight does not touch
`crates/aion-cli/tests/console_guide_asset.rs`, `tools/tree-sitter-awl/**`, or
`editors/**`; it does not relax, reword, or delete any existing gate; and it does
not choose word spellings to dodge a documentation check.

### D2 — placement: a module family inside `crates/aion-awl` (the brief's assumption, upheld)

The brief names module-vs-sibling-crate revisable. Upheld as a module family,
grounds recorded now and re-tested at the end of the leg:

* the surface is a **language** surface — it needs the crate's lexer
  (`src/lexer/tokens.rs`), stream (`src/parser/stream.rs`), spans
  (`src/spanned.rs`), and canonical printer (`src/printer/document.rs`); a
  sibling crate either forks them or takes a circular dependency;
* R7's teaching machinery is *inside* this crate (`src/guide/lookup.rs:25` is an
  exhaustive match over `crate::Keyword`) — a sibling crate cannot add a keyword;
* the 500-line law is **per file**, not per crate, and is satisfiable: the plan
  below lands eight source files, each budgeted well under the limit.

The fallback `crates/aion-awl-command` is retained as a live option **only** if
S3–S6 cannot hold any file under 500 lines; the wall admits it either way
(`crates/aion-awl` is a prefix). Either outcome is reported as a finding, and the
item-5 pricing decision remains the owner's at landing.

---

## §3 — ORDERED WORK (foundation-first, red-first; S-numbers are the leg's order)

### S1 — corpus baseline, taken BEFORE any grammar change (R8, first half)

Ordering is load-bearing: a "before and after" claim needs the *before* recorded
while the grammar is still the base grammar.

* Discover `**/*.awl` repo-wide at run time (never enumerated). Measured today:
  **431 documents** across five top-level roots — `crates/` 345 (of which
  `aion-awl` 332, `aion-server` 8, `aion-cli` 4, `aion` 1), `docs/` 47
  (`design` 24, `authoring` 18, `evidence` 5), `workflows/` 19, `examples/` 16,
  `gate-logs/` 4. The brief's amendment-2 suspicion is confirmed at the bytes:
  `examples/` and `workflows/` together are **35 of 431 — 8%** of the population,
  so a hand-enumerated claim over those two roots would have missed **92%** of
  the corpus it claimed to cover.
* Land `crates/aion-awl/tests/corpus_roundtrip.rs`: it walks the repo root,
  records for each document its path and its parse outcome (canonical
  `print(parse(src))` on success, the exact `ParseError` message + span on
  failure), and compares against a pinned manifest committed under
  `crates/aion-awl/tests/fixtures/corpus/manifest.txt`, generated in this step at
  base grammar. The test **lists** its corpus (which documents, how many, from
  which roots) so the completeness claim is printed, not assumed, and it fails if
  discovery returns fewer roots than the manifest names (an instrument that
  stopped looking must not read green).
* Model, not new invention: `crates/aion-awl/tests/parser_corpus.rs:74` already
  uses a shrink-guard; this extends the idea repo-wide.

### S2 — the refusal vocabulary (R4) — the spine every later R refuses through

* `src/command/cause.rs`: a **closed** `#[non_exhaustive]`-free enum of cause
  codes with a stable `as_str()` wire rendering, modelled on the landed posture
  at `crates/aion-fleet-plan/src/error.rs:58–87` (`ConclusionCause`) — read as
  prior art, **not** modified (that crate is outside the wall).
* Layer-naming per the carrier-contract law: **four distinct** error types, never
  one catch-all — parse-layer, check-layer, emit-layer, specimen-fire-layer,
  mirroring the `ConclusionCause` / `VerifierError` split at
  `crates/aion-fleet-plan/src/error.rs:97`.
* Each refusal carries `observed_bytes` (the literal bytes seen), a span, and its
  cause. `thiserror` throughout; no `unwrap`/`expect`/`panic`.
* Constructor discipline: an empty or catch-all cause is **unrepresentable** —
  the constructor returns a typed error, tested.
* **RED FIRST:** every cause code lands with its refusing specimen before any
  passing path exists. A cause with no red specimen is a build failure of this
  step, enforced by a test that iterates the enum.
* Existing `CheckError` (`src/checker/error.rs:5–19`) is span+message only and is
  **not** widened; the new family lands beside it, per R1's file note.

### S3 — structural argv: grammar, AST, parser (R1, first half)

* `src/command/decl.rs` — `CommandDecl { program, options, operands, admits_dash,
  env, cwd, timeout }`, typed throughout; options named+typed, operands
  positional+typed. No field is a string a shell re-parses.
* `src/command/parse.rs` — the top-level `command` declaration, wired into the
  dispatch at `src/parser/document.rs:450`; the dispatcher's refusal string at
  `:486–489` is extended, which `crates/aion-awl/tests/grammar_parser_gate.rs:72`
  gates as `DECLARATION_REFUSAL` (both must move together or the gate reds).
* **Amendment 1 is structural, not advisory:** a gate-leg command declaration has
  no production for a recipe, a dependency edge, or multi-step assembly — the
  grammar cannot represent one. A refusal test proves it per named form, and the
  diagnostic text carries the reason (a reader sees the exact command that ran).
* `admits_dash` is **declared, author-stated, per-program** — never inferred. A
  declaration naming a non-admitting program without its leading-dash refusal
  specimen is a check error with a span (R1's mandatory specimen).
* **Open question already surfaced, to be resolved in this step and reported
  either way:** the declared unit is probably the *invocation* (program +
  subcommand), not the bare program. Evidence in this tree:
  `git tag -- <name> <sha>` is the R1 canonical cure (admits), while the four
  specimens' census leg is `git diff --name-only <BASE>..HEAD`
  (`.fleet/fixtures/gate-scripts/seqlegs-impl.gate-entry.sh:15`) where a `--`
  before the revision operand would reinterpret `BASE..HEAD` as a **pathspec** and
  silently change the census's meaning. One `git` cannot be both. If the design
  brief's §R1 says otherwise, the source governs and the disagreement is the
  finding.

### S4 — typed interpolation, check-time, closed (R2)

* `src/command/interpolate.rs` — `{{parameter}}` resolves **only** over declared,
  typed parameters, at check/compile time, into whole-cloth argv. Never runtime
  composition, never a templating language (Q5's fence, restated in the module
  doc so nobody widens it later).
* Backtick command substitution is **refused with its own cause** — execution at
  evaluation time is outside the determinism boundary (CLAUDE.md invariant 2).
* File contents enter as **addresses**, never spliced strings.
* `literal_where_derived` — a hand literal in a fill position for which a declared
  derivation exists (a base SHA, a control token, any declared re-derive
  parameter) is a composer refusal carrying the literal in `observed_bytes`.
* Determinism assertion: same declaration + same parameters ⇒ **byte-identical**
  argv, tested directly.
* Undeclared parameter reference ⇒ check error with a span.

### S5 — env, cwd, timeout as declarations (R3)

* Env bindings are **structural fields**; a `K=V` string-prefix form is
  **unparseable** (grammar, not lint) — tested as a parse-layer refusal.
* PATH/ulimit hardening is stated **once** on the executing declaration scope and
  inherited; declared twice in one scope is a check error (one-home discipline).
  The specimens show why this matters: only
  `.fleet/fixtures/gate-scripts/seqlegs-impl.gate-entry.sh:3–4` hardens
  (`export PATH=$HOME/.cargo/bin:$PATH`, `ulimit -n 65536`); the other three
  harden not at all — so hardening is a per-flight parameter, not a constant.
* Cwd is declared. `{workspace_root}` is the **one** admitted placeholder,
  consumed from the existing rule at `src/workspace_root.rs:47` (the literal) and
  `:110` (`harness_path_form`) — **not extended**. Any *other* brace-placeholder in
  cwd is a check error: the closed-delta-set fence, mechanical.
* **No new venue-binding holes.** Venue binding is the deploy surface's (Q5 §2a)
  and lands with the ops-script retirement, last.
* Timeouts are declared values with named owners, and **absence is a legal
  declared state** — no ceiling, no default, no invented number (estate
  no-assumed-defaults law). A declaration with no timeout passes check and emits
  no timeout; asserted by test.

### S6 — the absorbed Just subset, with the CC0 suite as conformance fixtures (R5)

* `src/command/just_subset.rs` — exactly the design §3 absorbed list (parameters
  with defaults, variadic tails, `{{…}}` syntax) and nothing more.
* Every **refused** form named in design §3 gets a refusal test, one per named
  form (absence-of-escape discipline). Refused suite cases are carried as
  **refusal fixtures**, never skipped.
* Fixture harness runs under `cargo test` (so `cargo nextest run --workspace`
  picks it up — gate leg 4c). Fixtures land under
  `crates/aion-awl/tests/fixtures/command/just-subset/`.
* **Provenance in every fixture header:** which Just suite file, CC0 — or, where
  the CC0 suite carries no specimen for an absorbed production, an authored
  fixture whose header **names its absence from the suite** (Waffles' amendment 1:
  completeness is a property of the question; the question must not be
  unanswerable on a technicality).
* Coverage assertion: every absorbed production has ≥1 fixture, checked by a test
  that enumerates the productions rather than a hand-kept count.

### S7 — the emitter, and the diff-census family proven expressible (R1 second half, R6)

* `src/command/emit.rs` — for a program declared **admitting**, `--` is placed
  between options and operands **unconditionally**: the family closes by
  construction, not by review. Byte-verified against the canonical fixture
  `git tag -- <name> <sha>` (the wall-flight2 cure) with a leading-dash operand.
* R6 fixtures: **ONE declared command shape + FOUR parameter sets**, landed under
  `crates/aion-awl/tests/fixtures/command/gate-census/`, each naming source
  **repo**, **path**, and **digest** (the grill's repo-qualification fold). The
  parameters, read off the specimens, are exactly RESULT §A1's five:

  | parameter set | base pin | deliverable set | allowlist | battery | env hardening |
  |---|---|---|---|---|---|
  | `haem-wall` (haematite `.fleet/gate-entry.sh` @ `87de93b6`) | `fd168fcb…` | `gates.json`, `scripts/release/cut-tag.sh`, `known-carries*`, `docs/evidence/` | docs-and-gates-only | none (empty Rust closure) | none |
  | `haem-wall-r2` (haematite @ `125cbaac`) | `a4134c3c…` | `scripts/release/cut-tag.sh`, `docs/evidence/` | same as above | none | none |
  | `seqlegs-impl` (aion @ `957de0a1`) | `c590712f…` | `^crates/.*\.rs$`, `^docs/evidence/` | (none declared) | fmt + clippy + `cargo test --workspace` | `PATH`, `ulimit -n 65536` |
  | `speech-1` (docs `dispatch-rigs/speech-1-python-wire/KIT-gate-entry.sh` @ `e98ca99`) | `__CUT_COMMIT__` | `sdks/liminal-py/pyproject.toml` | `sdks/liminal-py/`, `.fleet/` | pytest in a fresh venv | none |

* Emitted argv is byte-compared against each specimen's **actual** command text
  (`git diff --name-only <BASE>..HEAD`), normalized **only** for declared
  structural differences, each named in the fixture. Any diff-census check the
  shape cannot express is a **reportable finding**, never papered over —
  `speech-1`'s `__CUT_COMMIT__` sentinel and `seqlegs`' absent allowlist are
  already flagged as candidates.
* ⛔ **Expressibility proof only.** No `.fleet/` script retires, no rig converts,
  no deployed document moves. `workflows/fleet-dev/fleet_dev.awl` byte-identity
  stands and is asserted by S1's manifest.

### S8 — structural teaching rows for every new word (R7)

Executed **after** D1's one-path amendment is in place. For each new word:

* one `aion awl guide` sentence + complete checked example documents —
  `src/guide/declarations.rs` (entry shape at `src/guide/entry.rs:131–214`),
  examples under `src/guide/examples/`, run through the real parse+check pipeline
  by `every_entry_example_passes_the_real_checker`;
* a glossary arm in the exhaustive match at `src/guide/lookup.rs:25`;
* a grammar-table position in `src/guide/grammar.rs:30–60` — with the
  `grammar_parser_gate` equality probes
  (`crates/aion-awl/tests/grammar_parser_gate.rs`) satisfied in **both**
  directions for top-level;
* an **AWL-REFERENCE §17** section, written once into
  `examples/assistant/resources/AWL-REFERENCE.md` and copied to
  `crates/aion-awl/guide-embed/AWL-REFERENCE.md`, with a `Section::new(17, …)`
  const at `src/guide/entry.rs:129`. Each new word's spelling must occur in §17's
  body or `guide_reference_gate.rs:126` reds — this is R7's build-failure law,
  already mechanized;
* LSP hover/completion: `crates/aion-awl-lsp/src/glossary.rs:48/63` and
  `completion.rs:271` already **derive** from the one table, so the row appears by
  construction; the leg adds tests asserting hover returns the guide sentence for
  each new word **against the table, not a copy**;
* the ops-console asset regenerated via `aion awl guide --json >
  apps/aion-ops-console/src/features/authoring/assets/awl-guide.json`, which
  `crates/aion-cli/tests/console_guide_asset.rs:48` compares (that test is **read,
  not edited**).

⛔ Tree-sitter is **not** in this cut's acceptance (Tom's pending confirmation).
Constraint that follows: `builtins::BUILTIN_TYPES` must **not** change, or
`crates/aion-awl/tests/editor_assets.rs:171` reds and drags
`tools/tree-sitter-awl/**` (outside the wall) in with it. New words are keywords
or positional words, never builtin types.

### S9 — corpus close-out and estate discipline (R8, second half)

* Re-run S1's corpus test against the **new** grammar: all 431 discovered
  documents must parse to the same result. A single divergence is a grammar
  regression and stops the leg.
* `cargo clippy --workspace --all-targets -- -D warnings` clean. If clippy fires,
  **fix the code** — `#[allow]`, `#[expect]`, `_var` renames, `#[ignore]`, and
  `#[cfg(any())]` are bypasses, not fixes.
* `cargo fmt --all` **APPLIED** before commit. ⛔ **No `fmt --check` gate** — Tom's
  ruling, docs `2ab9689`, canonical in `GATE-EXTRACTION-STANDARD.md`. Note the
  contrast the fixtures record: `seqlegs-impl.gate-entry.sh:29` still runs
  `cargo fmt --all --check` as a gate leg; that is the pre-ruling shape and is
  carried as a **specimen**, never as a model.
* No file over 500 lines; `mod.rs` declarations-only (`src/command/mod.rs` carries
  `pub mod` and re-exports and nothing else); `thiserror` for the new error types;
  no `unwrap`/`expect` in library code.

### S10 — report, clean tree, gate

* `.fleet/reports/build.md` is **a committed file** — the payload copy is a
  courtesy, the file is the deliverable. It carries, in this order: F-RIG-1 (D1)
  first; the §0 census verbatim; the §1 seam re-derivation with movement named;
  the D2 placement decision as taken with its reasoning; the corpus completeness
  claim with its discovered list and count; every R6 inexpressibility finding; and,
  for any requirement not completed, **the requirement named and the stop
  reported** — never a silent thinning.
* `cargo fmt --all`, commit everything, verify `git status --porcelain` is EMPTY,
  then run `.fleet/gate-entry.sh` on the committed tree. The gate reds on a dirty
  tree by design: a gate that runs on the working tree attests the working tree.

---

## §4 — WALLS CARRIED INTO EVERY STEP

* Gate legs consume the **command half only**: no recipe, no dependency graph, no
  multi-step assembly is *representable* in a gate-leg context (Q4 amendment 1).
* The **ratchet family is fully excluded** — `liminal22` and `sleepreap` are
  untouched, no script retired, none of its five checks absorbed (Q4 amendment 2).
* **Wall projection (design R6) is OUT** — ruled to the recipes cut, staging line 2.
* **No shims, no compat, no dual paths.** The surface lands whole or not at all.
* **Q5 consumed, not implemented:** no venue-binding holes beyond the existing
  `{workspace_root}`; harness-profile blocks are the deploy lane's later work.
* Security-adjacent surfaces (token minting, allowlist containment) are pointers to
  Cally Ray; nothing here analyzes them.
* Path wall as amended by D1 and by nothing else:
  `^(crates/aion-awl|apps/aion-ops-console/src/features/authoring/assets/awl-guide\.json|examples/assistant/resources/AWL-REFERENCE\.md|Cargo\.toml|Cargo\.lock|\.fleet/)`.

## §5 — EXIT CONDITION

`.fleet/gate-entry.sh` GREEN on a CLEAN committed tree: toolchain 1.97.1
pin-honored from `rust-toolchain.toml`; census within the wall with the
deliverable present under `crates/aion-awl/`; clippy `-D warnings` clean; a
nonzero count of new test annotations under `crates/aion-awl*`; the workspace
suite listed and green. Plus `.fleet/reports/build.md` committed.
