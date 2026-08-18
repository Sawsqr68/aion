# fleet plan — issue #70: migrate the workspace to beamr ≥0.19.2

Planning counsel, 2026-08-18. Branch `fleet-70`. Gate: `.fleet-gate.sh` (lock
witness → `cargo test --workspace` with NO gleam exclusions → workspace clippy).

---

## 0. Verdict, up front

The migration splits cleanly into a **reachable half** and a **hard wall**, and
the wall is not a matter of effort:

| Part | Status |
|---|---|
| Raise the workspace beamr floor to 0.19.2 | **reachable** |
| Fix the forced compile break (`type_chunk`) | **reachable**, exactly one site |
| Full 0.18→0.19 API census | **done below**, three deltas, one aion-visible |
| Payoff measurement (`aion-awl --test runtime_codecs` on 0.19.2) | **reachable** — `aion-awl` never touches liminal |
| Lock witness: ZERO beamr rows < 0.19.2 | **UNREACHABLE from this repo** |

The lock witness is blocked by a **third-party crate with no 0.19-compatible
release: `liminal-sdk`**. Every published version up to and including the head
(0.7.0) requires `liminal ^0.5.5`, and `liminal-rs 0.5.5` requires
`beamr ^0.16.3` + `haematite ^0.8.1` (→ `haematite 0.8.3` → `beamr ^0.17.1`).
`liminal-rs 0.6.0` and `liminal-server 0.9.0` are already published on
`beamr ^0.19.1`; **only `liminal-sdk` lags.**

Per fix-spec item 1 — *"if a third-party crate pins an old beamr, STOP and report
it as the blocker — no half-migration ships"* — the leg's terminal state is a
**measured blocker report**, not a partial bump. §6 shows the half-migration is
not merely untidy: it puts **two beamr copies in one `aion` binary**, which is
the failure class `liminal`'s own manifest names ("two disjoint beamr ranges
never share a tree", `/home/aion/stack/liminal/Cargo.toml:55-63`).

One leg: `.fleet/legs/migrate.md`.

---

## 1. Current state in this tree, grounded

### 1.1 The declared requirement

- `Cargo.toml:113` — `beamr = { version = "0.18.2", features = ["json", "encode"] }`
- `Cargo.toml:87-112` — the 26-line justification block for the 0.18.2/0.17.0/
  0.16.2/0.15.2/0.14.0 lineage. It narrates *why each hop was taken* and stops
  at 0.18.2; under CLAUDE.md's **NO BACKWARDS COMPATIBILITY / no zombie code**
  rule this commentary is part of the requirement and must move with it.
- `Cargo.toml:114` — `haematite = "0.8.3"`
- `Cargo.toml:118` — `liminal = { package = "liminal-rs", version = "0.5.0" }`
- `Cargo.toml:119` — `liminal-sdk = "0.5.0"`
- `Cargo.toml:120` — `liminal-server = "0.5.0"`

### 1.2 Member crates that name beamr

- `crates/aion/Cargo.toml:25` — `beamr = { workspace = true }`
- `crates/aion/Cargo.toml:48` — `beamr = { workspace = true, features = ["test-support"] }` (dev)
- `crates/aion-awl/Cargo.toml:20` — `beamr = { workspace = true }`
- `crates/aion-awl-package/Cargo.toml:21` — `beamr.workspace = true`

`.meridian/workflows/stacked-dev/worker/crates/haematite/Cargo.toml:11`
(`beamr = "0.6.1"`) is a vendored copy under `.meridian/`, **not** a member of
the root workspace (`Cargo.toml` `members`), is absent from `Cargo.lock`, and is
therefore out of scope.

### 1.3 The resolved lock today

- `Cargo.lock:698-701` — `beamr 0.16.3`
- `Cargo.lock:731-734` — `beamr 0.17.1`
- `Cargo.lock:762-765` — `beamr 0.18.2`

Three copies of the VM in one graph today.

### 1.4 Box mix (measured 2026-08-18)

`gleam 1.18.1`, `OTP 27`, `rustc 1.97.1`, `cargo 1.97.1`, 16 cores. This is
exactly the mix #64 excluded from gates — the population that is this
migration's proof.

