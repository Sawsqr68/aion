# RESUME — the authoring lane, where it stands

Written 2026-07-31 by Vesper. Supersedes `RESUME-CONSOLE-WASM-GATE.md`, which
was spent and deleted.

## Landed (all pushed unless marked)

| Commit | What |
| --- | --- |
| `6cc3ca1d` | AWL LSP references + rename, checker-driven, refusal-carrying (#111 CLOSED) |
| `973b3d58` | The tree-sitter grammar was two reserved words behind the compiler — `agent`, `advisory` — plus the Rust gate |
| `c06dbc45` | The embedded ops-console bundle was stale on main, independently; refreshed |
| `c5160637` | The console's vendored wasm is now checked against the grammar it was built from |
| `1bee0b67` | Zed and Neovim were colouring a language two releases old; mirror pushed and pinned; four-test gate (#112 CLOSED) |
| mirror `8d4d63e` | github.com/tomWhiting/tree-sitter-awl synced to aion `c5160637` |
| `b989789f` | `textDocument/documentHighlight` — declaration as Write, uses as Read, same span set the rename edits |
| `ec56f1b4` | **Item 1's finding fixed**: one wire mapping was spelled two ways in `wire_from_engine` |
| `013ae8e5` | **#97 CLOSED** — the live demonstration, quoted bytes, inverted control |

The grammar story is in `GRAMMAR-VOCABULARY-DIVERGENCE.md` — read it before
touching any grammar, query, wasm or editor pin. The #97 evidence is in
`BOUNDARY-CONTRACT-MEASUREMENT.md`.

## The rule this lane exists to enforce

**AWL's vocabulary lives in six places and nothing used to compare them.**

1. `crates/aion-awl/src/lexer/tokens.rs` — `Keyword::ALL`, authoritative.
2. `tools/tree-sitter-awl/grammar.js` + `queries/` — authoritative for editors.
3. `apps/aion-ops-console/src/features/authoring/assets/awl.wasm` — vendored,
   compiled; what the operator's own authoring surface loads.
4. `crates/aion-server/ops-console-embed/` — the built bundle the server serves.
5. `editors/zed-awl/languages/awl/*.scm` + the mirror `rev` pin.
6. `editors/nvim-awl/queries/awl/*.scm`.

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

## THE QUEUE — where it actually stands

| Item | State |
| --- | --- |
| 0. scoped clean | ✅ done — **14.3 MiB, a rounding error** |
| 1. clippy aion-rs + aion-server | ✅ RED as predicted → fixed → **exit 0** (`ec56f1b4`) |
| 4a. #97 live proof | ✅ **CLOSED** (`013ae8e5`) |
| 2. `cargo test -p aion-rs` | 🔄 **RUNNING** — launched ~08:41Z on a measured-quiet box |
| 4b. #83 | ⬜ NEXT — and it is a BUILD, not a tap re-run (see below) |

### Item 0 — the scoped clean, measured small and reported as such

`cargo clean -p aion-integration-cli -p million-processes -p xtask` removed
**330 files, 14.3 MiB**. `du -sh target/debug` read 31G before and 31G after —
it does not move a single displayed digit. Three leaf crates out of
twenty-three was always going to be small; it was smaller. **Reported to Cally
as not a remedy, nothing to be planned against it.** The full 32G wipe was
correctly refused: items 1–3 rebuild aion-rs/aion-server/aion-cli anyway, so a
cold wipe spends the freed space on its own rebuild and RAISES the peak.

### Item 1 — the finding and why it was fixed rather than suppressed

`too_many_lines` 102/100 on `wire_from_engine`, `aion-server/src/error.rs:579`.
The length was the symptom. **Eleven arms wrote
`WireError::backend_with_type(NAME, source.to_string())` inline while the rest
called `backend(NAME, source)` — and `error_engine::backend_wire` is that
expression verbatim.** One mapping, two spellings, one match. Collapsed to the
helper: 102 → 93 lines, no behaviour change.

**Deliberately NOT split into two functions**, which is the other way to satisfy
the lint: splitting needs a catch-all arm, which trades a lint for a silent
hole — a new `EngineError` variant would stop being forced to declare its wire
class. Exhaustiveness there is load-bearing.

Non-vacuity: the identical command produced a real finding six minutes earlier;
only the fix changed. Item 2's battery is unaffected — the dependency runs
`aion-server → aion-rs` one way only.

### 4a — CLOSED, and it found two defects the bench never could

Full evidence in `BOUNDARY-CONTRACT-MEASUREMENT.md`. Short form: run `0e45c766`
parked; `{"note": 42}` against `note: String` → **HTTP 400 / SignalRefused**
naming the pointer, value, expected type and deciding package version; history
**byte-identical** (779 bytes, sha256 `96b7d4ee…`, `cmp`-verified) read
strictly behind the 400.

**The inverted control is the part that makes it mean anything.** A valid
`ruling` on the same name → HTTP 200, `SignalReceived` + `WorkflowCompleted`,
new sha. Without it, a server that ignored every signal would have passed.

Teardown: explicit kill, exited 2s, zero orphans, both ports released, positive
control `pgrep -x zsh` = 17.

**Two defects filed separately, quarantined from #97's record:**

- **#113 (P0) — `AION_HOME` is an ISOLATION INVERSION.** With a legacy
  `aion-data` in CWD, the server WARNs "migration required" and then uses the
  legacy path. That path is 6.4G and Tom's live server holds **66 open handles**
  under it including `writer.lock`. **The only thing between my proof server and
  a live WAL was an unrelated missing feature flag.** Mechanism confirmed by
  relaunching from a clean directory, not guessed.
- **#114 — `aion server --endpoint` is advertised in `--help` and never read.**
  `crates/aion-cli/src/server.rs` has no `endpoint` handling; the bind is
  `server.grpc_address`. It failed loudly only because Tom's server squats
  50051. On a free box it binds wrong and looks fine.

### 4b — #83 is a BUILD, not a tap re-run

This was mis-scoped in my own earlier note. `AWLFAILED-CARRIES-NOTHING.md`
carries a ready-to-build checklist:

1. `error.gleam`: add `AwlEngineFailed(String)`; encode/decode; make
   `map_engine_error` carry `EngineFailure(message)`. Fix `map_spawn_error` and
   `map_timer_error` in the SAME commit (Half A).
2. `aion_awl_codec_test.gleam`: roundtrip the new variant; extend the
   exhaustive list.
3. `abi.rs`: add the tag to `AWL_ERROR_TAGS` with a comment — the closed set is
   a real pin and must not be widened casually.
4. Rebuild the prebuilt fixture copies of `aion@awl@error`, or confirm the
   harness rebuilds them.
5. Gates: `gleam test` in `gleam/aion_flow`; `cargo test -p aion-awl`;
   `cargo test -p aion-rs --test awl_bc4_differential` — must stay
   byte-identical, both backends call the same changed function.
6. **The close condition is the bytes, not a green test**: re-run the
   `BC4_DUMP_FAILED` tap on `step-bodies/valid/send_signal` and quote the
   recorded detail. #83 closes when the recorded JSON NAMES the failure.

The doc is explicit that it lands as ONE motion (splitting puts a partial fix
of a diagnosis defect on main while its headline case still records nothing)
and **after** the owed gates — which is why item 2 was run before it.

## Method notes worth keeping

- **A count is not an identification.** I reported "cargo=1 rustc=3, not me" at
  08:26Z, inferring from my flat target dir. Athena's argv capture showed it was
  my own `aion-cli` build. `pgrep` counts cannot distinguish my compiler from a
  stranger's — only argv plus session can. The 08:21Z finding stood because it
  was argv-measured; the 08:26Z one did not because it was inferred.
- **`pgrep -f` matches argv TEXT** and will count another seat's *quiet-check
  script* as a live compiler. Use `pgrep -x <exe>` plus a positive control
  (`pgrep -x zsh` non-zero).
- **Measure the remote, not a copy of it.** A stale local clone produced a
  confident twelve-word finding that was wrong; one `git fetch` reduced it to
  the two words already on record.
- **A noisy diff hides a real one.** The editor query copies differ by design,
  and the missing nodes sat inside that noise. The gate asserts node COVERAGE
  and says nothing about capture names.
- **`git checkout --` after a mutation check reverts uncommitted FIXES too.**
  Copy to the scratchpad and restore from there.
- **Verify isolation from the product's own log, not from the flag you passed.**
  This is the single reason #113 is a finding and not an incident.
- **An unchanged reading needs an inverted control.** Byte-identical history
  proves nothing until a valid input is shown to change it.

## Owed, in order

1. **#83 / 4b** — the build above, one motion, after item 2 reports.
2. **DM Artemis Peach directly, one line, when the queue closes** — that word is
   her GO-6 launch signal (full-workspace nextest, +6–12 GiB, needs the quiet
   box). She is idle until it arrives. Do not let it slip into a wrap-up.
3. **Cally's two findings on `enforce_contract`** (asymmetry itself APPROVED):
   - **A — the degraded state has no queryable trace after boot.** The only
     evidence a resident package runs with its contract off is a boot-time error
     line — "a log line at a moment nobody is watching", the exact hazard the
     Refuse door's own doc names. The catalog cannot answer *which resident
     packages run unchecked*. Remedy: a flag on the version record surfaced
     through `list_workflow_versions` — **deletion-class**, makes the invisible
     state impossible rather than better-logged.
   - **B — `recovery_still_loads_a_persisted_package_the_door_would_now_refuse`
     pins the LOADS half only.** A silently-quiet recovery load would pass, and
     the report is the ENTIRE compensation for the disabled contract. Pin the
     report itself.
   - Plus one design sentence: a middle door (load for pinned runs, deactivate
     the route) existed and was implicitly rejected. Right call for
     restart-availability — say so where the next reader will look.
4. `cargo test -p aion-server --test ops_console_packaging` — asserts the
   refreshed bundle ships in the packaged crate. Owed on this tree before
   release.
5. **#113 and #114** — the two defects 4a surfaced.

## 🔴 Standing constraints

- **Do not restart or go near Tom's server** — pid 99844,
  `./target/release/aion server`, ports 8080/50051, running from a DELETED
  binary and unrestartable (#108). It is on Athena's board as NAMED AND
  UNTOUCHABLE. Any proof stands up its OWN server, own port verified free
  before bind, own store under an owner-only home.
- **Awaiting Tom (HOLD OPEN, #88):** whether Gleam is a supported authoring
  surface or AWL is the only one.
