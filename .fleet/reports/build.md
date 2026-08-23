# BUILD LEG REPORT — awl-cut1 flight1

Leg: `build` (the only leg). Brief `.fleet/BRIEF.md` (Calliope, owner-grilled
by Vesper); rig `.fleet/RIG.md` (Waffles); plan `.fleet/plan.md`; leg brief
`.fleet/legs/build.md`.

**Tree receipt.** Base `5e2fb5a3ed32b53d48dbc585aa914acd056458c6`, verified an
ancestor of this leg's HEAD. Toolchain: `rust-toolchain.toml` pins `1.97.1`;
`rustc --version` = `1.97.1 (8bab26f4f 2026-07-14)`; `rustup show
active-toolchain` names
`/home/rocketfish/fleet/runs/awl-cut1-flight1/legs/build/rust-toolchain.toml`
as the override origin — the pin is HONORED from the pin file, not by
default-coincidence.

---

## F-RIG-1 — the staged wall makes the staged cut ungreenable; amended by exactly one path

⛔ **Waffles will be running an amended `gate-entry.sh` at re-gate.** This is
the first section of the report because of that.

**The defect, at the bytes.** The rig's wall admits `crates/aion-awl` as a
prefix, which includes the packaged reference copy
`crates/aion-awl/guide-embed/AWL-REFERENCE.md`. But
`crates/aion-awl/tests/guide_reference_gate.rs:221` — a **pre-existing** test
in the crate the wall admits — pins that embed byte-identical to the authored
twin at `examples/assistant/resources/AWL-REFERENCE.md`
(`crates/aion-awl/src/guide/entry.rs:75`), which the staged wall does **not**
admit:

```
crates/aion-awl/tests/guide_reference_gate.rs:221
fn embedded_reference_matches_the_authored_document() -> TestResult {
    let authored = fs::read_to_string(repo_path(guide::REFERENCE_PATH))?;
    assert!(guide::reference_text() == authored, …)
```

Editing the admitted file therefore mechanically forces an edit to an
unadmitted one. The wall as written is internally inconsistent.

**Why fatal rather than inconvenient.** Three pre-existing gates compose into
a hard chain, all measured on this tree:

1. `guide_reference_gate.rs:204` — `the_inventory_covers_the_whole_vocabulary`
   asserts `guide::all().len() == Keyword::ALL.len() + POSITIONAL_WORDS.len()`
   **by equality**. Any new word forces a glossary entry.
2. `guide/lookup.rs:25` — `for_keyword` is an exhaustive `match`, and
   `for_positional_word` is pinned against the lexer's own inventory. A new
   word fails to compile or fails the count until its entry is written.
3. `guide_reference_gate.rs:126` —
   `every_cited_section_actually_discusses_the_word` requires the cited
   section's **body text** to use the word, whole-word matched (`:151`).

Measured against the reference's sixteen sections at base: `command`,
`program`, `argv` and `shell` occur in §4/§16 and `cwd` in §16, but
**`option`, `operand`, `env`, `admits`, `variadic` and `hardening` occur in NO
section**. So within the wall as staged, **no new AWL word can land at all** —
which takes R1, R2, R3 and R7 with it. That is the whole cut, not a partial
landing question.

**Disposition.** RIG.md states its own precedence: *"Sources govern the brief;
the brief governs this rig."* R7 is BINDING and the wall is subordinate to it.
This leg added **exactly one alternative** to the wall regex in
`.fleet/gate-entry.sh` — `examples/assistant/resources/AWL-REFERENCE\.md` —
and nothing else, with the reasoning written into the gate script itself
(`.fleet/gate-entry.sh:25–56`) so nobody can read that script without seeing
it. **One line to revert.** The ruling is Waffles' at re-gate and Vesper's at
landing.

**What D1 did NOT license.** No other path was added. This leg did not touch
`crates/aion-cli/tests/console_guide_asset.rs`, `tools/tree-sitter-awl/**`, or
`editors/**`; it did not relax, reword or delete any existing gate; and it did
not choose word spellings to dodge a documentation check. The census below
shows zero files outside the amended wall.

---

## §0 — HAS THIS ALREADY HAPPENED? (the census, run by this leg against its own HEAD)

**Pathspec, stated:** the whole worktree, excluding `.git/`, `target/` and
`node_modules/`. Nothing else excluded.