---

## 2. R1 — census of the transitive holders

`cargo tree -i beamr@<v> --workspace -e normal,build`, run against HEAD:

**`beamr 0.16.3`** ← `haematite 0.7.0` ← `liminal-rs 0.5.0` ← {`aion-server`,
`aion-worker`, `liminal-sdk 0.5.0`, `liminal-server 0.5.0`}; also directly
← `liminal-rs 0.5.0` and `liminal-server 0.5.0`.

**`beamr 0.17.1`** ← `haematite 0.8.3` ← {`aion-server`, `aion-store-haematite`}.

**`beamr 0.18.2`** ← `aion-awl` and `aion-rs` (the workspace requirement).

Holder classification:

| Holder | In this workspace? | 0.19-clean release exists? |
|---|---|---|
| `aion-awl`, `aion-rs` | yes — `Cargo.toml:113` | n/a, this is the bump |
| `haematite` | no (crates.io) | **yes** — 0.9.0 → `beamr ^0.19.0` |
| `liminal-rs` | no (crates.io) | **yes** — 0.6.0 → `beamr ^0.19.1`, `haematite =0.9.0` |
| `liminal-server` | no (crates.io) | **yes** — 0.9.0 → `beamr ^0.19.1`, `haematite =0.9.0`, `liminal ^0.6.0` |
| `liminal-sdk` | no (crates.io) | **NO — head 0.7.0 requires `liminal ^0.5.5`** |

Source: crates.io sparse index (`https://index.crates.io/...`), read 2026-08-18.
Full `liminal-sdk` 0.7.0 dependency row: `liminal ^0.5.5` (optional),
`liminal-protocol ^0.7.0`, `liminal-server ^0.8.0` (optional). Its feature table
(`liminal-sdk-0.7.0/Cargo.toml`) is:

```
default = ["std"]
std     = ["thiserror/std", "serde/std", "dep:liminal", "dep:tungstenite"]
```

`dep:liminal` — i.e. `liminal-rs ^0.5.5` — is pulled in by `std`, and `std` is
the default. `crates/aion-worker/Cargo.toml:48` states in its own words why aion
needs it: *"the default `std` feature carries the real TCP push client and the
canonical wire codec."*

---

## 3. R2 — the forced compile break, and the `type_chunk` decision

### 3.1 The break

`beamr 0.19.x` added a tenth field to `ParsedModule`
(`~/.cargo/registry/src/index.crates.io-*/beamr-0.19.2/src/loader/load.rs:40`):

```rust
/// Raw `Type` chunk bytes, carried VERBATIM for re-emission (#95).
pub type_chunk: Option<Vec<u8>>,
```

The workspace has **exactly one** `ParsedModule` struct literal —
`crates/aion-awl/src/mir/select/assemble.rs:59-69` — so exactly one E0063.
(`crates/aion/tests/awl_bc4_differential/abi.rs:105-109` only *decodes* one;
`inspect_analysis.rs`, `inspect_support.rs`, `inspect_expect.rs`, `tests.rs`,
`inspect_tests.rs` only take `&ParsedModule`. Adding a field breaks none of
them.)

### 3.2 The decision: `type_chunk: None`, argued from beamr's source

`None` is not a default of convenience here; it is the only truthful value, and
beamr 0.19.2's own source says so in three places.

1. **`type_chunk` is the raw `Type` chunk of a module that was *decoded*.**
   `load.rs:362` — `let type_chunk = find_chunk(&chunks, b"Type").map(<[u8]>::to_vec);`
   It is populated only by reading a `Type` chunk out of an input container.
   `assemble` has no input container: it *synthesises* a module from AWL MIR
   (`assemble.rs:25-73`), and its parts (`builder.rs:209-225`, `BuilderParts` =
   `atom_table`, `atoms`, `literals`, `imports`, `lambdas`) contain no type
   table and no OTP type information of any kind. There is nothing to carry.

2. **The field's only consumer is typed-register re-emission.** The doc comment
   at `load.rs:33-39` is explicit: *"Typed-register operands index into this
   chunk. beamr's runtime never reads the table, but the encoder must re-emit it
   or every emitted typed register dangles."* `encode_module`
   (`encode/container.rs:116, 157-159`) uses it for exactly two things: to set
   `AtomEncoder::type_chunk_present`, and to append a verbatim `Type` chunk.

