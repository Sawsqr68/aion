# Leg `migrate` report — beamr ≥0.19.2 (issue #70)

## 1. Verdict

**BLOCKED — Path B.** Raising the workspace floor to `beamr 0.19.2`, adding the
single source-required `ParsedModule::type_chunk` field, compiling `aion-awl`, and
running all 28 `runtime_codecs` tests against real Gleam 1.18.1 / OTP 27 are
reachable and passed. The deliverable migration is not reachable: the highest
non-yanked `liminal-sdk` release is still 0.7.0 and its default `std` feature
selects `liminal-rs ^0.5.5`, which retains `beamr ^0.16.3` and a second haematite
line. The control with `liminal-sdk` default features disabled makes the graph
clean, isolating that one dependency edge, but it removes the real TCP push client
and canonical wire codec that aion ships. Per the no-half-migration rule, all
manifest, lock, and source changes were reverted; only this report lands.

## 2. R0 — crates.io resolver input and path choice

Command:

```console
$ curl -s https://index.crates.io/li/mi/liminal-sdk | tail -n 1
```

Highest non-yanked row, verbatim:

```json
{"name":"liminal-sdk","vers":"0.7.0","deps":[{"name":"futures-core","req":"^0.3","features":[],"optional":false,"default_features":false,"target":null,"kind":"normal"},{"name":"js-sys","req":"^0.3.102","features":[],"optional":true,"default_features":true,"target":"cfg(all(target_arch = \"wasm32\", target_os = \"unknown\"))","kind":"normal"},{"name":"liminal","req":"^0.5.5","features":[],"optional":true,"default_features":true,"target":null,"kind":"normal","package":"liminal-rs"},{"name":"liminal-protocol","req":"^0.7.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"liminal-server","req":"^0.8.0","features":[],"optional":true,"default_features":true,"target":null,"kind":"normal"},{"name":"serde","req":"^1","features":["derive","alloc"],"optional":false,"default_features":false,"target":null,"kind":"normal"},{"name":"serde_json","req":"^1","features":["alloc"],"optional":false,"default_features":false,"target":null,"kind":"normal"},{"name":"spin","req":"^0.9","features":["mutex","spin_mutex"],"optional":false,"default_features":false,"target":null,"kind":"normal"},{"name":"thiserror","req":"^2","features":[],"optional":false,"default_features":false,"target":null,"kind":"normal"},{"name":"tungstenite","req":"^0.30.0","features":["handshake"],"optional":true,"default_features":false,"target":null,"kind":"normal"},{"name":"wasm-bindgen","req":"^0.2.105","features":[],"optional":true,"default_features":true,"target":"cfg(all(target_arch = \"wasm32\", target_os = \"unknown\"))","kind":"normal"},{"name":"web-sys","req":"^0.3.102","features":["BinaryType","Blob","CloseEvent","Event","MessageEvent","WebSocket"],"optional":true,"default_features":true,"target":"cfg(all(target_arch = \"wasm32\", target_os = \"unknown\"))","kind":"normal"}],"cksum":"124c0d4f4aee4142db4c4ea9931c9872570fa01598c1fb27798ba41e2f6fe728","features":{},"features2":{"browser":["dep:js-sys","dep:wasm-bindgen","dep:web-sys"],"default":["std"],"embedded":["std","dep:liminal-server"],"std":["thiserror/std","serde/std","dep:liminal","dep:tungstenite"]},"yanked":false,"rust_version":"1.85","pubtime":"2026-08-13T10:25:15Z","v":2}
```

The row requires `liminal ^0.5.5`, which does not admit 0.6.0. **Path B was
taken.**

## 3. R3 — beamr 0.18.2 → 0.19.2 API census

The item-signature census covered public functions, structs, enums, traits, type
aliases, constants, and statics across both unpacked registry sources. The hot
field/variant surface and all 62 beamr paths named by aion were cross-checked by
the clean `cargo build -p aion-awl` and `runtime_codecs` compile against 0.19.2.

