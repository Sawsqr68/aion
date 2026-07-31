# AWL completion: the provider has to work on a document that does not parse

Measured and designed 2026-07-31 for task #31.

## The gap

`crates/aion-awl-lsp` advertises formatting, document symbols, definition and
hover. It advertises no `completionProvider` and contains no completion code —
`grep -rn "completion" crates/aion-awl-lsp/src` returns nothing. An AWL author
gets diagnostics and go-to-definition but has to remember the whole vocabulary.

## The constraint that decides the design

Every existing feature begins the same way:

```rust
let document = aion_awl::parse(source).ok()?;
```

For hover and go-to-definition that is fine — mid-edit there is nothing to hover.
**For completion it is fatal.** Completion fires precisely when the document is
half-written: the author has typed `wait a` and wants the signal names. At that
instant the document does not parse, so a provider written the same way returns
nothing in exactly the case it exists for.

That is the same shape as #74 — a feature that is correct in every visible
respect and does nothing in the situation it was built for. It is the first
thing this design refuses.

**So completion never depends on a successful parse.** Candidates are harvested
from the TOKEN stream (`aion_awl::lex`), which tolerates every half-written form
the parser rejects. If even lexing fails — an unterminated string is the common
case — the contextual keyword set is still returned, because a keyword list
needs no document at all.

## Context, then candidates

Context is decided from the text before the cursor, never from an AST:

| Position | Offered |
| --- | --- |
| column 0 | `workflow` `type` `worker` `child` `const` `subflow` `step` |
| inside the `workflow` header | `input` `signal` `outcome` `timeout` `query` |
| inside a `worker` block | `action` |
| inside a `step` body, at a statement start | declared actions and children, `spawn` `wait` `send` `sleep` `fork` `loop` `distribute` `sequence` `collect` `outcome` `route` `answer` `after` |
| after `route ` | declared outcomes and step names |
| after `after ` | step names |
| after `wait ` / `send ` | signal names |
| after `\|>` | `filter` `sort` `map` `count` `any` `all` `route` |
| after `:` in a declaration | `String` `Int` `Float` `Bool` and declared type names |
| after `when ` | bindings, `is` `not` `and` `or` `empty` `present` `absent` `otherwise` |

Candidates are harvested by walking the token stream for `<keyword> <ident>`
pairs — `worker`/`action`/`type`/`signal`/`child`/`step`/`const`/`input`/
`outcome`/`subflow` — plus `-> <ident>` for the bindings a step body produces.

## Two supply-line fixes in `aion-awl` first

Both are the same law: a vocabulary with several independent copies diverges,
and every copy looks right in isolation.

1. **`Keyword` cannot be enumerated.** There is `as_word` and `from_word` and no
   `ALL`. A hand-typed completion list would silently fall behind the language
   the day a keyword is added. `Keyword::ALL` is added, pinned by a count
   derived from `as_word`'s own source rather than written down, and the LSP
   asserts every keyword appears in at least one context — so a new keyword
   fails a test instead of quietly going missing from completion.

2. **The positional words have no shared name.** `send`, `to`, `query`,
   `answer`, `run` and `in` are read as ordinary identifiers in one grammar
   position each. That is DELIBERATE and well documented — the corpus already
   carries `action send(to: String, …)`, `step run` and `wait decision ->
   answer`, so reserving any of them would break real documents. Nothing about
   that changes.

   What is missing is a shared NAME. The parser compares against a bare literal
   at each point of use (`word == "send"` appears twice in one file), so any
   consumer outside the parser — the completion provider being the first — has
   to re-type the vocabulary and hope it matches. They become named constants
   the parser uses, so completion offers exactly the words the parser accepts
   and a change to one is a change to both.

## What the pins caught, on their first run

Both guards earned their keep immediately, which is the argument for building
them rather than trusting the curation:

- `every_keyword_is_offered_somewhere` failed on its very first run naming
  `when`. The word was in no context at all, so an author writing a guarded
  outcome would never have been offered it. Placed in the step body, where an
  `outcome … when …` line lives.
- `cargo clippy -p aion-awl -p aion-awl-lsp --all-targets -- -D warnings`
  surfaced a `similar_names` error in `schema/derive.rs` that `73c6ed50`
  (#106's first half) had landed unlinted, and an `unnecessary_hashes` error in
  `tests/declared_signal_contract.rs`. With the `unnecessary_wraps` that
  `4d9ca3b0` landed, that is **three separate commits on main carrying clippy
  errors** — the lint gate has genuinely not been running, and every crate
  touched so far has surfaced one. All three are fixed; the pattern is the
  finding.

## The verification that matters

The claim "completion works on a document that does not parse" is not
self-evident from reading the code, so it is mutation-checked: gating the
harvest on a successful `parse` — exactly the way hover and go-to-definition
are written — fails 6 of the 14 unit tests plus the wire proof, while the
keyword-only tests correctly stay green because a vocabulary needs no document.
That split is the evidence the tests discriminate rather than merely pass.

## What is deliberately NOT done

No `resolveProvider`, no snippets, no signature help, no import completion.
Those are separate features; this one is the vocabulary and the declared names,
which is what an author cannot get from anywhere else.
