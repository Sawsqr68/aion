# BRIEF — aion: AWL command-builder, CUT 1 (gate-leg commands, diff-census family) — IMPLEMENTATION flight

Authored 2026-08-23 by Calliope Tim Tam, commissioned by Waffles
(08:29Z; his review, then the grill, then staging). **Sources, all
settled:** the design brief
`design/awl-command-builder-design-brief-20260823.md` at its
close-round state (§6 closed @ `0091531`, carrier law folded @
`c8f5e37`); the Q4 determination @ docs `6c7a45f` (ratified — §5 of the
design brief defers to it); the Q5 determination @ docs `4125e66`
(ratified). Where this brief and those sources disagree, THE SOURCES
GOVERN and the disagreement is a reportable finding.

**SCOPE (Q4 staging line 1, exactly):** gate-leg commands, the
DIFF-CENSUS family only — design R1–R3 — with both Q4 amendments
BINDING: (1) gate legs consume the command half ONLY; recipes,
dependency graphs, and multi-step assembly are not reachable from a
gate-leg declaration in any form — the grammar must make this
unrepresentable in a gate-leg context, not discouraged; (2) the ratchet
family is FULLY EXCLUDED — no ratchet script retired, none of its five
distinctive checks absorbed in this cut; they lift to requirements in a
later flight.

**TREE PIN:** the base pins relanded aion main — `5e2fb5a3e`, settled
and verified at Waffles' own ls-remote (08:34Z); staging names it.
Design-brief seam citations were derived at `c590712f5` (v0.20.0); the
flight re-derives every cited seam against its own HEAD and reports
movement (a brief's citations are claims about the commit they were
derived on).

**HAS THIS ALREADY HAPPENED? — first question, answered before any
work:** on the flight's own clone of current main, prove by search that
no part of this contract already exists: a command-declaration
construct in the AWL grammar (search the lexer/parser word tables), an
argv emitter, an `admits_dash`/end-of-options property in any spelling,
`{{`-interpolation handling outside string display, any Just-grammar
conformance fixtures. State the pathspec and the zero (or triage every
hit) in the report — an undeclared corpus is not a census.

## The one-sentence contract

AWL gains a typed command-declaration surface — structural argv, typed
check-time interpolation, declared env/cwd/timeout — whose emitter and
checker make the wall-flight2 defect family unwritable, proven
expressible over the diff-census gate-entry family as fixtures, with
every pre-existing AWL document byte-identical and every new word
carrying its structural teaching row.

## Placement assumption (named, revisable)

The surface is assumed to land as a module family inside
`crates/aion-awl` (beside lexer/parser/checker/printer per that crate's
charter) — **revisable, not decided**: module-vs-sibling-crate is item
5's pricing (design §3), and nothing below depends on the answer. If
the flight finds the module shape violates the crate's size/lint
discipline (no file over 500 lines), a sibling crate
`crates/aion-awl-command` is the fallback, reported as a finding either
way.

## Requirements — BINDING

- **R1 — structural argv (design R1, verbatim semantics).** A declared
  command is a typed list — program, options (named, typed), operands
  (positional, typed) — never a string a shell re-parses. The
  admits-`--` end-of-options property is a DECLARED, author-stated,
  per-program fact — never inferred. Two faces, two disciplines: for a
  program declared admitting, the emitter places `--` between options
  and operands unconditionally — the family closes by construction;
  for a program declared NOT admitting, a leading-dash refusal
  specimen is MANDATORY — `awl check` REFUSES a declaration naming a
  non-admitting program without its specimen, with a span. No command
  sits in an unstated middle.
  *Acceptance:* the emitter's output for an admitting program with a
  leading-dash operand is byte-verified to carry `--` (the
  wall-flight2 cure, `git tag -- <name> <sha>`, as the canonical
  fixture); a non-admitting declaration without its specimen is a
  check error with a span; option/operand type mismatches are check
  errors. Files: the parser/checker/emitter modules under the
  placement assumption; checker diagnostics beside the existing
  check-error family.
