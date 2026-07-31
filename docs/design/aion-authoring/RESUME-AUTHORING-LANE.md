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

## The queue — RELEASED 2026-07-31 ~08:13Z

The freeze is lifted and the per-command GO regime ended with it. Cally
released the held queue in its recorded order under normal rules: announced,
owned, bounded, re-declared before each item. GB-scale deletions still
declare-and-wait.

**Gate 1 (Cally's, still standing at the time of writing): wait for Athena's
fresh gate-two disk baseline.** Her remedy half-life is ~2h, so every disk
number from this morning is stale and the queue launches against her fresh
reading. Nothing below starts before that.

My own disk reading, taken 08:15Z as an input to hers, not a substitute:
`/System/Volumes/Data` 825Gi used / **72Gi avail**, 92%. The largest single
object on the box is **aion's `target/` at 32G — mine**, against iridium 9.1G,
beamr 8.2G, manifold 2.5G, liminal 2.0G, lys 1.6G. If Athena's reading is
tight, my target dir is the cheapest remedy anyone has today; that is offered
to Cally and not yet taken.

Then, in order:

1. `cargo clippy -p aion-rs -p aion-server --all-targets -- -D warnings`
   (`-p aion-package` discharged at `4462765a`). **Predicted RED, on record.**
   Findings land on their OWN battery, not mixed into another lane's.
2. `cargo test -p aion-rs` — the #106 battery. Until it runs, `4d9ca3b0` is
   unverified at battery level and must be described that way.
3. **4a — the #97 live demonstration.** `cargo build -p aion-cli`, a real
   server, and quoted recorded bytes. NOT the `target/debug/aion` shortcut.
4. **4b — #83**, its own motion on its own battery.

**Item 4 was raised as one thing and is two.** Cally's release named "the
`BC4_DUMP_FAILED` tap" as #97's evidence. It is not. That tap lives in
`crates/aion/tests/awl_bc4_differential/driver.rs:462` and dumps a failed
fixture's reference trail — it is **#83's** close condition
(`AWLFAILED-CARRIES-NOTHING.md:155`: re-run it on
`step-bodies/valid/send_signal` and quote the recorded detail; #83 closes when
the recorded JSON names the failure). **#97** is the declared-contract signal
boundary, and its acceptance
(`BOUNDARY-CONTRACT-MEASUREMENT.md:179`) is a live server, a
contract-violating signal, and quoted history proving the run's bytes are
byte-identical and it stays parked, plus the caller's 400. Split raised with
Cally; neither gets dropped.

Still owed and unscheduled: `cargo test -p aion-server --test
ops_console_packaging`, which asserts the refreshed bundle ships in the
packaged crate. Owed on this tree before release.

## 🔴 Standing constraints

- **Do not restart or go near Tom's server** (#108 — running from a deleted
  binary, cannot be restarted). The #97 proof stands up its OWN server, own
  port, own store.
- **Do not touch `aion-rs`/`aion-server` SOURCE until items 1 and 2 have run.**
  A battery that goes red must have one lane's changes under it, not two.
  Until then, work stays in: `aion-awl`, `aion-awl-lsp`, the ops console,
  `tools/tree-sitter-awl`, `editors/`, docs.