| construct sought | where searched | result |
|---|---|---|
| a command-declaration construct in the AWL grammar | `crates/aion-awl/src/lexer/**`, `crates/aion-awl/src/parser/**` | **ZERO.** `Keyword` (`lexer/tokens.rs:101`) carried 58 variants; none was `command`, `program`, `option`, `operand`, `env`, `cwd` or `admits`. `POSITIONAL_WORDS` (`:70`) was `[&str; 6]` and carried none of them. The top-level dispatch (`parser/document.rs:450–495`) admitted exactly `const`/`type`/`worker`/`child`/`subflow`/`step`. |
| an argv emitter | `crates/`, `sdks/`, `gleam/` | **ZERO in the AWL family.** 15 `argv` hits under `crates/aion-awl*`, every one a doc comment or a fixture narration; not one constructs an argv. Every functional `argv` is downstream plumbing outside the family (`aion-integration-norn/src/harness.rs`, `aion-cli/src/main.rs`, `aion-fleet-plan/src/verify.rs`, `aion-integration-acp/src/harness.rs`). None derives an argv from an AWL declaration. |
| an `admits_dash` / end-of-options property, ANY spelling | whole worktree, case-insensitive: `admits[_-]?dash`, `end[_-]?of[_-]?options`, `double[_-]?dash`, `dash[_-]?dash`, `opt(ion)?s?[_-]terminator` | **ZERO** outside `.fleet/`. Measured count outside `.fleet/`: **0**. The 8 hits inside `.fleet/` are the brief, plan and leg brief describing what does not exist. |
| `{{`-interpolation handling outside string display | `crates/aion-awl*` | **ZERO as a language construct.** All 104 `{{` occurrences are Rust `format!` brace escapes emitting literal Gleam braces (`emitter/codecs.rs`, `forks.rs`, `flows.rs`, …) or doc-comment escapes (`ast/command.rs:190,245`). No `{{name}}` was ever lexed, parsed or resolved. |
| Just-grammar conformance fixtures | whole worktree | **ZERO.** The only `justfile` in the tree is `.meridian/workflows/onatopp-dev-norn/justfile`, a Meridian dispatch script. `CONTRIBUTING.md:32` mentions justfiles only to say the repo has none. No CC0 suite was vendored anywhere. |

**Triage of the one adjacent hit that is NOT the contract.**
`crates/aion-awl/src/ast/command.rs:1–272` (+ `command_tests.rs`) is a
pre-existing **string**-command surface for declared `run "…"` action bodies.
It is prior art and it is not this cut:

* it **scans**, it never emits — `command.rs:24–27` says so outright
  ("Nothing here renders a command back into a string");
* its interpolation is `$name` / `${name}` (`:169–210`), not `{{…}}`, and it
  mirrors the worker SDK's `CommandTemplate::parse` (`:10–14`);
* it has no options/operands distinction, no types, no `--` property, no env,
  cwd or timeout — it carries only `references` and `program_end` (`:44–51`).

It therefore neither satisfies nor blocks R1–R3. **The wall-flight2 defect
family was live in this tree**: nothing there could place `--`, because
nothing there constructed an argv at all.

**A second adjacent hit, triaged (not in the plan's census, found by this leg).**
`crates/aion-cli/tests/awl_corpus_gate.rs` is a pre-existing corpus gate that
runs `aion awl check` over every LIVING `.awl` document. It is **not** R8's
instrument and is not a substitute: it excludes `fixtures`, `golden`,
`templates`, `docs/design`, `docs/archive`, `docs/evidence` and `gate-logs` by
class, and it asks a different question (does a living document still check
clean) than R8's (does every pre-existing document read the SAME before and
after). Its floor is 35 documents; R8's corpus is 431. Left untouched; R8's
instrument lands beside it, and the two exclusion sets are stated in each
file's own header so no reader mistakes one for the other.

---

## §1 — SEAM RE-DERIVATION (re-run by this leg; a citation is a claim about a commit)

The design brief's seams were derived at aion `c590712f5` (v0.20.0). This
flight's base is `5e2fb5a3e`. Measured here:

```
git diff --no-ext-diff --stat c590712f5 5e2fb5a3e -- crates/aion-awl \
    crates/aion-awl-lsp crates/aion-awl-package \
    examples/assistant/resources/AWL-REFERENCE.md \
    apps/aion-ops-console/src/features/authoring/assets/awl-guide.json
```

→ **empty.** Five commits separate the two shas (`c7706858`, `f1f301b8`,
`ed9da66f`, `603474bb`, `5e2fb5a3`); **none touches the AWL crate family or
either teaching asset.** **MOVEMENT: NONE.** Every design-brief seam citation
in the AWL family stands unchanged at this base. Reported as a fact, measured
on this run, not inherited from the plan.

---

## §2 — PLACEMENT DECISION AS TAKEN (D2)

**Decision: a module family inside `crates/aion-awl`, at
`crates/aion-awl/src/command/`.** The brief's assumption is UPHELD; the
sibling-crate fallback `crates/aion-awl-command` was not needed and was not
created.

**Reasoning, as taken:**

1. **The surface is a language surface.** It needs the crate's lexer
   (`src/lexer/tokens.rs`), its token stream (`src/parser/stream.rs`), its
   spans (`src/lexer/tokens.rs:3`), its type reader (`src/parser/types.rs`)
   and its canonical printer's own escapers (`src/printer/exprs.rs`). A
   sibling crate would either fork all five — five second accounts of the
   grammar — or take a circular dependency.
