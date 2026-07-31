# RESUME — the console wasm gate, designed and probed, not yet written

Written 2026-07-31 ~07:25Z immediately before a context compaction.
`origin/main` = **`c06dbc45`**, working tree CLEAN and fully pushed. Nothing is
uncommitted; nothing is at risk. This note exists so the next step does not have
to be re-derived.

## Landed this stretch (all pushed)

| Commit | What |
| --- | --- |
| `6cc3ca1d` | AWL LSP references + rename, checker-driven, refusal-carrying, mutation-checked (task #111 CLOSED) |
| `973b3d58` | The tree-sitter grammar was two reserved words behind the compiler — `agent`, `advisory` — plus the Rust gate that makes the next drift loud |
| `c06dbc45` | The embedded ops-console bundle was stale on main, independently of the above; refreshed so the grammar fix actually reaches the operator |

Design notes: `GRAMMAR-VOCABULARY-DIVERGENCE.md` (same directory). Task #112
carries the open riders.

## THE NEXT STEP — write and run the console wasm test

**Why it matters.** The Rust gate
(`crates/aion-awl/src/lexer/grammar_vocabulary_tests.rs`) reads `grammar.js`,
NOT the compiled artefact. So editing the grammar and forgetting
`tree-sitter build --wasm` + `cargo xtask build-ops-console` stays completely
silent — the same silent-drift shape the gate exists to kill, one layer down.
The console currently has **zero** tests referencing highlighting at all.

**It is now unblocked.** `cargo xtask build-ops-console` ran `bun install` in
the app dir, so `apps/aion-ops-console/node_modules/web-tree-sitter` EXISTS
(28 packages installed). Test runner is `bun test` (NOT vitest); style is
`import { describe, expect, test } from 'bun:test'` — see
`apps/aion-ops-console/src/features/authoring/lib/diagnostics.test.ts`.

**Put it at**
`apps/aion-ops-console/src/features/authoring/lib/highlight-assets.test.ts`.

### The probe is already PROVEN — do not re-derive it

Parsing a keyword ALONE ON A LINE yields a `(keyword)` node, so the test can be
driven by the grammar rather than by a hardcoded list. Measured, all 56 grammar
words, zero misses:

```
grammar words: 56
not captured as keyword when alone on a line: []
```

56 is exactly right and cross-checks the Rust gate: 57 `Keyword::ALL` variants,
minus `true`/`false` (which the grammar gives its own `boolean` node), plus `in`
(a positional word the grammar deliberately colours). 57 − 2 + 1 = 56.

### Assertions the test must carry

1. **The artefact knows every word the grammar declares.** Read
   `tools/tree-sitter-awl/grammar.js`, extract the array between
   `const keywords = [` and `];` (split on `'`, take odd indices), and for each
   word assert the VENDORED wasm captures it as `keyword`. **This is the whole
   point** — it is what catches "grammar edited, wasm not rebuilt".
2. **Non-vacuity**: assert the extracted list has > 50 entries, or every loop
   below passes over nothing.
3. **Control**: `worker` captures as `keyword` (proves the harness works).
4. **Inverted control**: an ordinary name such as `incident_id` captures NO
   `keyword` (proves the test is not just colouring everything).
5. **The regression this closes**, in real position:
   `action ask(prompt: String) -> String` / `    agent, timeout 10m` — assert
   `agent` and `advisory` capture as `keyword`.
6. **The deliberate omission holds**: in
   `wait approval timeout 30s -> answer`, the binding `answer` captures
   NOTHING. That asymmetry is the load-bearing design decision — see the design
   note.

### Mechanics

- Load BOTH vendored artefacts, not the source grammar:
  `apps/aion-ops-console/src/features/authoring/assets/awl.wasm` and
  `.../assets/highlights.scm`.
- API: `import { Language, Parser, Query } from 'web-tree-sitter'`, then
  `await Parser.init()`, `await Language.load(path)`, `parser.setLanguage(...)`,
  `new Query(language, scmText)`, `query.captures(tree.rootNode)`. Capture
  objects have `.name` and `.node`; `.node.text` works. Call `tree.delete()`.
- Path from the test file to the repo root is **six** levels
  (`lib → authoring → features → src → aion-ops-console → apps → root`), so
  grammar.js is `../../../../../../tools/tree-sitter-awl/grammar.js`. Prefer
  resolving from `import.meta.dir` rather than trusting the cwd.
- Run it with `cd apps/aion-ops-console && bun test`. **Do not land it unrun.**

### Then mutation-check it

Replace the vendored `awl.wasm` with the pre-fix one and confirm the new test
fails on `agent`/`advisory` and nothing else. The old bytes are recoverable:
`git show 973b3d58~1:apps/aion-ops-console/src/features/authoring/assets/awl.wasm`.

## Scratch toolchain — installed and working, reuse it

`tree-sitter-cli@0.26.10` (the pinned version) and `web-tree-sitter@0.26.10` are
installed under
`<scratchpad>/ts-cli/node_modules/.bin` and `<scratchpad>/ts-cli/node_modules/`.

`tree-sitter-cli` will NOT install inside `tools/tree-sitter-awl` — that
package's own native `tree-sitter` dependency fails `node-gyp` under node v26.
Install it into a scratch prefix instead:
`npm install --prefix <scratch> tree-sitter-cli@0.26.10`.

Full grammar change recipe, all four steps mandatory:

```
cd tools/tree-sitter-awl
tree-sitter generate && tree-sitter test        # expect 18/18
tree-sitter build --wasm
cp tree-sitter-awl.wasm apps/aion-ops-console/src/features/authoring/assets/awl.wasm
cd <repo> && cargo xtask build-ops-console      # WITHOUT THIS THE FIX NEVER SHIPS
cargo xtask verify-ops-console                  # expect: matches a clean rebuild (23 files)
```

## Verified clean, nothing owed (do not re-investigate)

- All 8 editor query files (`zed-awl`, `nvim-awl`, in-repo, vendored) compile
  against the current grammar — no stale node references.
- `crates/aion-awl/src/unlowered.rs` is exemplary: exhaustive statement match,
  no wildcard, one documented honest refusal (`after` on substeps).
- The printer covers all 13 `Statement` variants.
- The server's AWL tests use `include_str!` from real fixtures — no duplicated
  syntax that could drift.

## 🔴 Standing constraints — UNCHANGED

**The frozen queue. Each item runs ONLY on Cally's explicit NAMED GO naming the
exact command. No named GO, no launch.**

1. `cargo clippy -p aion-rs -p aion-server --all-targets -- -D warnings`
   (`-p aion-package` discharged at `4462765a`). **Predicted RED, on the
   record.**
2. `cargo test -p aion-rs` — the #106 battery. Until it runs, `4d9ca3b0` is
   unverified at battery level and must be described that way.
3. `cargo build -p aion-cli` + the **#97 live demonstration** against a real
   server, quoting recorded bytes. NOT the `target/debug/aion` shortcut.

Newly owed and in the same held class:
`cargo test -p aion-server --test ops_console_packaging`, which asserts the
refreshed bundle ships in the packaged crate. Owed on this tree before release.

Also: **do not restart Tom's server** (#108 — running from a deleted binary).
**#83** stays designed-and-held; it lands as ONE motion after the two owed
gates, on its own battery.

Durability-dirs matter is CLOSED at Cally's seat — facet of ruled event #1,
count stays two, nothing owed from this seat.
