# The editor grammar and the language disagree about AWL's vocabulary

Measured and fixed 2026-07-31 by Vesper, from `origin/main` `6cc3ca1d`.

## What this is

AWL's vocabulary is defined twice, in two languages, with nothing comparing
them:

| Definition | Location | Authority |
| --- | --- | --- |
| `Keyword::ALL` + `as_word` | `crates/aion-awl/src/lexer/tokens.rs` | **authoritative** — this is the lexer the compiler runs |
| `const keywords = [...]` | `tools/tree-sitter-awl/grammar.js` | drives every editor's highlighting |

The tree-sitter grammar is not a niche extra. It is compiled to wasm and
vendored into the ops console at
`apps/aion-ops-console/src/features/authoring/assets/awl.wasm`, where
`highlight.worker.ts` runs it under `web-tree-sitter`. **It is the grammar
behind the operator's own authoring surface**, as well as `editors/zed-awl`
(pinned at mirror rev `fa1afb22`) and `editors/nvim-awl`.

This is the multi-enforcer divergence family: one invariant, several
independent enforcers, no shared source of truth. Every copy looks right in
isolation, so nothing ever fails. The grammar was last touched at `20f4309b`,
**436 commits** before this measurement.

## The defect

Two words the language reserves and the grammar did not know:

- `agent` — landed at `8451e0e5` (task #98). The checker refuses the wrong
  shape and the worker reads the marker off the document. It has force.
- `advisory` — `Keyword::Advisory`, `tokens.rs:149`, `as_word` arm at `:316`.

In the ops console, in Zed and in Neovim, both rendered as ordinary
identifiers. A word the compiler enforces looked to the author like a name they
had made up.

Fixed by adding them to `const keywords`, regenerating, rebuilding the wasm
with `tree-sitter build --wasm` (CLI 0.26.10, the pinned version) and
re-vendoring it. Corpus fixture `Action markers: agent and advisory` pins both,
and mutation-checks cleanly: removing the two words fails that fixture and no
other.

## What this grammar actually is, and two readings I got wrong

I reached the right answer twice by being corrected by evidence rather than by
reasoning, and both wrong turns are worth keeping because each is a tempting
mistake.

**Wrong reading 1: "`in` is reserved by the grammar but not by the language, so
it is drift — remove it."** `Keyword::from_word("in")` is indeed `None`; `in`
is a POSITIONAL WORD. Removing it turned two passing corpus fixtures red, and
the reason is in the fixture: `fork item in items sequential`. There, `in` **is
the structure**.

The thing I had not established was what kind of grammar this is. It is a
**highlighting grammar, not a parsing grammar**: `fork_statement` matches the
bare word `fork` and lets the rest of the line fall into a generic token
stream, where `$.keyword` is the only thing that gives a word colour. In that
design the global list is not a claim about what the language reserves. It is
the colouring mechanism, and `in` belongs in it.

**Wrong reading 2: "then the grammar is missing the OTHER five positional
words — `send`, `to`, `query`, `answer`, `run` — add them for consistency."**
Adding them turned three fixtures red. The clearest is `wait approval timeout
30s -> answer`, which binds a result called `answer`.

That is the whole argument, and it is asymmetric on purpose. This grammar
cannot tell a statement opener from a binding, so listing a word colours it
EVERYWHERE. `in` costs nothing because nobody names a binding `in`. The other
five are ordinary English words that authors do bind. **Leaving a statement
opener plain is a cosmetic loss; tinting an author's own binding is the editor
lying about their code.** The uneven treatment is the correct answer.

## Two false positives, discounted

A naive diff of the two lists also reports `true`, `false` and `awl`:

- `true` / `false` — the grammar gives booleans their own node
  (`boolean: _ => choice('true','false')`), captured as `@constant.builtin`.
  That is a refinement, better than lumping them in with keywords.
- `awl` — the grammar's own `name:` field, swept in by the text extraction.

Reporting those would have been three fabricated defects.

## The gate

`crates/aion-awl/src/lexer/grammar_vocabulary_tests.rs` reads `grammar.js`
through `include_str!` and pins the relationship in both directions:

1. `every_reserved_word_is_known_to_the_editor_grammar` — every `Keyword::ALL`
   word is coloured. Mutation-checked: deleting `agent`/`advisory` from the
   grammar reproduces the original defect exactly (`["advisory", "agent"]`) and
   fails only this test.
2. `the_grammar_colours_nothing_the_lexer_cannot_account_for` — the reverse
   direction, so the gate cannot be satisfied by a grammar listing every
   English word.
3. `the_specifically_typed_words_are_still_covered` — the `true`/`false`
   exemption carries the rule name that must keep covering it, so the exemption
   list cannot become a way to silence the gate.
4. `the_coloured_exception_is_a_genuine_positional_word` and
   `the_bindable_positional_words_are_deliberately_left_plain` — the `in`
   asymmetry is asserted, with its reason, rather than left as a comment
   somebody later "tidies".

Each carries a non-vacuity control.

## The second gate — the compiled artefact

The gate above reads `grammar.js`. It therefore says nothing about whether the
vendored wasm was ever rebuilt from it, which is the same silent-drift shape one
layer down: edit the grammar, skip `tree-sitter build --wasm`, and the Rust gate
stays green while the console keeps colouring the old vocabulary. Before this
the console had **zero** tests touching highlighting at all.

`apps/aion-ops-console/src/features/authoring/lib/highlight-assets.test.ts`
loads the two vendored artefacts — `assets/awl.wasm` and `assets/highlights.scm`,
the exact pair `highlight.worker.ts` imports — under `web-tree-sitter`, and
checks the COMPILED grammar against `const keywords` read from `grammar.js`.
Both halves are needed: the Rust one catches a missing word, this one catches a
missing rebuild.

It is driven by the grammar rather than by a hand-copied list, because a copied
list drifts exactly the way the wasm did. The mechanism is that a keyword alone
on a line lexes as `keyword`, so every declared word can be asked directly. It
carries the same shape of controls: non-vacuity on the extracted array, `worker`
as a positive control, `incident_id` as an inverted one, `agent`/`advisory` in
real action-marker position, and `answer` asserted to stay plain in
`wait approval timeout 30s -> answer`.

Mutation-checked decisively. Restoring the pre-fix wasm
(`git show 973b3d58~1:apps/.../awl.wasm`) fails exactly two tests and names
exactly the two words — `["agent", "advisory"]` — while both control tests stay
green, so the failure is about the artefact's vocabulary and not the harness
breaking wholesale.

It runs in the house battery as `console:test` (`scripts/battery.sh`).

## The third and fourth copies — the editors

Chasing the last rider (the Zed mirror) found the same family twice more.

**The mirror push was owed and the record of it was wrong.** A local clone said
the published mirror was twelve keywords and the whole flow vocabulary behind.
Fetching first showed that clone was stale: the mirror had been synced on
2026-07-28 (`635f8b7`, aion `28eb1eec`) and the real gap was exactly the two
words this document is about. The twelve-word finding was an artefact of not
fetching, and is recorded here because acting on it would have produced a
confident, wrong commit message. Measure the remote, not a copy of it.

Synced and pushed as mirror `8d4d63e`, verified present by an anonymous
`git ls-remote` with global and system git config nulled, then pinned in
`editors/zed-awl/extension.toml`. Two further things the mirror was publishing
were corrected there: its README was aion's README carried over verbatim, so it
opened "This directory is the AUTHORITATIVE home of the AWL grammar" — exactly
backwards, and the same divergence trap one level up; and it shipped
`test/corpus-sweep.sh` and `test/highlight-smoke.sh`, both of which resolve
their fixture root as `<grammar>/../..` and so have never been runnable from a
standalone checkout, while the README told readers to run them.

**Both editors' own query copies were stale on the flow vocabulary.**
`editors/zed-awl/languages/awl/` and `editors/nvim-awl/queries/awl/` each
vendor an adaptation of the authoritative queries, and each was missing
`raw_string`, `const_declaration`, `subflow_declaration`, `json_body` and
`schema_body`, and stopped the combinator list at `count` (no `any`, no `all`).
Zed's outline query also listed no `const` or `subflow`, so neither appeared in
its symbol outline.

This one hid behind a legitimate difference. The two copies deliberately use
different capture NAMES — Zed writes `@comment.doc` and `@number`, Neovim
writes `@comment.documentation` and `@number.float` — because each editor's
theme vocabulary differs. A diff is therefore expected to be noisy, and the
missing nodes sat inside that noise looking like more of the same.

## The editor gate

`crates/aion-awl/tests/editor_assets.rs`, four tests:

1. `every_editor_covers_every_node_the_authoritative_query_colours` — the
   invariant is NODE COVERAGE, not capture names. A node the authoritative
   query gives a colour to must be given some colour by every copy; the name is
   each editor's own business, and a copy may capture MORE (extra folds are a
   UX choice) but never less. Non-vacuity control on the extracted set.