2. **R7's teaching machinery is inside this crate and cannot be reached from
   outside it.** `src/guide/lookup.rs:96` resolves positional words against
   `crate::lexer::POSITIONAL_WORDS`, and `guide::all()` is pinned by equality
   against that inventory. A sibling crate cannot add a word to it.
3. **The 500-line law is per file and is satisfied with room.** The family is
   19 files; the largest is `clauses.rs` at **472** code lines (excluding
   comments and blanks), the next `check.rs` at 382. `mod.rs` carries `mod`
   declarations and re-exports and nothing else.

**What the placement cost, reported.** Four items in `crates/aion-awl` were
promoted from `pub(super)` to `pub(crate)` so the sibling module could reach
them rather than duplicate them: `parser::stream::{Stream, describe}`,
`parser::types::{parse_type_ref, type_ref_span}`, `parser::error::ParseError::new`,
and `printer::exprs::{duration_text, string_literal, type_ref_text}`. Each is
a widening within the crate only — nothing became `pub` — and each is
commented at its re-export with why duplication was the worse answer. This is
the one visible cost of the module choice and it is on the record for the
item-5 pricing decision, which remains the owner's at landing.

---

## §3 — WHAT LANDED, REQUIREMENT BY REQUIREMENT

### R1 — structural argv, with the DECLARED admits-`--` property ✅

`crates/aion-awl/src/command/decl.rs` — `CommandDecl` is a typed list:
`program` (literal invocation words, never fills), `options` (named by their
exact flag, optionally typed+filled), `operands` (positional, named, typed,
filled). No field is a string a shell re-parses; the emitter's product is a
`Vec<String>` of whole elements.

The end-of-options property is **declared, author-stated, per invocation**,
spelled `admits "--"` / `refuses "--"`, and it is **mandatory**: absence is
`end_of_options_unstated`, restatement is `end_of_options_restated`. No
command sits in an unstated middle.

*Byte-verified acceptance* (`tests/command_emit.rs`):

* `an_admitting_invocation_carries_the_marker_before_its_operands` — the
  canonical wall-flight2 cure emits exactly
  `["git", "tag", "--", "-v2.0", "HEAD"]`;
* `the_marker_is_placed_whatever_the_operands_turn_out_to_be` — the marker is
  placed for an ordinary operand too. **Unconditional by construction**: there
  is no path in `emit()` that reaches an admitting invocation's operands
  without pushing the marker first;
* `a_refusing_invocation_never_carries_the_marker` — the diff census emits
  `["git","diff","--no-ext-diff","--name-only","<pin>..HEAD"]` and asserts the
  absence of `--`;
* `a_refusing_invocation_refuses_a_leading_dash_operand` — emit-layer refusal
  `leading_dash_operand`, observed bytes `-x..HEAD`;
* the mandatory specimen: `leading_dash_specimen_missing` is a **check** error
  with a span, and `command_specimen.rs` FIRES the specimen through the real
  emitter, so what protects a refusing invocation is executed, not asserted in
  prose.