| # | Delta | Kind | Aion-visible site | Result |
|---:|---|---|---|---|
| 1 | `ParsedModule` gained `pub type_chunk: Option<Vec<u8>>` | breaking, E0063 | `crates/aion-awl/src/mir/select/assemble.rs:59` | **temporary fix validated; not landed on Path B** |
| 2 | `resolve_imports` now returns `Vec<ResolvedImport>` rather than `Vec<Option<ResolvedImport>>` | breaking signature | `assemble.rs` pass-through | **no change needed**; inferred `resolved` is handed directly to `validate_module` |
| 3 | `validate_module` now accepts `&[ResolvedImport]` rather than `&[Option<ResolvedImport>]` | breaking signature | same pass-through | **no change needed**; changed in lockstep with delta 2 |
| 4 | `ProcessContext::{dict_get_all, dict_erase_all, dict_get_keys}` removed/replaced | breaking | zero sites | **no change needed** |
| 5 | `native::TermAccumulator`, `ProcessContext::with_accumulator`, and nine accumulator methods added | additive | zero sites | **no change needed** |
| 6 | `Process::{gc_attempts, gc_completions, nested_handler_floor, set_nested_handler_floor, pop_exception_handler_in_nest}` added | additive | zero sites | **no change needed** |
| 7 | `scheduler::{ProcessVmCensus, process_vm_census}` added | additive | zero sites | **no change needed** |
| 8 | `AtomTable::with_common_atoms()` delegates to `new()`; signature unchanged | behavioural | ABI/harness users | **no change needed** |
| 9 | `From<ReplayMismatch> for ExecError` no longer cfg-limited | additive under aion features | zero sites | **no change needed** |
| 10 | Validator clears stale frame size at control-flow terminators and updates it for `Trim`; #64 regression added | behavioural payoff | `runtime_codecs` beam population | **measured: no source change needed; 28/28 passed with real Gleam** |
| 11 | `src/ar1_ordering_control.rs` added; `loader/decode` is byte-for-byte directory-identical | no decode API delta | exhaustive `Instruction`/`Operand` matches | **no change needed** |

Feature names `json`, `encode`, `test-support`, and `readiness` remain present;
workspace consumers inherit them and need **no change**.

### Confirmation 1 — item signatures

Command (the script extracts public item declaration lines from each source and
prints the relevant removed/changed dictionary methods):

```console
$ python3 - <<'PY'
from pathlib import Path
import re, difflib
base = Path('/home/aion/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f')
old = base/'beamr-0.18.2'/'src'
new = base/'beamr-0.19.2'/'src'
pat = re.compile(r'^\s*pub\s+(?:async\s+)?(?:unsafe\s+)?(?:fn|struct|enum|trait|type|const|static)\b')
def extract(root):
    out=[]
    for p in sorted(root.rglob('*.rs')):
        rel=p.relative_to(root)
        for line in p.read_text().splitlines():
            if pat.search(line): out.append(f'{rel}:{line.strip()}')
    return out
for line in difflib.unified_diff(extract(old), extract(new), fromfile='beamr-0.18.2', tofile='beamr-0.19.2', lineterm=''):
    if line.startswith(('-', '+')) and not line.startswith(('---','+++')) and any(name in line for name in ('dict_get_all','dict_erase_all','dict_get_keys')):
        print(line)
PY
-native/context/mod.rs:pub fn dict_get_all(&self) -> Result<Vec<(Term, Term)>, Term> {
-native/context/mod.rs:pub fn dict_erase_all(&mut self) -> Result<Vec<(Term, Term)>, Term> {
-native/context/mod.rs:pub fn dict_get_keys(&self, value: Term) -> Result<Vec<Term>, Term> {
+native/context/mod.rs:pub fn dict_erase_all_to_list(&mut self) -> Result<Term, Term> {
```

The complete census also records the replacement additions
`dict_entries_to_list` and `dict_keys_for_value_to_list`; the three old methods
above are the three removals.

### Confirmation 2 — removed method use in aion

```console
$ grep -rn 'dict_get_all\|dict_erase_all\|dict_get_keys' --include='*.rs' crates/ benchmarks/
```

Output: **empty**.

### Confirmation 3 — decoder surface

```console
$ diff -rq /home/aion/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/beamr-0.18.2/src/loader/decode /home/aion/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/beamr-0.19.2/src/loader/decode
```

Output: **empty**.

## 4. R2 — `type_chunk` decision and validated patch

The decision was made from beamr 0.19.2 source, not from a default guess:

- `loader/load.rs:32-40` defines `type_chunk` as raw, verbatim `Type` bytes whose
  indices are used by typed registers.