3. **The AWL selector emits no typed registers, so the field is inert on this
   path — and beamr *refuses* rather than degrades if that ever changes.**
   `encode/compact.rs:106-133` guards the single operand writer:

   ```rust
   if !atoms.type_chunk_present {
       return Err(EncodeError::TypedRegisterWithoutTypeChunk { .. });
   }
   ```

   with the comment: *"a silent fallback wearing a success exit"* — precisely the
   thing it will not do. Measured in this tree: the selector constructs nine
   `Operand` variants (`Operand::X` ×76, `Unsigned` ×55, `Label` ×40, `Atom` ×33,
   `Y` ×25, `List` ×7, `Literal` ×6, `Integer` ×6) and **zero**
   `Operand::TypedRegister`. All three `Operand::TypedRegister` occurrences under
   `crates/aion-awl/src/mir/select/` are read-side match arms —
   `inspect_support.rs:351`, `:428`, `:442` — as is the one in
   `crates/aion/src/runtime/module.rs:224`. There is not one construction site
   in the workspace.

   So with `type_chunk: None` the guard is unreachable today, and if a future
   AWL lowering ever emitted a `#tr{}` operand it would produce a hard
   `EncodeError`, not a container that beamr reads and `beam_disasm` cannot —
   which is the *safer* of the two states, not the weaker one.

4. **A non-`None` value would be a fabrication.** Setting
   `type_chunk: Some(vec![])` would flip `type_chunk_present` to `true` and emit
   an empty `Type` chunk into every AWL-compiled module, disarming the guard
   while promising a table that exists but indexes nothing. `container.rs:126-133`
   records that `Type` is one of the chunks measured to be genuinely optional to
   `beam_disasm` (OTP 29), so emitting an empty one buys nothing and costs the
   refusal.

**Ruling: `type_chunk: None` at `assemble.rs:68`, with a comment recording (3) —
that the AWL emitter constructs no typed registers, so the field is absent
because there is no type table, not because one was dropped.** No `#[allow]`,
no blind default anywhere else.

### 3.3 The self-gate keeps working, unchanged

`assemble.rs:154-162` re-loads the emitted bytes through
`load_beam_chunks → resolve_imports → validate_module`. On 0.19.2 the round trip
still holds: no `Type` chunk in, no `Type` chunk out, `type_chunk: None` both
sides.

---

## 4. R3 — full 0.18.2 → 0.19.2 API census

Method: public-item signature extraction (`pub fn|struct|enum|trait|type|const|
static`) over both unpacked registry sources, plus a comment-stripped
field/variant diff of `loader/load.rs`, `loader/decode/`, `module.rs`,
`error.rs`, `process/mod.rs`, `native/mod.rs`, `atom/table.rs`,
`scheduler/mod.rs`. 2203 → 2222 public items.

### 4.1 The complete delta

