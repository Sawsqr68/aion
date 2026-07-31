# 🔴 RESUME HERE — the rename/references lane is UNCOMMITTED

Written 2026-07-31 ~07:55Z, immediately before a context compaction.

## Where main is

`origin/main` = **`1d357beb`**, pushed and clean. Everything below is work
sitting in the tree ON TOP of it and **not yet committed**.

Landed and pushed earlier in the session: #74 (`4462765a`), #31
(`d489e5e8`), resume note (`1d357beb`), plus #96 corrected by measurement (task
board only — the AWL query surface is complete; only the live operator-facing
demonstration is owed, and that is behind the freeze with #97).

## What is in the working tree, uncommitted

A `textDocument/references` + `textDocument/rename` lane for the AWL language
server — the natural companion to the completion provider that landed in
`d489e5e8`.

| File | State |
| --- | --- |
| `crates/aion-awl-lsp/src/rename.rs` | NEW, complete |
| `crates/aion-awl-lsp/src/rename_tests.rs` | NEW, 10 tests |
| `crates/aion-awl-lsp/src/lib.rs` | `mod rename;` added |
| `crates/aion-awl-lsp/src/server.rs` | capabilities + 3 request arms + `handle_rename` |
| `crates/aion-awl-lsp/tests/protocol.rs` | wire proof appended, NOT yet verified |

**Last known-green state: `cargo test -p aion-awl-lsp --lib` → 24 passed, 0
failed.** That was BEFORE the protocol-level wire proof was appended.

## 🔴 The four things that must be finished

`cargo clippy -p aion-awl-lsp --all-targets -- -D warnings` currently reports
four errors, and the last `cargo test -p aion-awl-lsp` run printed no
`test result` line at all, which means the test target is not compiling —
almost certainly the appended wire proof. Fix in this order:

1. `crates/aion-awl-lsp/tests/protocol.rs` — find out why the test target does
   not build. Run `cargo test -p aion-awl-lsp --test protocol 2>&1 | head -40`
   and read the actual error rather than guessing. The appended test is
   `live_references_and_rename_including_a_refusal`.
2. `server.rs:197` `handle_request` is now 126 lines (limit 100). Split it —
   the natural cut is a `handle_semantic_request` helper holding the
   definition / hover / references / prepareRename arms, matching the existing
   `handle_rename` extraction.
3. `server.rs:366` "mutable key type" — the `HashMap<Uri, Vec<TextEdit>>` in
   `handle_rename`'s `WorkspaceEdit`. `lsp_types::Uri` has interior mutability
   as far as clippy can see. Check whether `WorkspaceEdit::changes` can take a
   different map, or restructure to `document_changes` with `TextDocumentEdit`,
   which avoids keying by `Uri` entirely and is the richer LSP form anyway.
4. `rename_tests.rs:25` and `:36` — `map(...).unwrap_or_else(...)` on an
   `Option`; use `map_or_else`.

Then: `cargo fmt --all`, re-run `cargo test -p aion-awl-lsp` (expect 24 lib +
2 corpus + 4 protocol), clippy clean, and commit.

## The mutation check that is still OWED

The load-bearing claim of this lane is **"a rename is not a text search."** The
fixture deliberately gives the name `incident_id` to three different
declarations — a workflow input, an action parameter, and a type field — and
`a_different_declaration_sharing_the_name_is_untouched` asserts only the input
moves.

Prove it discriminates: change `target_at` to collect spans by matching
`declaration.name` instead of `declaration.span`, and confirm that test fails
while the others stay green. **Do not skip this** — it is the whole reason the
feature is built on the checker's recorded resolutions rather than on string
matching, and an unverified claim of that kind is exactly what this seat has
been correcting all day.

## Design notes worth keeping

- **The edit set comes from the checker, not from text.** `SemanticAnalysis`
  records, for every span it resolved, which declaration it refers to.
  `target_at` collects every span whose `declaration.span` equals the target's,
  plus the declaration's own name span. A text match would rewrite string
  literals, comments and unrelated bindings.
- **Refusal is part of the feature.** A rename that produces a document meaning
  something else is worse than no rename, because the author sees success and
  moves on. Three refusals: not-a-declaration, wrong lexical class, collision.
- **The case rule is derived, not written down.** `same_lexical_class` LEXES
  the proposed name and requires one token of the same kind as the current
  name. That is why it rejects a reserved keyword, the wrong case class,
  punctuation, an empty name and a multi-word name in a single check — and why
  it cannot drift from the language. Same instinct as `Keyword::ALL`'s
  source-derived count in `d489e5e8`.
- **The collision check is deliberately scope-blind** and errs towards
  refusing. AWL scoping is real (two steps may each bind `result`), so it will
  occasionally refuse a safe rename. That is the correct direction: a refusal
  costs one rejected prompt, a wrong accept silently merges two things.
- **A refused rename must be a protocol ERROR**, never an empty edit list. An
  editor handed no edits shows the author nothing, and they conclude the server
  is broken. The wire proof asserts the error carries the reason.

## Standing constraints — unchanged

**The queue is still held and each item needs an explicit NAMED GO from Cally:**

1. `cargo clippy -p aion-rs -p aion-server --all-targets -- -D warnings`
   (narrowed — `-p aion-package` discharged at `4462765a`). **Predicted RED**,
   and that prediction is on the record with Cally: three separate commits have
   now landed on main carrying clippy errors (`unnecessary_wraps` from
   `4d9ca3b0`, `similar_names` from `73c6ed50`, `unnecessary_hashes` in a test),
   and every crate touched has surfaced one. The lint gate has not been running.
2. `cargo test -p aion-rs` — the #106 battery. Until it runs, `4d9ca3b0` is
   unverified at battery level and must be described that way.
3. `cargo build -p aion-cli` + the #97 live demonstration against a real
   server, quoting recorded bytes. Not the `target/debug/aion` shortcut.

Also: **do not restart Tom's server** (#108 — it runs from a deleted binary).
#83 stays designed-and-held; it lands as ONE motion after the two owed gates,
on its own battery.

Athena measured that #75 (`e866f723`) is NOT implicated in the `.tmp*` trees
she saw under system temp during my aion-package lanes: those are ordinary
rustc/cargo `TempDir`-on-drop behaviour that reclaimed themselves within ~60
seconds. Presence during a build is not persistence. Nothing owed there.

One law amendment, from Athena via Cally: the clause "`TZ=UTC stat` does NOT
convert" is **RETIRED** — it does convert correctly. The durable forms are an
offset that travels (`-t '…%z'`) or a conversion the tool performs; never a
hand-written `Z` suffix.
