# AWL for Zed

Zed language support for AWL, Aion's workflow language.

The extension provides:

- `.awl` file detection
- Tree-sitter highlighting, indentation, and outline queries
- the AWL language server via `aion awl lsp` over stdio

The language itself is documented in [`docs/AWL.md`](../../docs/AWL.md), with
the full reference at
[`examples/assistant/resources/AWL-REFERENCE.md`](../../examples/assistant/resources/AWL-REFERENCE.md).

## Vocabulary

Highlighting keywords come from the grammar's own list
(`tools/tree-sitter-awl/grammar.js`), which carries AWL's reserved inventory —
including `agent` and `advisory`, the action-declaration config markers. `true`
and `false` are highlighted as booleans rather than as keywords.

AWL's *positional* openers — `send`, `to`, `query`, `answer`, `run` — are
deliberately **not** tinted. They are not reserved words: the corpus carries
`action send(to: String, …)`, `step run`, and `action parse_output(…, query:
String)` today, so the parser recognises each only in the one grammar position
where nothing else may stand. Leaving a statement opener plain is a cosmetic
loss; tinting an author's own binding would be the editor lying about their
code.

The language server does not have that constraint, because a completion is a
suggestion rather than a claim about existing text: `aion awl lsp` completes
over the full reserved inventory *and* the positional openers, both read from
the compiler's own tables (`Keyword::ALL` and `POSITIONAL_WORDS` in
`crates/aion-awl/src/lexer/tokens.rs`), so a new keyword cannot go missing from
completion without failing a test.

## Prerequisite: install Aion

The extension does not bundle the Aion CLI. Install or build `aion`, then ensure the executable is available in the environment used to launch Zed:

```sh
aion --version
```

By default the extension finds `aion` through the worktree `PATH` and launches:

```sh
aion awl lsp
```

To use a specific executable, add an LSP binary override to Zed settings:

```json
{
  "lsp": {
    "awl": {
      "binary": {
        "path": "/absolute/path/to/aion",
        "arguments": ["awl", "lsp"]
      }
    }
  }
}
```

If `arguments` is omitted, the extension supplies `awl lsp`. An `env` object in the same `binary` setting is also passed to the server process.

## Install as a development extension

Rust must be installed through `rustup`; Zed uses that toolchain when compiling Rust-backed development extensions.

1. Open Zed.
2. Open the Extensions page.
3. Click **Install Dev Extension**, or run the `zed: install dev extension` action from the command palette.
4. Select this directory: `editors/zed-awl`.
5. Open an `.awl` file and confirm the language selector shows **AWL**.
6. Use `zed: open log` if installation or language-server startup fails. For extension stdout/stderr, quit Zed and launch it from a terminal with `zed --foreground`.

A full interactive installation test is intentionally left to an operator with Zed available.

## Grammar source

Zed requires grammar sources to come from a Git repository and an immutable revision. Current Zed manifests support a `path` inside that repository, so `extension.toml` references this repository at a pinned commit and sets:

```toml
path = "tools/tree-sitter-awl"
```

No grammar vendoring is required. When changing the grammar, update the pinned `rev` in `extension.toml` to a commit that contains the new generated parser.

The upstream `tools/tree-sitter-awl/queries/folds.scm` file is not copied because Zed's current language-extension query set does not define a `folds.scm` contract. Its highlighting captures were translated to Zed theme captures (notably `@comment.doc`), and its indentation query was translated from `@indent.begin`/`@indent.end` to Zed's `@indent`/`@end` captures.

## Development and verification

```sh
cd editors/zed-awl
cargo check --target wasm32-wasip1
```

The manifest and language configuration are TOML. The language queries can be checked against the repository grammar with Tree-sitter's query command; see the repository verification workflow or run the equivalent `tree-sitter query` commands from `tools/tree-sitter-awl`.

## Primary references

This extension follows the current Zed documentation and production examples reviewed during implementation:

- <https://zed.dev/docs/extensions/developing-extensions>
- <https://zed.dev/docs/extensions/languages>
- <https://github.com/zed-industries/zed/tree/main/crates/extension_api>
- <https://github.com/zed-industries/extensions>
- <https://github.com/gleam-lang/zed-gleam>