| # | Change | Kind | aion sites | Action |
|---|---|---|---|---|
| 1 | `ParsedModule` + `pub type_chunk: Option<Vec<u8>>` (`load.rs:40`) | **breaking (E0063)** | `assemble.rs:59` | **fix** — §3 |
| 2 | `resolve_imports` → `(Vec<ResolvedImport>, _)` was `(Vec<Option<ResolvedImport>>, _)` (`load.rs:528-533`) | **breaking** | `assemble.rs:158` | **none** — see 4.2 |
| 3 | `validate_module(_, &[ResolvedImport])` was `&[Option<ResolvedImport>]` (`validate.rs:17-20`) | **breaking** | `assemble.rs:159` | **none** — see 4.2 |
| 4 | `ProcessContext::{dict_get_all, dict_erase_all, dict_get_keys}` **removed**; replaced by `dict_entries_to_list`, `dict_erase_all_to_list`, `dict_keys_for_value_to_list` (`native/context/mod.rs`) | **breaking** | **zero** | none |
| 5 | `native::TermAccumulator` + `ProcessContext::with_accumulator` and 9 methods (`native/context/accumulator.rs`) | additive | zero | none |
| 6 | `Process::{gc_attempts, gc_completions, nested_handler_floor, set_nested_handler_floor, pop_exception_handler_in_nest}` (`process/mod.rs`) | additive | zero | none |
| 7 | `scheduler::{ProcessVmCensus, process_vm_census}` (`scheduler/inventory.rs`) | additive | zero | none |
| 8 | `AtomTable::with_common_atoms()` now delegates to `new()` (`atom/table.rs:186-207`) | behavioural, signature identical | `abi.rs:106`, harnesses | none |
| 9 | `impl From<ReplayMismatch> for ExecError` no longer `#[cfg(any(threads, cooperative))]` (`error.rs:311`) | additive under aion's features | zero | none |
| 10 | **validator #64 fix**: `current_frame_size` is cleared by control-flow terminators and updated by `Trim` (`validate.rs:222-241`), + regression test `shared_failure_island_after_foreign_tail_is_not_checked_against_stale_frame` (`validate.rs:549-575`) | behavioural — **the payoff** | `runtime_codecs` population | measure |
| 11 | `src/ar1_ordering_control.rs` added; `Instruction`/`Operand`/`ExitReason`/`ResolvedImport`/`Module` all byte-identical (`loader/decode/` dir diff: **empty**) | none | — | none |

**Feature names are unchanged**: `json`, `encode`, `test-support`, `readiness`
all still exist in `beamr-0.19.2/Cargo.toml` (`:81` `test-support = []`), so
`Cargo.toml:113` and `crates/aion/Cargo.toml:48` need no feature edits.

### 4.2 Why deltas 2 and 3 need no aion change

They moved in lockstep and aion's only site is a pass-through:

```rust
// crates/aion-awl/src/mir/select/assemble.rs:158-159
let (resolved, _report) = resolve_imports(&parsed, &registry, &NoBifs, &AllCapabilitiesPolicy);
validate_module(&parsed, &resolved)
```

`resolved` is never destructured, indexed, or `.flatten()`ed here — its element
type is inferred and handed straight on. `Vec<Option<ResolvedImport>>` →
`Vec<ResolvedImport>` is invisible at this site. Recorded because the census must
record it, not because it needs work.

`Module::resolved_imports` was **already** `Vec<ResolvedImport>` in 0.18.2
(`module.rs:130`, identical in both), so the three test sites that push into it —
`crates/aion/src/runtime/handle/tests.rs:92`,
`crates/aion/src/runtime/handle/test_support.rs:94`,
`crates/aion/src/runtime/nif_activity_delivery_tests.rs:340` — are unaffected.

### 4.3 Census scope caveat, for the builder to close

The signature diff above covers **items**, and the field/variant diff covers the
ten hot modules listed. It does not mechanically cover every struct field and
enum variant in beamr's whole surface. Every beamr path aion actually names is
enumerated in §4.4; the builder confirms the census the only way that counts —
a clean `cargo build`/`cargo test` of the crates that link 0.19.2.

### 4.4 The beamr surface aion actually uses (62 distinct paths)

`beamr::term::Term` (31), `beamr::atom::Atom` (27), `beamr::native::ProcessContext`
(23), `beamr::atom::AtomTable` (14), `beamr::term::binary_ref::BinaryRef` (12),
`beamr::loader::decode::*` (11), `beamr::process::ExitReason` (10),
`beamr::term::boxed::Tuple` (6), `beamr::loader::load::ParsedModule` (6),
`beamr::scheduler::*` (4), `beamr::module::ModuleOrigin::Preloaded` (4),
`beamr::loader::load_beam_chunks` (4), `beamr::loader::decode::compact::Operand`
(4), `beamr::constant_pool::ConstantPool::new` (4),
`beamr::loader::encode::encode_module` (3), `beamr::loader::Instruction` (3),
`beamr::native::NativeEntry` (3), `beamr::term::format::format_term` (2),
`beamr::scheduler::EXIT_EVENT_CAPACITY` (2), `beamr::native::NativeFn` (2),
`beamr::module::{ModuleRegistry, Module}` (2 each), `beamr::ets::OwnedTerm` (2),
and singles: `beamr::term::pid_ref::PidRef`, `beamr::term::boxed::{Cons, Closure}`,
`beamr::term::binary::Binary`, `beamr::scheduler::dirty::DirtySchedulerKind::Cpu`,
`beamr::scheduler::OwnedException`,
`beamr::native::stdlib_stubs::maps_bifs::ContinuationStep`,
`beamr::native::process_bifs`, `beamr::native::Capability::ExternalIo`,
`beamr::native::BifRegistryImpl`, `beamr::module::ResolvedImportTarget`,
`beamr::loader::validate::validate_module`, `beamr::loader::load_module_with_origin`,
`beamr::loader::encode::EncodeError`, `beamr::error::ExecError`,
`beamr::constant_pool::materialise_literals`.