- `loader/load.rs:360-362` populates it only with
  `find_chunk(&chunks, b"Type").map(<[u8]>::to_vec)` while decoding an input
  container.
- Aion's assembler synthesises a module from `BuilderParts`; `builder.rs:209-225`
  shows those parts contain atoms, literals, imports, and lambdas, but no type
  table or type information.
- All three `Operand::TypedRegister` occurrences in
  `crates/aion-awl/src/mir/select/` are read-side match arms in
  `inspect_support.rs:351,428,442`; there is no construction site.
- `loader/encode/container.rs:116,157-159` uses `type_chunk.is_some()` to arm the
  operand encoder and conditionally re-emits the bytes. `Some(Vec::new())` would
  falsely claim a table exists and emit an empty `Type` chunk.
- `loader/encode/compact.rs:106-132` returns
  `EncodeError::TypedRegisterWithoutTypeChunk` if a typed register is ever emitted
  without that chunk. That hard failure is the desired future behavior.

Therefore `None` is truthful: there is no type table to carry, not a table that
was dropped.

Exact source patch validated in the temporary 0.19.2 tree:

```diff
@@ -66,6 +66,11 @@
         literals: parts.literals,
         string_table: Vec::new(),
         line_info: Vec::new(),
+        // The `Type` chunk is absent because this assembler synthesises from
+        // BuilderParts, which carry no type table, and emits no TypedRegister.
+        // If that changes, beamr rejects encoding with
+        // TypedRegisterWithoutTypeChunk rather than writing a dangling index.
+        type_chunk: None,
     };
```

Validation:

```console
$ cargo build -p aion-awl
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 03s
exit=0 wall_seconds=64

$ grep -n 'allow(\|expect(\|ignore' /tmp/wf70-payoff.patch
```

The forbidden-attribute grep output was empty. `resolve_imports` and
`validate_module` compiled unchanged.

## 5. R4 — payoff measurement on the #64-excluded box mix

Environment used for the authoritative run matched the gate:

```console
$ export HOME=/home/aion
$ export PATH="$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin"
$ gleam --version
gleam 1.18.1
$ rustc --version
rustc 1.97.1 (8bab26f4f 2026-07-14)
$ cargo --version
cargo 1.97.1 (c980f4866 2026-06-30)
```

### Lock witness in the same tree/session as the test

```console
$ cargo tree -i beamr@0.19.2 --workspace -e normal,build
beamr v0.19.2
├── aion-awl v0.17.0 (/home/aion/fleet/runs/fleet-70-beamr-floor/legs/migrate/crates/aion-awl)
│   ├── aion-awl-lsp v0.17.0 (/home/aion/fleet/runs/fleet-70-beamr-floor/legs/migrate/crates/aion-awl-lsp)
│   │   └── aion-cli v0.17.0 (/home/aion/fleet/runs/fleet-70-beamr-floor/legs/migrate/crates/aion-cli)
│   ├── aion-awl-package v0.17.0 (/home/aion/fleet/runs/fleet-70-beamr-floor/legs/migrate/crates/aion-awl-package)
│   │   ├── aion-cli v0.17.0 (/home/aion/fleet/runs/fleet-70-beamr-floor/legs/migrate/crates/aion-cli)
│   │   └── aion-server v0.17.0 (/home/aion/fleet/runs/fleet-70-beamr-floor/legs/migrate/crates/aion-server)
│   │       └── aion-cli v0.17.0 (/home/aion/fleet/runs/fleet-70-beamr-floor/legs/migrate/crates/aion-cli)
│   ├── aion-cli v0.17.0 (/home/aion/fleet/runs/fleet-70-beamr-floor/legs/migrate/crates/aion-cli)
│   └── aion-server v0.17.0 (/home/aion/fleet/runs/fleet-70-beamr-floor/legs/migrate/crates/aion-server) (*)
└── aion-rs v0.17.0 (/home/aion/fleet/runs/fleet-70-beamr-floor/legs/migrate/crates/aion)
    ├── aion-cli v0.17.0 (/home/aion/fleet/runs/fleet-70-beamr-floor/legs/migrate/crates/aion-cli)
    ├── aion-client v0.17.0 (/home/aion/fleet/runs/fleet-70-beamr-floor/legs/migrate/crates/aion-client)
    │   └── aion-cli v0.17.0 (/home/aion/fleet/runs/fleet-70-beamr-floor/legs/migrate/crates/aion-cli)
    ├── aion-server v0.17.0 (/home/aion/fleet/runs/fleet-70-beamr-floor/legs/migrate/crates/aion-server) (*)
    └── million-processes v0.1.0 (/home/aion/fleet/runs/fleet-70-beamr-floor/legs/migrate/benchmarks/million-processes)
```