- **R2 — typed interpolation, check-time, closed (design R2 +
  close-round fill-side rule).** `{{parameter}}` resolves only over
  declared, typed parameters, at check/compile time, into whole-cloth
  argv — a construction surface, never runtime composition. Just's
  backtick command substitution is REFUSED (execution at evaluation
  time — the determinism boundary does not admit it); file contents
  enter as addresses, never spliced strings. The close-round refusal
  lands here: `literal_where_derived` — a hand literal in a fill
  position for which a declared derivation exists (a base SHA, a
  control token, any declared re-derive parameter) is a composer
  refusal carrying the literal as observed bytes. Q5's fence is this
  requirement's fence: interpolation closed over declared parameters
  is NOT a templating language, and nothing in this cut may widen it
  toward one.
  *Acceptance:* an undeclared parameter reference is a check error
  with a span; a backtick form is a refusal with its own cause; a
  `literal_where_derived` specimen refuses with the literal in
  `observed_bytes`; resolved argv is deterministic (same declaration +
  same parameters ⇒ byte-identical argv, asserted by test).
- **R3 — env, cwd, timeout as declarations (design R3, verbatim).**
  Env bindings are structural fields, never `K=V` string prefixes.
  PATH/ulimit hardening is stated once on the executing declaration
  scope and inherited. Cwd is declared; `{workspace_root}` is the one
  admitted placeholder, per its existing REFERENCE §16 semantics —
  Q5's hybrid, consumed not extended: this cut adds NO new
  venue-binding holes (venue binding is the deploy surface's, Q5 §2a,
  and lands with the ops-script retirement, LAST). Timeouts are
  declared values with named owners; ABSENCE IS A LEGAL DECLARED
  STATE — the builder invents no ceiling and no default (estate
  no-assumed-defaults law; an unset timeout defers to the engine's own
  behavior, never to a number this flight makes up).
  *Acceptance:* a string-prefix env form is unparseable (grammar, not
  lint); hardening declared twice in one scope is a check error
  (one-home discipline); a declaration with no timeout passes check
  and emits no timeout; `{workspace_root}` in cwd resolves per the
  existing rule and any OTHER brace-placeholder in cwd is a check
  error (the closed-delta-set fence, mechanical).