Spread over 112 files (`crates/aion/src/runtime/**` is the boundary module per
CLAUDE.md; `crates/aion-awl/src/mir/select/**` is the emitter). Cross-referenced
against the delta table: **only `ParsedModule` is hit.**

---

## 5. R5 — the payoff, and why it is measurable despite the wall

`aion-awl` depends on `beamr` (`crates/aion-awl/Cargo.toml:20`) and on **no**
liminal crate. `cargo tree -i beamr@0.18.2` puts `aion-awl` on the workspace
requirement directly. So with `Cargo.toml:113` raised to 0.19.2,
`cargo test -p aion-awl --test runtime_codecs` compiles and runs against
**0.19.2 only**, whatever `aion-server`'s liminal subtree resolves to.

This is a lock-witnessed measurement, not a patch-line claim — the trap the
issue records. The witness is `cargo tree -i beamr@0.19.2` showing `aion-awl`
under it, taken in the same tree as the test run.

Target population: `crates/aion-awl/tests/runtime_codecs.rs` and its ten
submodules, including the named #64 hanger
`crates/aion-awl/tests/runtime_codecs/stmt_parity.rs:177`
`child_call_statement_and_spawn_execute_with_reference_parity`. The harness
shells out to a real `gleam build`
(`crates/aion-awl/tests/runtime_codecs/harness.rs`) and loads the produced beams
through `load_module_with_origin` (`harness.rs:393, 404`) — i.e. through the
validator that delta 10 fixed, on gleam 1.18.1 / OTP 27.

---

## 6. R4 — the lock witness, and why it cannot go clean here

### 6.1 Reproduced mechanically

In a scratch copy of HEAD (`git archive HEAD` → `/tmp/wf70`, this tree
untouched):

**Step 1** — `Cargo.toml:113` → `"0.19.2"`, resolve:
```
beamr rows: 0.16.3, 0.17.1, 0.19.2      ← two pre-floor rows survive
```

**Step 2** — plus `haematite = "0.9.0"`, `liminal = 0.6.0`,
`liminal-sdk = "0.7.0"`, `liminal-server = "0.9.0"` (every estate crate at its
0.19-clean head), resolve:
```
beamr rows:      0.16.3, 0.17.1, 0.19.2   ← unchanged
haematite rows:  0.8.3, 0.9.0             ← duplicated
liminal-rs rows: 0.5.5, 0.6.0             ← duplicated
```
```
$ cargo tree -i beamr@0.16.3
beamr v0.16.3
└── liminal-rs v0.5.5
    └── liminal-sdk v0.7.0
        ├── aion-server  └── aion-cli
        └── aion-worker

$ cargo tree -i beamr@0.17.1
beamr v0.17.1
└── haematite v0.8.3
    └── liminal-rs v0.5.5
        └── liminal-sdk v0.7.0 → aion-server, aion-worker
```

**Step 3** — control, to prove the single cause: `liminal-sdk` with
`default-features = false`, resolve:
```
beamr rows:      0.19.2      ← CLEAN
liminal-rs rows: 0.6.0       ← CLEAN
```

One edge, `liminal-sdk`'s `std → dep:liminal ^0.5.5`, holds the whole thing.
`default-features = false` is not a fix: it drops the TCP push client and wire
codec aion links (`crates/aion-worker/Cargo.toml:48`).

### 6.2 It is a type wall, not only a lock wall

`crates/aion-worker/src/runtime/liminal.rs` imports both sides:

