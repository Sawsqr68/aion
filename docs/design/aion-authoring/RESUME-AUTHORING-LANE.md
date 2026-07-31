# RESUME — the authoring lane, where it stands

Written 2026-07-31 by Vesper. `origin/main` = **`1bee0b67`**, working tree
clean and fully pushed (only untracked `RELEASE-0.9.2.md`, pre-existing and not
mine). Supersedes `RESUME-CONSOLE-WASM-GATE.md`, which was spent and deleted.

## Landed this stretch (all pushed)

| Commit | What |
| --- | --- |
| `6cc3ca1d` | AWL LSP references + rename, checker-driven, refusal-carrying (task #111 CLOSED) |
| `973b3d58` | The tree-sitter grammar was two reserved words behind the compiler — `agent`, `advisory` — plus the Rust gate |
| `c06dbc45` | The embedded ops-console bundle was stale on main, independently; refreshed |
| `dd67ce88` | (resume note, now deleted) |
| `c5160637` | The console's vendored wasm is now checked against the grammar it was built from |
| `1bee0b67` | Zed and Neovim were colouring a language two releases old; mirror pushed and pinned; four-test gate (task #112 CLOSED) |
| mirror `8d4d63e` | github.com/tomWhiting/tree-sitter-awl synced to aion `c5160637` |

The whole story is in `GRAMMAR-VOCABULARY-DIVERGENCE.md` (same directory) —
read that before touching any grammar, query, wasm or editor pin.

## The rule this lane exists to enforce

**AWL's vocabulary lives in six places and nothing used to compare them.**

1. `crates/aion-awl/src/lexer/tokens.rs` — `Keyword::ALL`, authoritative.
2. `tools/tree-sitter-awl/grammar.js` + `queries/` — authoritative for editors.
3. `apps/aion-ops-console/src/features/authoring/assets/awl.wasm` — vendored,
   compiled; what the operator's own authoring surface loads.
4. `crates/aion-server/ops-console-embed/` — the built bundle the server serves.
5. `editors/zed-awl/languages/awl/*.scm` + the mirror `rev` pin.
6. `editors/nvim-awl/queries/awl/*.scm`.

Each is now gated:

| Seam | Gate |
| --- | --- |
| 1 ↔ 2 | `crates/aion-awl/src/lexer/grammar_vocabulary_tests.rs` |
| 2 ↔ 3 | `apps/aion-ops-console/src/features/authoring/lib/highlight-assets.test.ts` |
| 3 ↔ 4 | `cargo xtask verify-ops-console` — **CI-only, and CI is disabled (#71)** |
| 2 ↔ 5, 6 | `crates/aion-awl/tests/editor_assets.rs` |

**Changing the grammar means all four steps, every time:**

```
cd tools/tree-sitter-awl
tree-sitter generate && tree-sitter test                 # expect 18/18
sh test/corpus-sweep.sh && sh test/highlight-smoke.sh    # TS-G1, TS-G3
tree-sitter build --wasm
cp tree-sitter-awl.wasm ../../apps/aion-ops-console/src/features/authoring/assets/awl.wasm
cd ../.. && cargo xtask build-ops-console                # WITHOUT THIS THE FIX NEVER SHIPS
cargo xtask verify-ops-console                           # expect 23 files
# then the mirror: sync, push, anonymous ls-remote, bump rev + grammar-sha256
```

`tree-sitter-cli` will NOT install inside `tools/tree-sitter-awl` — that
package's own native `tree-sitter` dependency fails `node-gyp` under node v26.
Install the CLI alone into a scratch prefix:
`npm install --prefix <scratch> tree-sitter-cli@0.26.10`.

## Method notes worth keeping

- **Measure the remote, not a copy of it.** A stale local clone produced a
  confident twelve-word finding that was wrong; one `git fetch` reduced it to
  the two words already on record.
- **A noisy diff hides a real one.** The editor query copies differ by design
  (capture names follow each editor's theme vocabulary), and the missing nodes
  sat inside that noise. The gate therefore asserts node COVERAGE and says
  nothing about names.
- **`git checkout --` after a mutation check reverts uncommitted FIXES too.**
  Copy the file to the scratchpad and restore from there instead.

## 🔴 Standing constraints — UNCHANGED

**The frozen queue. Each item runs ONLY on Cally's explicit NAMED GO naming the
exact command. No named GO, no launch.**

1. `cargo clippy -p aion-rs -p aion-server --all-targets -- -D warnings`
   (`-p aion-package` discharged at `4462765a`). **Predicted RED, on record.**
2. `cargo test -p aion-rs` — the #106 battery. Until it runs, `4d9ca3b0` is
   unverified at battery level and must be described that way.
3. `cargo build -p aion-cli` + the **#97 live demonstration** against a real
   server, quoting recorded bytes. NOT the `target/debug/aion` shortcut.
4. `cargo test -p aion-server --test ops_console_packaging` — asserts the
   refreshed bundle ships in the packaged crate. Owed on this tree before
   release.

Also: **do not restart Tom's server** (#108 — running from a deleted binary).
**#83** stays designed-and-held (`AWLFAILED-CARRIES-NOTHING.md`); it lands as
ONE motion after the two owed gates, on its own battery.

In-class work (no GO needed): `aion-awl`, `aion-awl-lsp`, the ops console,
`tools/tree-sitter-awl`, `editors/`, docs.