- **R4 — the refusal vocabulary, landed style, layer-named.** Every
  refusal this surface produces carries a stable closed cause code
  (new codes are a design change), `observed_bytes`, and the
  layer-naming discipline of the carrier-contract law (design §6
  close): a parse failure, a check refusal, an emit refusal, and a
  specimen-fire failure are DIFFERENT LAYERS with different causes —
  never one catch-all. Forbidden causes are unrepresentable at
  construction (the landed `StageFailed` posture: the constructor
  refuses an empty or catch-all cause, typed error, no panic).
  *Acceptance:* the cause-code set is a closed enum; every code has at
  least one refusing specimen (red-first); a constructor fed an
  illegal cause returns a typed error (no `unwrap`/`expect`/`panic` —
  workspace lints are the estate's, `-D warnings`, no `#[allow]`).
- **R5 — own parser over the absorbed Just subset, with the CC0 suite
  as conformance fixtures.** The absorbed grammar is exactly the
  design §3 list (recipes' authoring shapes as they appear in
  command/parameter declarations: parameters with defaults, variadic
  tails, `{{…}}` syntax) — nothing more; the refused list (§3) is
  enforced by refusal with a test per named form (absence-of-escape
  discipline). Just's CC0 test suite is carried as fixtures FOR
  EXACTLY THE ABSORBED SUBSET, so grammar drift against author
  expectation is a red test, not a surprise; suite cases exercising
  refused forms are carried as REFUSAL fixtures, not skipped.
  *Acceptance:* the fixture harness runs in `cargo test`; every
  absorbed grammar production has at least one fixture — Just-suite-
  derived where the CC0 suite carries a specimen, and where it does
  NOT, an authored fixture satisfies the row with its absence-from-
  suite named in the fixture header (Waffles' amendment 1:
  completeness is a property of the question — the question must not
  be unanswerable on a technicality); every refused form named in
  design §3 has a refusal test; the fixture provenance (which Just
  suite file, CC0 — or authored-with-reason) is recorded in the
  fixture header.
- **R6 — the diff-census family is EXPRESSIBLE, proven as fixtures.**
  The four primary rigs' diff-census gate-entry legs (the `gate_entry`
  family RESULT §A1 measured: one machine, per-flight values — base
  pin, deliverable set, allowlist, battery-or-not, env hardening) are
  expressed as ONE declared command shape plus FOUR parameter sets,
  landed as in-repo fixtures with the emitted argv asserted against
  the scripts' actual command lines (the conversion is
  parameterization, F1 — the fixture proves it at the bytes). ⛔ This
  is expressibility proof, NOT deployment: no `.fleet/` script is
  retired, no rig is converted, no deployed document moves — the
  replacement seam is the gates brief's, sequenced behind seqlegs
  relanding and its own staged act.
  *Acceptance:* four fixture parameter sets, one shape; per-rig
  emitted argv byte-compared against the corresponding script's
  command text (normalized only for the declared structural
  differences, each named in the fixture); any diff-census check the
  shape CANNOT express is a reportable finding, never papered over.
  *Provenance discipline (Waffles' amendment 3, repo-qualified at the
  grill):* staging carries the four gate scripts into the flight tree
  as pinned specimens, and each fixture names its source REPO, script
  PATH, and the commit DIGEST it was taken at — repo-qualified
  because gate-entry specimens live in the docs repo's dispatch-rigs
  AND in aion rig branches' `.fleet/` (grill fold, Vesper): cite
  whichever repo holds the bytes, so the comparison is against cited
  bytes, never remembered ones.
- **R7 — structural teaching rows for every new word.** Every language
  word this cut lands (the command-declaration keywords, the
  admits-`--` property, parameter kinds, the refusal cause names that
  surface to authors) lands WITH: its `aion awl guide` sentence and
  checked example (the guide's one-table spine — CLI, hover, console —
  extended, not forked), its AWL-REFERENCE section, and LSP surfacing
  (hover/completion in `crates/aion-awl-lsp`, derived from or
  conformance-checked against the one table). A missing row is a BUILD
  FAILURE, not a review note (the standing structural law; tree-sitter
  rides Tom's pending confirmation and is NOT in this cut's acceptance).
  *Acceptance:* the guide's example-gate runs the new examples; the
  reference section exists and is cited from the guide table; LSP
  hover returns the guide sentence for each new word (test against the
  table, not a copy).
- **R8 — byte-identity and estate discipline.** Every pre-existing AWL
  document in the repo parses to the same result before and after (no
  grammar regression). The corpus is DISCOVERED, repo-wide by
  extension (`**/*.awl`), never enumerated by hand (Waffles'
  amendment 2: the claim is repo-wide, so the instrument must be —
  if `examples/` and `workflows/` are genuinely the whole population,
  discovery proves it instead of asserting it); the discovered list
  IS the completeness claim. `cargo clippy --workspace
  --all-targets -- -D warnings` clean; `cargo fmt --all` APPLIED
  before commit — ⛔ NO fmt-check gate (grill fold, Vesper: Tom's
  estate gate ruling 2026-08-17 @ docs `2ab9689`, canonical in
  `GATE-EXTRACTION-STANDARD.md` — "no fmt gate"; format is autoformat
  discipline, never a gate leg); no
  file over 500 lines; `mod.rs` declarations-only; `thiserror` for
  the new error types; no `unwrap`/`expect` in library code.
  *Acceptance:* the round-trip corpus test is part of the suite,
  discovers its corpus at run time, and lists it (which documents,
  how many, from which roots);
  the lint gates are the flight's gate legs, run on the committed
  tree with the receipt naming the tree state measured (head +
  clean/dirty — the close-round receipt law).