- `:63` — `use liminal::protocol::WorkerRegistration;` (workspace `liminal`)
- `:64` — `use liminal_sdk::{PushClient, PushWriter, PushedFrame};`
- `:1430-1436` — `fn registration_from(..) -> WorkerRegistration { WorkerRegistration { .. } }`
- `:481` — `PushClient::connect_with_registration(address, registration)`

Move the workspace `liminal` to 0.6.0 while `liminal-sdk` links 0.5.5 and that
call is `E0308`: two distinct `WorkerRegistration` types from two semver-
incompatible copies of the same crate. `crates/aion-server/src/worker/
liminal_contract_tests.rs:47-48` builds the same type on the server side.

### 6.3 It is a VM-duplication hazard, not a cosmetic one

`aion-cli` links `aion-awl` (beamr 0.19.2) **and** `liminal-sdk` (beamr 0.16.3):
two BEAM implementations, two sets of process/atom/scheduler statics, in one
`aion` binary. `liminal`'s own manifest names this class in its most recent
dependency commit — *"two disjoint beamr ranges never share a tree"*
(`/home/aion/stack/liminal/Cargo.toml:55-63`, and commit `aa466b7` on that
checkout: *"deps: beamr 0.16.3 → 0.17.1 + haematite =0.8.1 → =0.8.3, ONE MOTION"*).
This is why "land the bump anyway and clean the lock later" is not on the table.

### 6.4 The unblock, stated precisely

**Upstream ask (ablative-io/liminal):** publish `liminal-sdk 0.8.0` with
`liminal ^0.6.0`, `liminal-protocol ^0.7.0`, `liminal-server ^0.9.0`.
`liminal-rs 0.6.0` and `liminal-server 0.9.0` are already on crates.io at
`beamr ^0.19.1` + `haematite =0.9.0`; `liminal-sdk` is the only crate in the
chain that has not followed. Nothing in aion can substitute for it.

Cheap encouragement for the ask: the API diff `liminal-sdk 0.5.0 → 0.7.0` shows
**zero removed or changed public item signatures** (275 → 288 items), and
`haematite 0.8.3 → 0.9.0` likewise shows **zero** (1001 → 1001) — haematite's
0.9.0 is a pure beamr-floor bump. `liminal-rs 0.5.0 → 0.6.0`: zero removed, one
added. `liminal-server 0.5.0 → 0.9.0`: two changed
(`cluster/membership.rs poll_once`, `server/participant/dispatch.rs new`), and
aion names neither (`grep -rn "poll_once|ConnectionIncarnation|MembershipDelta"
crates/` → empty). So the consumer-side cost of the whole estate hop, once
`liminal-sdk` follows, is close to zero.

### 6.5 Gate consequence, stated so nobody is surprised

`.fleet-gate.sh:23-27` (leg 1, lock witness) **will fail** on a blocked outcome.
That is the gate working: it is reporting truthfully that the migration cannot
complete from inside this repository. A green gate is only available after the
upstream release in §6.4. Do not weaken `.fleet-gate.sh` to obtain one.

---

## 7. R6 — scope fence

In scope: `Cargo.toml` (the dependency block and its commentary), `Cargo.lock`,
and `crates/aion-awl/src/mir/select/assemble.rs`. Plus the leg report.

Out of scope and explicitly refused: dropping or feature-gating away
`liminal-transport` (a shipped default transport — `crates/aion-server/
Cargo.toml:166`, `crates/aion-cli/Cargo.toml:128`); reimplementing
`liminal-sdk`'s push client behind `default-features = false`; any
`[patch.crates-io]` (the issue's own recorded measurement trap); touching
`.fleet-gate.sh`; `#[allow]`/`#[expect]`/`#[ignore]`; historical records under
`gate-logs/` and `CHANGELOG.md`'s shipped sections.

---

## 8. Leg decomposition

One leg, `.fleet/legs/migrate.md`, because every part shares one working tree
and one decision gate. It carries a re-witness step first: if `liminal-sdk`
≥0.8.0 with `liminal ^0.6.0` has appeared on crates.io by build time, the leg
takes the full-migration path instead of the report path. Both paths are
specified in full.