### `runtime_codecs`

```console
$ cargo test -p aion-awl --test runtime_codecs
running 28 tests
test gleam_test_support::an_unreachable_registry_is_infrastructure ... ok
test gleam_test_support::a_rejected_module_is_a_defect ... ok
test gleam_test_support::an_unsatisfiable_requirement_is_a_defect ... ok
test gleam_test_support::the_ci_tar_archive_failure_is_infrastructure ... ok
test gleam_test_support::download_progress_alone_is_not_a_registry_failure ... ok
test fix_cycle::empty_list_literal_materializes_nil_at_runtime ... ok
test assert_list_badmatch_reports_the_subject_at_runtime ... ok
test concat::general_concat_executes_with_emitter_byte_parity ... ok
test semantic_pins::value_boolean_operators_skip_faulting_rhs_at_runtime ... ok
test pipes::pipe_combinators_execute_with_reference_parity ... ok
test fix_cycle::dev_brief_fix_cycle_encode_reaches_dispatch_with_reference_parity ... ok
test direct_codecs_execute_with_reference_parity ... ok
test fork_generality::plain_call_site_config_merges_per_key_with_reference_parity ... ok
test nested_collection_predicates_execute_with_reference_parity ... ok
test fork_generality::fork_inside_loop_completes_two_passes_with_reference_parity ... ok
test fork_generality::sequential_fork_site_config_and_index_prelude_dispatch_with_reference_parity ... ok
test pipes::pipe_child_stage_executes_with_reference_parity ... ok
test semantic_pins::duration_boundary_executes_exactly_and_next_second_is_refused ... ok
test fork_generality::named_fork_heterogeneous_site_config_parity ... ok
test fork_generality::named_fork_homogeneous_site_config_parity ... ok
test fork_generality::parallel_action_fork_site_config_dispatches_with_reference_parity ... ok
test child_envelope::real_child_outcome_envelope_completes_parent_fold_on_both_backends ... ok
test child_envelope::child_output_codec_is_strict_and_symmetric_on_both_backends ... ok
test stmt_parity::index_and_lists_execute_with_reference_parity ... ok
test stmt_parity::wait_signal_binds_payload_with_reference_parity ... ok
test stmt_parity::child_call_statement_and_spawn_execute_with_reference_parity ... ok
test fallible_predicates_execute_through_production_hosts_with_parity ... ok
test stmt_parity::wait_timeout_arms_execute_with_reference_parity ... ok

test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.86s
runtime_codecs_exit=0 wall_seconds=9
```

The harness emitted `COMPILED ... under real gleam build` for every Gleam-backed
fixture, including the named #64 hanger
`stmt_parity::child_call_statement_and_spawn_execute_with_reference_parity`.
This is the box mix previously excluded by #64, and beamr 0.19.2's validator
accepted its generated beams.

An earlier invocation inherited a PATH without `/usr/local/bin`, printed
`SKIPPED (PROVES NOTHING): gleam is not available on PATH`, and was discarded.
The result above is the authoritative gate-PATH rerun; it used real Gleam 1.18.1.

### Lints

```console
$ cargo clippy -p aion-awl --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 40.64s
clippy_exit=0 wall_seconds=41
```

```console
$ cargo fmt --check
Diff in /home/aion/fleet/runs/fleet-70-beamr-floor/legs/migrate/crates/aion-server/src/filesystem.rs:47:
 /// same absence of an override.
 const PRIVATE_FILE_MODE: u32 = 0o600;
 
-
 /// A held directory descriptor confining all subsequent operations beneath it.
 pub(crate) struct ConfinedDir {
     dir: Dir,
fmt_exit=1 wall_seconds=6
```

`cargo fmt --check` is **not green** because of a pre-existing extra blank line in
`crates/aion-server/src/filesystem.rs`, outside this leg's strict source fence.
It was not changed. The temporary migration's only Rust diff was the five-line
`type_chunk` patch shown above.