**FINDING F-UNIT-1 — the declared unit is the INVOCATION, not the bare
program. RESOLVED as the plan predicted, and it is a disagreement with nothing
— it is a fact the sources' own canonical fixtures force.** `git tag --
<name> <sha>` is R1's cure (admits), while the four pinned specimens' census
leg is `git diff --name-only "${BASE}"..HEAD`
(`.fleet/fixtures/gate-scripts/seqlegs-impl.gate-entry.sh:15`), where a `--`
before the revision operand would reinterpret `BASE..HEAD` as a **pathspec**
and silently change what the census measures. One `git` cannot be both. The
surface therefore declares `program "git" "tag"` and `program "git" "diff"`
as distinct invocations, and the module doc
(`src/command/decl.rs:16–28`) carries the reasoning at the seam. Nothing in
the design brief's R1 says otherwise; had it, the source would govern.

### R2 — typed, check-time, closed interpolation ✅

`src/command/fill.rs`. `{{parameter}}` resolves only over declared, typed
parameters, at check/compile time, into whole-cloth argv.

* **Backticks REFUSED**, at the PARSE layer, with their own cause
  `backtick_substitution` and the observed bytes. A `Fill` carrying a backtick
  is not a value that exists.
* `literal_where_derived` (`src/command/derived.rs`) — a hand literal in a
  fill for which a derivation exists is refused **carrying the literal as
  observed bytes**. The shape set is CLOSED and small, and stated: a 40-hex
  git object name, a 64-hex content digest, and the fleet control-token shape.
  `derived_tests.rs` pins both directions — the three shapes are caught
  wherever they sit in a literal, and sixteen ordinary strings
  (`HEAD`, `main`, `--name-only`, `deadbeef`, a 39-hex run, an uppercase-hex
  run, a near-miss control token…) are left alone, because a rule that guessed
  would teach authors to route around it.
* File contents enter as addresses, never spliced strings (stated at
  `fill.rs:26–30`; there is no production that reads a file).
* **Q5's fence is restated in the module doc so nobody widens it later**
  (`fill.rs:10–19`): no conditionals, no loops, no filters, no calls, no
  nesting. `an_interpolation_that_is_not_a_bare_parameter_name_is_refused`
  pins ten refused forms including `{{base | upper}}`, `{{env('HOME')}}` and
  `{{ if x }}`.
* **Determinism asserted, not assumed**:
  `the_same_declaration_and_parameters_emit_byte_identical_argv` resolves the
  same declaration sixteen times and compares.
* An undeclared parameter reference is a check error with a span
  (`parameter_undeclared`).

### R3 — env, cwd, hardening, timeout as declarations ✅

* **Env is structural.** `env NAME = "value"`. The `K=V` string-prefix form is
  **unparseable** — `env "PAGER=cat"` is refused by the GRAMMAR
  (`env_string_prefix`, parse layer), not by a lint.
  `env_cwd_and_hardening_resolve_structurally` additionally asserts that no
  emitted argv element ever contains `=` from an env binding.
* **Hardening has one home.** `hardening` with `path` and `open_files`.
  Declared twice in one scope is `hardening_restated` (check layer, naming the
  line of the first). An empty block is `hardening_empty` — an empty block and
  an absent one are different claims and must not read alike.
  The specimens show why hardening is a per-flight parameter and not a
  constant: only `seqlegs-impl.gate-entry.sh:3–4` hardens
  (`export PATH=$HOME/.cargo/bin:$PATH`, `ulimit -n 65536`); the other three
  harden not at all.
* **Cwd is declared; `{workspace_root}` is the one admitted placeholder**,
  consumed from the existing rule (`crate::workspace_root::WORKSPACE_ROOT_PLACEHOLDER`)
  and NOT extended. Any other brace placeholder is `cwd_placeholder_unknown`
  (specimen: `cwd "{repo_root}/src"`, observed bytes `{repo_root}`). **No new
  venue-binding holes**: the resolved `Argv.cwd` carries the placeholder
  UNEXPANDED, because expanding it is the executing side's act against its own
  workspace.
* **TIMEOUT ABSENCE IS LEGAL AND NO DEFAULT WAS INVENTED.**
  `a_declaration_with_no_timeout_emits_no_timeout` asserts `timeout_ms == None`,
  `timeout_owner == None`, `open_files == None`, `hardened_path == None`,
  `cwd == None` and an empty env for a declaration that states none. The field
  is `Option<i64>` and nothing anywhere substitutes a number. A declared
  timeout names an `owner`; an unowned one is `timeout_owner_empty`, because a
  ceiling nobody owns is the shape an invented default takes.

### R4 — the refusal vocabulary, layer-named, closed ✅

`src/command/cause.rs` + `src/command/error.rs`.

* **35 cause codes**, a closed enum with a stable `as_str()` wire rendering
  and a `CommandCause::ALL` inventory pinned against the layer partition.
* **Four layers, four distinct error types** — `CommandParseError`,
  `CommandCheckError`, `CommandEmitError`, `CommandSpecimenError`. Never one
  catch-all. `CommandCause::layer()` is an exhaustive match with no wildcard,
  so a cause added without a layer fails to compile.
* **Every refusal carries `observed_bytes`**, a span and its cause;
  `rendered()` puts all three in the diagnostic an author reads.
* **Constructor discipline, tested**: `every_cause_constructs_at_its_own_layer_and_nowhere_else`
  builds all 35 at their own layer and asserts a **typed error** for all 105
  wrong-layer attempts. `a_constructor_fed_an_empty_diagnostic_returns_a_typed_error`
  refuses an empty diagnostic — a refusal that says nothing is a catch-all
  wearing a specific cause's name. `the_vocabulary_carries_no_catch_all_code`
  pins the ABSENCE of `other`/`unknown`/`error`/`failed`/`invalid`/`bad`/`misc`.
* **No `unwrap`/`expect`/`panic` in library code.** The internal door
  (`CommandRefusal::refuse`) derives the LAYER FROM THE CAUSE, so misfiling is
  not caught — it is unwritable. `the_internal_door_files_every_cause_at_its_own_layer`
  proves it for all 35.
* **RED FIRST, mechanised.** `tests/command_refusals.rs::every_cause_has_a_refusing_specimen`
  iterates the ENUM and fails the build if any code lacks a specimen. 30
  causes have document specimens; each is asserted to refuse with its own
  cause AND its own observed bytes, and
  `every_specimen_refuses_at_the_layer_its_cause_names` proves the parse/check
  split is real.

**FINDING F-CAUSE-1 — `specimen_wrong_cause` is SHADOWED BY CONSTRUCTION,
recorded honestly.** Its red arm is unreachable in this cut, and the reason is
structural: `fire_specimen` binds every declared parameter with a value shaped
to its declared type, so `argument_missing`, `argument_undeclared` and
`argument_type_mismatch` cannot arise during a firing, and
`leading_dash_operand` is precisely the cause the specimen stands for. The
code is KEPT rather than removed, because collapsing "did not refuse" into
"refused wrongly" is exactly the catch-all the layer-naming law forbids, and
the emit vocabulary is expected to grow. What is asserted is what can be: the
code is constructible, renders its own wire string, and is filed at the
specimen layer (`command_specimen.rs:the_wrong_cause_code_is_filed_at_the_specimen_layer_though_its_red_arm_is_shadowed`).
This is the same honesty the rig's own probe battery applies to its two
unreachable arms (`toolchain_mismatch`, `suite_empty`). Reported, not hidden.

### R5 — own parser over the absorbed Just subset, CC0 suite as conformance fixtures ✅

`src/command/just_subset.rs` names the absorbed subset (4 productions) and the
refused list (6 forms) as tables the fixture harness reads, so a row added
without a fixture fails.

**The CC0 suite was taken AT THE BYTES, not from memory.** `casey/just` was
cloned at commit `b20386abdbae867a49cdff6c3c0f2b547faa9b23`; `Cargo.toml:12`
declares `license = "CC0-1.0"` and `LICENSE` is the CC0 1.0 Universal text.
Every derived fixture cites the repo, the commit, the file, and that file's
SHA-256:

| suite file | sha256 |
|---|---|
| `tests/parameters.rs` | `44014fc86b258c4a2573bd5f96245106ed4eabe84c61c559c8d63816219f3a56` |
| `tests/interpolation.rs` | `361740d3b249bbb8bfb67361402b320d1282cf37c90e1a77998e0bbdc88fa753` |
| `tests/backticks.rs` | `ff2f3e01abf698998dc6541608983fce70d29e92ec98e8b935df66248d095ac6` |
| `tests/dependencies.rs` | `538ea0952b3d76f57966c0b72e8477ca1786d1911d265cb490f72a41173cadfa` |
| `tests/export.rs` | `cbaf93ebf7b825941f8db3e0da490db1b57ede9d96771ac6bfb47ef3add7aa49` |

11 fixtures under `crates/aion-awl/tests/fixtures/command/just-subset/`, each
quoting the `JUST-SPECIMEN` it was derived from and, where AWL differs, saying
what differs and why. **`every_fixture_records_its_provenance`** refuses a
fixture that cites a suite file with no sha256 — the gate that keeps this from
degrading into remembered bytes. **No authored-for-absence fixture was needed:
the suite carried a specimen for every absorbed production and every refused
form.**

Refused suite cases are carried as REFUSAL fixtures, not skipped. Two are
worth naming: `parameter_default_values_may_not_use_later_parameters` becomes
a `parameter_undeclared` refusal fixture — Just refuses it too, so the two
agree and a drift becomes red; and
`closing_curly_brace_can_abut_interpolation_close` becomes the
`expression_interpolation` refusal, which is the fixture that keeps Q5's fence
from being widened one convenience at a time.

Every refused form named in the design's refused list has its own refusal
test, BY NAME, with its reason in the diagnostic
(`grammar_parser_gate.rs::every_assembly_form_is_refused_by_name_with_its_reason`,
six forms).

### R6 — the diff-census family is EXPRESSIBLE, proven at the bytes ✅ (with one finding)

⛔ **EXPRESSIBILITY PROOF, NOT DEPLOYMENT.** No `.fleet/` script retired, no
rig converted, no deployed document moved. `workflows/fleet-dev/fleet_dev.awl`
is byte-identical (asserted by the R8 corpus manifest and confirmed by an
empty `git diff` on the path).

**ONE declared shape** —
`crates/aion-awl/tests/fixtures/command/gate-census/shape.awl` — plus **FOUR
parameter sets**. All four specimens' digests were re-verified against the RIG
table and match:

| specimen | digest (re-hashed this run) | matches RIG table |
|---|---|---|
| `haem-wall.gate-entry.sh` | `1e1e3ad2d189b5b2c8fb07b6e302e5cc3505ac7fa7f84ac5561020e30ff10a1e` | ✅ |
| `haem-wall-r2.gate-entry.sh` | `ae66bb0b6819f36f46515017640b8bd6eea3e5009add725a3d70ab4209256832` | ✅ |
| `seqlegs-impl.gate-entry.sh` | `ef5b82a83eaead5a2ac42e6a026df99bb6bc0acdb9f2917ab4c787283c6f1804` | ✅ |
| `speech-1.KIT-gate-entry.sh` | `e1544910aee3378d3fbab17856235f8bb1bd7a7b6ca0dec54b497a2babaa7a5a` | ✅ |

`tests/command_gate_census.rs` does three things, in order: it **re-hashes**
each carried specimen against the digest its fixture cites (so a specimen
edited after the fixture was written fails rather than passing against a stale
copy); it **re-reads the cited line** of the cited specimen on this run; and
it byte-compares the shape's emitted argv against that line, normalized only
by the two normalizations each fixture declares in its own header. The
normalizer carries its own control
(`the_normalizer_rejects_a_command_line_that_is_not_the_census`).

All four rigs run the byte-identical census command
`git diff --name-only "${BASE}"..HEAD`, and the shape emits it for each pin.
The two declared normalizations, both about the shell rather than the command:
(1) the specimen line is a `changed=$(…)` assignment, so the compared text is
the substitution's body; (2) `"${BASE}"` is one shell word whose quotes exist
only to survive word splitting, which this surface does not perform.

**FINDING F-R6-1 — env hardening is expressible as a VALUE but not as a
PRESENCE, so one shape cannot cover all four rigs' hardening dimension.**
Reported, not papered over. At the bytes: `seqlegs-impl.gate-entry.sh:3–4`
carries `export PATH=$HOME/.cargo/bin:$PATH` and `ulimit -n 65536`; the other
three specimens carry neither. A command declaration either states a
`hardening` block or does not, and presence is not a value — an empty declared
PATH is not an inherited PATH, and inventing "empty means inherit" would be
exactly the assumed default the estate's law forbids. The shape therefore
covers the census ARGV for all four (which is what R6 asks the fixtures to
compare) and the hardening dimension for one. Closing it needs either a
declared-optional-presence form or two shapes, and that is a design question,
not an implementation one. Recorded in the fixture header itself
(`seqlegs-impl.params`, `INEXPRESSIBLE:`) so it cannot be read past.

**Two RESULT §A1 dimensions are out of the census command's argv BY
CONSTRUCTION, stated so they read as considered rather than missed.** The
deliverable set and the allowlist are applied by the gate leg's own
grep/wall step to the census OUTPUT — they never reach `git diff`'s argv — and
battery-or-not is a different leg entirely (clippy/nextest for seqlegs, pytest
for speech-1, none for the two haematite rounds), which is a different command
shape and not the diff-census family.

**Noted, not a finding:** `speech-1`'s BASE is the sentinel `__CUT_COMMIT__`,
text-substituted into the script before dispatch. The shape expresses it
faithfully because a base pin arrives as a declared PARAMETER — which is the
cure for the sentinel rather than an accommodation of it.

### R7 — structural teaching rows for every new word ✅

**Thirteen new words**, every one POSITIONAL rather than reserved. That is a
measurement, not a preference: `command` alone stands in **54** `.awl`
documents of this tree today as a harness-section field name, and `cwd` (38),
`path` (52), `refuses` (23), `program` (5), `env` (4), `operand` (3) and
`option` (1) stand in others. Reserving any would break documents that exist —
the lexer's own stated doctrine for `run`, `send`, `to`, `query`, `answer`
and `in`.

Each of the thirteen (`command`, `program`, `admits`, `refuses`, `specimen`,
`option`, `operand`, `env`, `cwd`, `hardening`, `path`, `open_files`, `owner`)
lands with **all four rows**:

1. **Guide sentence + checked examples** — `src/guide/commands.rs`, examples in
   `src/guide/examples/decl_commands.rs`. Both example documents run through
   the crate's REAL parse+check pipeline and are pinned CANONICAL against the
   printer by the pre-existing `every_entry_example_passes_the_real_checker`.
2. **Grammar-table position** — a new `GrammarPosition::CommandBody` (16th) and
   a new `GrammarForm::CommandClause` (8th), with `form_of` filing `command`
   as a Declaration and its clauses as command clauses.
3. **AWL-REFERENCE §17 "Command declarations"** — written into
   `examples/assistant/resources/AWL-REFERENCE.md` and copied to
   `crates/aion-awl/guide-embed/AWL-REFERENCE.md`. The pre-existing
   `every_cited_section_actually_discusses_the_word` verifies, whole-word, that
   §17's body uses every one of the thirteen. **This is the build-failure law,
   already mechanized — and it is what F-RIG-1 is about.**
4. **LSP hover + completion** — both derived from the one table, never a copy.

**The LSP hover exception, argued rather than carved.** `glossary.rs` answers
hover for reserved words only, because a positional word lexes as an
identifier and hover cannot know whether it carries its language meaning. For
the command cluster hover CAN know: each word is recognised only as the first
word of a line, and whether that line opens a top-level declaration or stands
inside a `command` block is readable from the text before the cursor — the
same reading completion already performs. So hover answers under exactly that
condition and stays silent otherwise. Two tests, both directions:
`every_command_word_hovers_to_the_guide_sentence_from_the_table` reads the
expected text FROM `aion_awl::guide` on each run (against the table, not a
copy), and `a_command_word_used_as_a_name_still_hovers_to_nothing` proves
`command` inside a `harness` section still claims nothing, with a positive
control beside it.

**Console asset regenerated** via `aion awl guide --json` and compared by the
pre-existing `crates/aion-cli/tests/console_guide_asset.rs`, which was READ,
not edited.

⛔ **Tree-sitter is NOT in this cut's acceptance** and was not touched.
`builtins::BUILTIN_TYPES` is unchanged, so `tests/editor_assets.rs:171` and
the Zed mirror pin are untouched; new words are positional words, never
builtin types. *Stated for the record:* the tree-sitter grammar therefore does
not colour the thirteen new words. That is the expected consequence of the
ruling, not an omission.

### R8 — byte-identity and estate discipline ✅

**The corpus is DISCOVERED, repo-wide by extension, at run time, and LISTED.**
`crates/aion-awl/tests/corpus_roundtrip.rs`. Its exclusion set is three
classes and no file list: dot-directories, `target`, `node_modules`. Nothing
else — fixture corpora, deliberately-invalid documents, dated evidence and
design records are all IN, because a document that is supposed to be refused
must go on being refused with the same message.

**The completeness claim, printed by the test on every run:**

```
corpus discovered by extension over <repo>: 443 documents across 5 roots
  — crates/ 357, docs/ 47, examples/ 16, gate-logs/ 4, workflows/ 19
