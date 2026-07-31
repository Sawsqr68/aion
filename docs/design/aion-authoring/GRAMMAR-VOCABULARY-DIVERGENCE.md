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

**Still owed, and not done here:** `editors/zed-awl/extension.toml` pins the
standalone mirror at rev `fa1afb22`. That mirror is a different repository, so
this change does not reach Zed until it is pushed there and the rev bumped.
Cross-repo changes land in their own repo, never vendored.