## 6. R6.1 — blocker reproduced in scratch

Scratch setup:

```console
$ export HOME=/home/aion
$ export PATH="$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin"
$ rm -rf /tmp/wf70 && mkdir -p /tmp/wf70
$ git archive HEAD | tar x -C /tmp/wf70
$ cd /tmp/wf70
```

### Step 1 — beamr 0.19.2 alone

After changing only the workspace beamr requirement and running
`cargo metadata --format-version 1 >/dev/null`:

```console
beamr "0.16.3"
beamr "0.17.1"
beamr "0.19.2"
```

Two pre-floor beamr rows survive.

### Step 2 — all published estate heads

After additionally setting `haematite = "0.9.0"`, workspace `liminal-rs =
"0.6.0"`, `liminal-sdk = "0.7.0"`, and `liminal-server = "0.9.0"`, then
resolving:

```console
beamr "0.16.3"
beamr "0.17.1"
beamr "0.19.2"
haematite "0.8.3"
haematite "0.9.0"
liminal-rs "0.5.5"
liminal-rs "0.6.0"
```

Holder chains:

```console
$ cargo tree -i beamr@0.16.3 --workspace -e normal,build
beamr v0.16.3
└── liminal-rs v0.5.5
    └── liminal-sdk v0.7.0
        ├── aion-server v0.17.0 (/tmp/wf70/crates/aion-server)
        │   └── aion-cli v0.17.0 (/tmp/wf70/crates/aion-cli)
        └── aion-worker v0.17.0 (/tmp/wf70/crates/aion-worker)
            ├── aion-cli v0.17.0 (/tmp/wf70/crates/aion-cli)
            └── aion-server v0.17.0 (/tmp/wf70/crates/aion-server) (*)

$ cargo tree -i beamr@0.17.1 --workspace -e normal,build
beamr v0.17.1
└── haematite v0.8.3
    └── liminal-rs v0.5.5
        └── liminal-sdk v0.7.0
            ├── aion-server v0.17.0 (/tmp/wf70/crates/aion-server)
            │   └── aion-cli v0.17.0 (/tmp/wf70/crates/aion-cli)
            └── aion-worker v0.17.0 (/tmp/wf70/crates/aion-worker)
                ├── aion-cli v0.17.0 (/tmp/wf70/crates/aion-cli)
                └── aion-server v0.17.0 (/tmp/wf70/crates/aion-server) (*)
```

### Step 3 — control, liminal-sdk defaults disabled

With only `liminal-sdk` changed to
`{ version = "0.7.0", default-features = false }` and the graph re-resolved:

```console
beamr "0.19.2"
liminal-rs "0.6.0"
```

The control is clean. This isolates the blocker to
`liminal-sdk`'s `default -> std -> dep:liminal ^0.5.5` edge. It is evidence, not a
shipping option: aion-worker's manifest records that the default `std` feature
carries the real TCP push client and canonical wire codec.

## 7. Type wall in the split graph

The affected aion sites are exactly those cited in the brief:

- `crates/aion-worker/src/runtime/liminal.rs:63` imports workspace
  `liminal::protocol::WorkerRegistration`.
- Line 64 imports `liminal_sdk::PushClient`.
- Line 481 passes the workspace registration to
  `PushClient::connect_with_registration`.
- Lines 1430-1436 construct that workspace type.

A natural `cargo check -p aion-worker --features liminal-transport` of Step 2
first exposed an additional upstream incompatibility: `liminal-rs 0.5.5` resolves
its `haematite ^0.8.1` requirement to 0.8.3 but does not initialise 0.8.3's
`DatabaseConfig::node_cache_budget` field:

```console
error[E0063]: missing field `node_cache_budget` in initializer of `DatabaseConfig`
   --> .../liminal-rs-0.5.5/src/durability/store.rs:600:37
600 |     let database = Database::create(DatabaseConfig {
    |                                     ^^^^^^^^^^^^^^ missing `node_cache_budget`
error: could not compile `liminal-rs` (lib) due to 1 previous error
cargo_check_exit=101
```

To expose the type wall behind that earlier third-party failure, the scratch-only
old subtree was pinned to the floor it declares:

```console
$ cargo update -p haematite@0.8.3 --precise 0.8.1
$ cargo check -p aion-worker --features liminal-transport
error[E0308]: mismatched types
   --> crates/aion-worker/src/runtime/liminal.rs:481:69
    |
481 |         let client = PushClient::connect_with_registration(address, registration)
    |                      -------------------------------------          ^^^^^^^^^^^^ expected `WorkerRegistration`, found a different `WorkerRegistration`
    |                      |
    |                      arguments to this function are incorrect
    |
note: there are multiple different versions of crate `liminal` in the dependency graph
   --> .../liminal-rs-0.5.5/src/protocol/frame.rs:235:1
235 | pub struct WorkerRegistration {
    | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ this is the expected type
   ::: .../liminal-rs-0.6.0/src/protocol/frame.rs:235:1
235 | pub struct WorkerRegistration {
    | ----------------------------- this is the found type
note: associated function defined here
   --> .../liminal-sdk-0.7.0/src/remote/tcp/push_client.rs:190:12
190 |     pub fn connect_with_registration(
    |            ^^^^^^^^^^^^^^^^^^^^^^^^^
error: could not compile `aion-worker` (lib) due to 1 previous error
cargo_check_exit=101
```

The pin was diagnostic isolation in `/tmp/wf70`, not a proposed dependency
change. Even setting the lock duplication aside, the split `liminal` graph is not
type-correct.

## 8. Required unblock — addressed to `ablative-io/liminal`

Please publish **`liminal-sdk >=0.8.0`** with:

- `liminal ^0.6.0`
- `liminal-protocol ^0.7.0`
- `liminal-server ^0.9.0`

`liminal-rs 0.6.0` and `liminal-server 0.9.0` are already published on
`beamr ^0.19.1` / `haematite =0.9.0`. The measured public API deltas make the
remaining hop small: `liminal-sdk 0.5.0 -> 0.7.0` changed zero public item
signatures; haematite 0.8.3 -> 0.9.0 also changed zero and is a pure floor bump;
liminal-rs 0.5.0 -> 0.6.0 removed none; and aion names neither of the two changed
liminal-server items. `liminal-sdk` is the remaining edge that prevents a single
0.19-compatible estate.

## 9. Gate consequence and final worktree

The unchanged gate was run after reverting the temporary migration:

```console
$ sh .fleet-gate.sh
FAIL: pre-0.19.2 beamr in the resolved lock:
version = "0.16.3"
version = "0.17.1"
version = "0.18.2"
gate_exit=1 wall_seconds=0
```

This is `.fleet-gate.sh:23-27` working as designed: leg 1 refuses a graph with any
pre-0.19.2 beamr row. The gate was not edited or weakened. It correctly prevents
the full test and workspace-clippy legs from being described as a completed
migration before the upstream release exists.

Final scope choices:

- Reverted `Cargo.toml`, `Cargo.lock`, and
  `crates/aion-awl/src/mir/select/assemble.rs`; no half-migration ships.
- Did not add `[patch.crates-io]`.
- Did not disable `liminal-sdk` default features or remove/feature-gate the shipped
  liminal transport.
- Did not alter tests, add allow/expect/ignore attributes, or modify the gate.
- Did not edit `CHANGELOG.md`; that is a Path A action after the full migration is
  actually possible.
- Did not run later gate legs after the lock witness failed; `set -e` intentionally
  stops the immutable gate there.

## 10. Issue #70 acceptance, item by item

| Acceptance item | Status | Evidence |
|---|---|---|
| 1. Workspace/lock on beamr ≥0.19.2 | **blocked** | `liminal-sdk 0.7.0 -> liminal-rs ^0.5.5 -> beamr ^0.16.3`; Step 2 retains 0.16.3 and 0.17.1 |
| 2. Required 0.19 source migration | **blocked from landing** | The sole `type_chunk: None` patch is source-argued and compiles, but no half-migration may ship |
| 3. Remove the #64 gate exclusion / prove validator population | **blocked from landing** | The exclusion cannot come off until the workspace graph runs ≥0.19.2. Its evidence is already in hand: real Gleam 1.18.1 `runtime_codecs` passed 28/28 on beamr 0.19.2, including the named hanger |
| 4. Full gate clean | **blocked** | Immutable gate leg 1 reports all three pre-floor rows and exits 1 |

The reachable half is evidence, not the migration. The requested lock witness was
not obtained, and the outcome is therefore faithfully **BLOCKED**.