431 baseline documents re-read identically; 443 discovered in total
documents added since the baseline: 12   (all of them this cut's own fixtures)
```

The baseline manifest
(`crates/aion-awl/tests/fixtures/corpus/manifest.txt`, 431 records) was
generated **BEFORE any grammar change**, at base grammar — the only order in
which a before-and-after claim means anything. It records, per document, the
SHA-256 of `print(parse(source))` on success (387 documents) or the exact
error line, column and message on failure (44 documents), so the pin covers
the PRINTER as well as the parser.

**Result: all 431 pre-existing documents read exactly as they did at the
baseline. Zero regressions.** The instrument also fails if a baseline ROOT
stops being reached, and carries its own control
(`the_comparison_rejects_an_outcome_the_tree_does_not_produce`).

**The brief's amendment-2 suspicion is confirmed at the bytes:** `examples/`
and `workflows/` together are **35 of 431 — 8%**, so a hand-enumerated claim
over those two roots would have missed **92%** of the corpus it claimed to
cover.

* `cargo clippy --workspace --all-targets -- -D warnings` — **clean**. Twelve
  findings were fixed by fixing the code; **no `#[allow]`, `#[expect]`,
  `#[deny]`, `#[ignore]`, `_var` rename or `#[cfg(any())]` was added
  anywhere.** The two `single_match_else` findings were fixed by extracting
  named functions (`check_sole_hole`/`check_mixed_fill`,
  `supplied_value`/`defaulted_value`), which improved the code.
* `cargo fmt --all` **APPLIED** before commit. ⛔ No `fmt --check` gate was
  added. (Recorded contrast: `seqlegs-impl.gate-entry.sh:29` still runs
  `cargo fmt --all --check` as a gate leg; it is carried as a specimen of the
  pre-ruling shape, never as a model.)
* **No file over 500 code lines.** Largest new file: `clauses.rs`, 472.
  Largest file this cut EDITED: `printer/document.rs` and `parser/document.rs`,
  both 482. No file was pushed over by this cut.
* `mod.rs` is `mod` declarations and re-exports only.
* `thiserror` for every new error type; no `unwrap`/`expect`/`panic` in
  library code.

---

## §4 — CO-MOVES: existing tests this cut had to move, and why each is a same-direction change

⛔ **No gate was edited to make it pass.** Four pre-existing tests moved, each
because it is an EQUALITY pin that a new word necessarily changes, and each
was designed to force exactly this. The assertions are unchanged in every
case; only the pinned value moved.

1. **`crates/aion-awl/tests/grammar_parser_gate.rs:71` `DECLARATION_REFUSAL`.**
   The parser's dispatch now admits `command` and its diagnostic names it. The
   gate matches that sentence as a prefix, and its own doc says: *"if the
   parser rewords one, every probe reads as 'admitted' and the equality
   assertions below go loudly red — the gate fails safe rather than quietly
   matching nothing."* It did exactly that, on the full-suite run, before this
   constant was moved. The equality assertions are untouched, and
   `the_refusal_diagnostics_name_what_the_parser_actually_admits` still proves
   the sentence and the dispatch name the same set.
2. **`src/guide/grammar_tests.rs`** — `POSITION_VARIANTS` 15→16,
   `FORM_VARIANTS` 7→8, two new ordinals, one new `CommandClause` arm. The
   test's own doc says a new variant *"fails to COMPILE here until it is given
   an ordinal"*. It did.
3. **`src/guide/grammar_tests.rs::positions_are_read_from_both_halves_of_the_table_in_order`**
   — `positions_of("timeout")` gained a fifth position, because `timeout` is
   REUSED by the command surface rather than respelled. The vector is pinned
   whole on purpose and now names the fifth.
4. **`src/lexer/tokens_tests.rs`** — `POSITIONAL_WORDS` 6→19 entries.

Two ADDITIONS strengthen existing gates rather than relaxing them:
`the_command_body_offer_is_openers_plus_witnessed_words` extends
`grammar_parser_gate` to cover the new position in BOTH directions (the same
class the workflow header gets), and
`every_assembly_form_is_refused_by_name_with_its_reason` proves Amendment 1
per named form. The gate's own COVERAGE paragraph was updated to say so.

**Two real defects the new gate caught, both fixed:**

* `after` is a RESERVED word, so it never reached the identifier arm where the
  assembly-word list is checked, and a dependency edge spelled `after` fell
  through to a generic `clause_unknown`. `parse_clause` now reads the word
  whether the lexer made it a keyword or an identifier, so every refused form
  is refused BY NAME. A specimen was added for the keyword spelling.
* `path`, `open_files` and `owner` were offered at `CommandBody` but open no
  body line (`path`/`open_files` stand inside the `hardening` block, `owner`
  mid-line on the `timeout` clause). That is precisely the false
  "stands in a command declaration" the gate exists to catch. They are now
  modelled as WITNESSED words, each carrying a checker-passing witness
  document, exactly as the workflow header's mid-line words are.

---

## §5 — WALLS, EACH CONFIRMED HELD

* ⛔ **Amendment 1, STRUCTURAL** — held. No recipe, dependency or assembly form
  is REPRESENTABLE in a gate-leg command declaration. Six named forms
  (`recipe`, `depends`, `needs`, `after`, `steps`, `then`), each refused by
  name with cause `recipe_form_refused`, each with its own test, and the
  review-property reason rides in the diagnostic text ("a reader of a gate leg
  sees the exact command that ran").
* ⛔ **Amendment 2, the ratchet family** — held. `liminal22` and `sleepreap`
  are untouched; no script retired, no check absorbed. Verified: zero matches
  for either name in the diff.
* ⛔ **No deployed artifact moves** — held. `fleet_dev.awl` byte-identical; no
  `.fleet/` script retired; the sealed scripts untouched. The only `.fleet/`
  change is F-RIG-1's one wall alternative plus this report and the fixtures
  the rig itself staged.
* ⛔ **No shims, no compat, no dual paths** — held. The surface landed whole.
  The pre-existing `ast/command.rs` string scanner was neither wrapped nor
  extended nor deprecated; it serves a different construct (`run "…"` bodies)
  and is untouched.
* **Wall projection (design R6) is OUT** — not implemented; ruled to the
  recipes cut.
* **Q5 consumed, not implemented** — no venue-binding holes beyond the
  existing `{workspace_root}`; the closed-delta-set fence is mechanical in
  `check_cwd`.
* **Security-adjacent surfaces** — not analyzed; pointers to Cally Ray stand.

---

## §6 — FINDINGS INDEX

| id | finding | disposition |
|---|---|---|
| **F-RIG-1** | The staged wall is internally inconsistent and makes the staged cut ungreenable for ANY new AWL word. | Amended by exactly one path, documented in the gate script, one line to revert. **Waffles' ruling at re-gate; Vesper's at landing.** |
| **F-UNIT-1** | The declared end-of-options unit is the INVOCATION (program + subcommand), not the bare program. | Resolved in the design as taken; forced by the sources' own canonical fixtures (`git tag` admits, `git diff` must not). No disagreement with any source. |
| **F-R6-1** | Env hardening is expressible as a VALUE but not as a PRESENCE, so one shape cannot cover the four rigs' hardening dimension. | Reported at the bytes, recorded in the fixture header. Needs a design answer (optional-presence form, or two shapes), not an implementation one. |
| **F-CAUSE-1** | `specimen_wrong_cause` has no reachable red arm in this cut's emit vocabulary. | Shadowed by construction, recorded honestly, code retained (collapsing it would be the catch-all the law forbids). Same shape as the rig's own two shadowed probe arms. |
| **F-CENSUS-1** | A second adjacent construct exists: `crates/aion-cli/tests/awl_corpus_gate.rs`. | Triaged in §0 — a different question over a different corpus (35-document floor vs 431). Not a substitute for R8's instrument; untouched. |

**No requirement was thinned.** R1–R8 all landed. Nothing was deferred, and
there is no partial requirement to report.

---

## §7 — EVIDENCE SUMMARY

* **63** new test annotations under `crates/aion-awl*`.
* **35** cause codes, every one with a refusing specimen or an honestly
  recorded shadowing.
* **431/431** pre-existing AWL documents re-read byte-identically.
* **4/4** gate-script specimens re-hashed against the RIG table and matched;
  four parameter sets byte-compared against their own cited lines.
* **11** Just-subset conformance fixtures, every one citing `casey/just` at
  `b20386abdbae867a49cdff6c3c0f2b547faa9b23` (CC0-1.0) with its source file's
  SHA-256.
* **13** new words, each with guide sentence, checked example, grammar
  position, AWL-REFERENCE §17 row, and LSP hover + completion.
* `cargo clippy --workspace --all-targets -- -D warnings` clean, with no
  suppression of any kind added.

— the build leg. To Waffles for the landing re-gate (**read F-RIG-1 first —
you will be running an amended script**), then Vesper for owner review and
landing, then Calliope for the conformance read.