## Walls

- ⛔ **Amendment 1, structural:** no recipe, dependency, or assembly
  form is reachable from a gate-leg command declaration — the grammar
  refuses it, a test proves the refusal. A gate leg's review property
  (a reader sees the exact command that ran) is the reason and rides
  in the diagnostic text.
- ⛔ **Amendment 2:** the ratchet family (`liminal22`, `sleepreap`) is
  untouched — not converted, not retired, no check absorbed. Its five
  checks lift to requirements in a later flight (the
  better-behaviour-is-a-requirement law).
- ⛔ **No deployed artifact moves:** `fleet_dev.awl` byte-identity
  stands; no `.fleet/` script retires; the sealed scripts wait on
  item-2 typed capture and are not this flight's to touch.
- ⛔ **No shims, no compat:** the surface lands whole or not at all;
  nothing dual-paths.
- **Wall projection (design R6) is OUT of this cut — RULED (Waffles,
  08:34Z), assigned to the recipes cut, staging line 2, where prompt
  assembly lands.** Grounds: in the diff-census family the allowlist
  is per-flight DATA (F1's list) and arrives as a parameter, so
  expressibility needs no projection; projection is deriving walls
  from lane declarations, which is assembly's act. Recorded as ruled,
  not proposed.
- **Q5 consumed, not implemented:** no venue-binding holes beyond the
  existing `{workspace_root}`; harness-profile blocks are the deploy
  lane's later work; the closed-delta-set fence is mechanical in R3.
- **Carrier laws apply to the eventual dispatch prompts** (staging's
  concern), not to this brief's content — noted so the omission reads
  as sequenced, not missed.
- Security-adjacent surfaces (token minting, allowlist containment):
  pointers to Cally Ray stand as the design brief carries them;
  nothing here analyzes them.

## Evidence duties

Red-first throughout: every R4 cause code, every refused grammar form,
and R1's leading-dash family land with refusing specimens BEFORE the
passing paths. R6's fixtures assert at the bytes with named
normalizations. The report carries: the has-this-already-happened
census (pathspec + zero or triage), the re-derived seam cites with
movement named, the placement decision as taken with its reasoning,
the corpus round-trip completeness claim, and — for any requirement
the flight cannot complete — the requirement named and the stop
reported, never a silent thinning (no-lazy-code law; a partial landing
is not a landing).

— Calliope; to Waffles for review, then the grill, then staging.

## OWNER GRILL — PASSED WITH FOLDS (Vesper, 2026-08-23 08:4xZ)

Grilled whole against the ratified sources (Q4 @ `6c7a45f`, Q5 @
`4125e66`, the close-round state, the design brief). Verdict: **the
brief STANDS and staging may proceed on it.** Two folds applied above
at the grill (both marked in place): R8's `cargo fmt --check` gate
replaced with autoformat discipline per Tom's no-fmt-gate ruling
(`2ab9689`, verified at the canonical standard's own bytes); R6's
fixture provenance repo-qualified. Two facts recorded, no text moved:
the R6 wall's "sequenced behind seqlegs relanding" precondition is MET
— seqfix relanded at aion `5e2fb5a3e`, this brief's own base; and the
item-5 wall was declared OPEN by the owner (08:41Z) after that
relanding — the placement assumption's report-a-finding shape stands
unchanged, and the pricing decision remains the owner's at landing,
not the flight's. One staging note that is NOT a brief change:
the flight runs on rocketfish, where the pin-ignored defect was live
until today — the gate-entry MUST carry the toolchain-pinning
standard's assertions (`toolchain_absent`/`toolchain_unpinned`/
`toolchain_mismatch`), and its verification leg must discriminate
pin-honored from default-coincides (1.97.1-via-pin vs
1.98.0-via-stable makes this discriminable on this venue today).

— Vesper, owner. My gate at landing; findings against the sources
remain reportable over this sign.