2. `no_query_names_a_node_the_grammar_does_not_produce` — the reverse
   direction, across all nine query files, against `src/node-types.json`. A
   pattern naming a renamed node matches nothing and reports nothing.
3. `the_zed_pin_names_a_mirror_commit_carrying_this_grammar` — `extension.toml`
   carries a `# grammar-sha256` digest of the four grammar sources the mirror
   publishes. Change the grammar and this fires offline; the failure prints the
   four-step recipe and the new digest, and says in terms that updating the
   digest alone records a push that did not happen.
4. `the_recorded_pin_is_present_on_the_public_mirror` — the network half of the
   publish-before-pin ruling. Gated at runtime on `AION_VERIFY_MIRROR_PIN=1`
   (never `#[ignore]`) and it prints a SKIPPED line when it does not run.

All four mutation-checked, each naming exactly the right thing: removing
`subflow_declaration` from Zed's highlights reports `["subflow_declaration"]`;
typo'ing a node reports `["schemma_body"]`; adding a word to `grammar.js`
without touching the pin fires (3); pointing `rev` at an unpushed sha fires (4)
against the real remote; and the skip path was observed printing.

One trap found while writing it: `manifest_value` had to be scoped to the
`[grammars.awl]` table, because `repository` is ALSO a top-level key naming the
aion repository — a flat read would have sent the publish-before-pin check at
the wrong remote and passed.

## Rebuilding the grammar

`tree-sitter-cli` is not installable inside `tools/tree-sitter-awl` on this box
— the package's own `tree-sitter` native binding fails `node-gyp` under node
v26. Install the CLI alone into a scratch prefix instead:

```
npm install --prefix <scratch> tree-sitter-cli@0.26.10
```

then, in `tools/tree-sitter-awl`: `tree-sitter generate && tree-sitter test &&
tree-sitter build --wasm`, and copy `tree-sitter-awl.wasm` over
`apps/aion-ops-console/src/features/authoring/assets/awl.wasm`.

**Discharged 2026-07-31:** the mirror push and the rev bump — see "The third
and fourth copies" below. `editors/nvim-awl` tracks the mirror's default branch
rather than a revision, so it needed no pin.
