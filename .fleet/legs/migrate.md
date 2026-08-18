# Leg `migrate` — beamr ≥0.19.2 (issue #70)

**Read `.fleet/plan.md` first.** It carries the census, the `type_chunk`
argument, and the mechanical reproduction of the blocker. This brief is the
work order; the plan is the evidence it rests on.

Branch: `fleet-70`. Gate: `.fleet-gate.sh`.

---

## Standing rules (from `CLAUDE.md`, non-negotiable)

- No `#[allow]`, `#[expect]`, `#[deny]`, `#[ignore]`, `_var` renames, or
  `#[cfg(any())]`. If clippy fires, fix the code.
- No deleting, skipping, or weakening any test.
- No `.unwrap()` / `.expect()` in library code.
- No backwards-compat shims, no zombie code — when a version moves, its
  justifying commentary moves with it.
- No behavioural rewrites hitching a ride. Scope is `Cargo.toml`, `Cargo.lock`,
  `crates/aion-awl/src/mir/select/assemble.rs`, and the leg report. Nothing else.
- Do **not** modify `.fleet-gate.sh`. A failing gate leg is a result, not a bug
  in the gate.
- Do **not** add `[patch.crates-io]`. The issue records the exact trap: a patch
  whose version does not satisfy the dependent's requirement is dropped
  silently and the build proceeds on registry code.

Environment: `export HOME=/home/aion` before any cargo invocation (the gate does
the same at `.fleet-gate.sh:12`; cargo resolves `~/.cargo` and `~/.rustup`
through it). Box mix is gleam 1.18.1 / OTP 27 / rustc 1.97.1 — the #64-excluded
mix, which is the point.

---

## R0 — Re-witness the blocker before doing anything else

The plan's blocker was measured 2026-08-18. Re-check it, because one upstream
release changes which path you take.

```
curl -s https://index.crates.io/li/mi/liminal-sdk
```

For the highest non-yanked version, read its `deps` entry for `liminal`.

- **If the requirement is `^0.5.x`** (state as of writing: head is 0.7.0,
  `liminal ^0.5.5`) → **Path B**, §R6.
- **If a version exists whose `liminal` requirement admits 0.6.0** → **Path A**,
  §R5, using that version.

Record the raw index row you read, verbatim, in the report. Do not take the
plan's word for it and do not take a crates.io web page's word for it — the
index is the resolver's own input.

**AC-R0:** the report contains the index row for the head `liminal-sdk` version
and names the path taken.

---

## R1 — Raise the workspace beamr requirement

`Cargo.toml:113`:

```toml
beamr = { version = "0.18.2", features = ["json", "encode"] }
```
→
```toml
beamr = { version = "0.19.2", features = ["json", "encode"] }
```

`"0.19.2"` is a caret range with floor 0.19.2, which is what the estate floor
requires. Do not write `"=0.19.2"`.

Feature names are unchanged in 0.19.2 (`json`, `encode`, and `test-support` at
`beamr-0.19.2/Cargo.toml:81` all still exist), so `crates/aion/Cargo.toml:48`
(`features = ["test-support"]`), `crates/aion/Cargo.toml:25`,
`crates/aion-awl/Cargo.toml:20`, and `crates/aion-awl-package/Cargo.toml:21`
need no edit — they all inherit `workspace = true`.

**Also rewrite `Cargo.toml:87-112`.** That 26-line block justifies the 0.18.2
lineage hop by hop and now describes a version the workspace no longer requires;
under the no-zombie-code rule it must move with the requirement. The replacement
must say, in the block's own register:

- what 0.19.2 carries that 0.18.2 does not — the #64 validator fix: the frame
  check became control-flow-aware, so a shared failure island reached after a
  foreign tail call is no longer checked against a stale frame size
  (`beamr-0.19.2/src/loader/validate.rs:222-241`, regression test at `:549-575`);
- that this is the estate version floor (Artemis via Waffles, 2026-08-18:
  *"version floor beamr ≥0.19.2; OTP-27/gleam-1.18.1 proven"*);
- that 0.19 is a breaking bump and what it broke here (`ParsedModule.type_chunk`,
  and `resolve_imports`/`validate_module` dropping their `Option` in lockstep —
  see `.fleet/plan.md` §4);
- keep the load-bearing history the existing block records for the earlier hops
  (0.18.2 message delivery + frame naming, 0.18.1 RF-006 JIT GC rooting, 0.17.0
  JIT re-entry `NativeServices`, 0.16.2 REVIEW-23-07 C1+C2, 0.15.2 `make_fun`/
  `put_map` GC, 0.14.0 `loader/encode`). Those are why the caret floor is where
  it is; do not delete them, re-point them.
- keep the existing block's honesty clause: a green build proves we COMPILE on
  0.19.2 and says nothing about whether the long runs survive.

**AC-R1:** `grep -n beamr Cargo.toml` shows `version = "0.19.2"` and a
commentary block that names 0.19.2's content, not 0.18.2's, as the reason for
the floor.

---

## R2 — Fix `type_chunk`, and only `type_chunk`

`crates/aion-awl/src/mir/select/assemble.rs:59-69` is the **only**
`ParsedModule` struct literal in the workspace (verified: `grep -rn
"ParsedModule {" --include="*.rs" crates/ benchmarks/ gleam/` returns exactly
this one line). Add the field:

```rust
    let parsed = ParsedModule {
        name: module_atom,
        atoms: parts.atoms,
        instructions,
        imports: parts.imports,
        exports,
        lambdas: parts.lambdas,
        literals: parts.literals,
        string_table: Vec::new(),
        line_info: Vec::new(),
        type_chunk: None,
    };
```

`None` is the truthful value and must carry a comment saying **why**, not that
it is a default. The argument is set out in full at `.fleet/plan.md` §3.2; the
comment you write should compress it to its load-bearing half:

> The `Type` chunk is absent because there is no type table to carry, not
> because one was dropped. `type_chunk` is populated only by decoding a `Type`
> chunk out of an input container (`beamr` `loader/load.rs:362`); this assembler
> synthesises a module from AWL MIR and its `BuilderParts` (`builder.rs:219-225`)
> hold no type information at all. The field's sole consumer is typed-register
> re-emission (`loader/encode/container.rs:116, 157-159`), and the AWL emitter
> constructs no `Operand::TypedRegister` — every occurrence in this crate is a
> read-side match arm (`inspect_support.rs:351, 428, 442`). Should a future
> lowering ever emit one, beamr refuses with
> `EncodeError::TypedRegisterWithoutTypeChunk` (`loader/encode/compact.rs:127-132`)
> rather than writing a container only beamr can read — a hard error, which is
> the outcome we want.

**Before writing that comment, verify each of its claims yourself** against the
unpacked source at
`~/.cargo/registry/src/index.crates.io-*/beamr-0.19.2/src/loader/`. If any claim
does not hold, the decision is reopened, not the comment softened.

**Do not** set `Some(Vec::new())`. That flips `type_chunk_present` to `true`
(`container.rs:116`), disarms the encoder's refusal, and emits an empty `Type`
chunk promising a table that indexes nothing.

**Do not** touch `assemble.rs:158-159`. `resolve_imports`/`validate_module` both
dropped their `Option` in the same release and `resolved` is a pass-through
there — the site compiles unchanged. Confirm by compiling, not by editing.

**AC-R2:** `cargo build -p aion-awl` is clean; the diff to `assemble.rs` is one
field plus its comment; `grep -rn "allow(\|expect(\|ignore" ` over the diff is
empty.

---

## R3 — Complete the API census and record it

`.fleet/plan.md` §4 carries the census produced at planning time: eleven deltas,
one aion-visible, plus the 62-path list of the beamr surface aion actually uses.
Your job is to **confirm it, extend it where it is thin, and record it** —
including the entries where no change was needed (fix-spec item 3 requires that
explicitly).

Confirm by re-running, against 0.19.2 as resolved in your tree:

1. The item-signature diff (plan §4, method paragraph). Expect three
   removed/changed: `ProcessContext::dict_get_all`, `::dict_erase_all`,
   `::dict_get_keys`.
2. `grep -rn "dict_get_all\|dict_erase_all\|dict_get_keys" --include="*.rs"
   crates/ benchmarks/` → expect empty (those three are the only removals, and
   aion never called them).
3. `diff -rq beamr-0.18.2/src/loader/decode beamr-0.19.2/src/loader/decode` →
   expect empty (so `Instruction` and `Operand` are unchanged and no exhaustive
   match in `crates/aion/src/runtime/module.rs` or
   `crates/aion-awl/src/mir/select/inspect_support.rs` breaks).

Close the plan's stated gap (§4.3): the signature diff covers items, not every
struct field. The only way that counts is a compile of everything that links
0.19.2 — §R4.

**AC-R3:** the report contains the census as a table with an explicit
`no change needed` row for every delta that needed none, and the three
confirmation commands with their actual output.

---

## R4 — Measure the payoff (do this on BOTH paths)

This is reachable even when blocked, and it is the migration's proof. `aion-awl`
depends on `beamr` (`crates/aion-awl/Cargo.toml:20`) and on no liminal crate, so
it links 0.19.2 regardless of what `aion-server`'s subtree resolves to.

```
export HOME=/home/aion
cargo tree -i beamr@0.19.2 --workspace -e normal,build      # witness: aion-awl under it
cargo test -p aion-awl --test runtime_codecs
```

Take the `cargo tree` witness **in the same tree, in the same session, as the
test run** — the issue's struck negative exists precisely because a version
claim was made at a manifest line instead of at the resolved graph.

Expected: 28/28 green on gleam 1.18.1 / OTP 27, including the named #64 hanger
`child_call_statement_and_spawn_execute_with_reference_parity`
(`crates/aion-awl/tests/runtime_codecs/stmt_parity.rs:177`). The harness shells
out to real `gleam build` and loads the beams through `load_module_with_origin`
(`crates/aion-awl/tests/runtime_codecs/harness.rs:393, 404`) — i.e. through the
validator that 0.19.2 fixed.

Report the **actual** result with wall-clock and pass/fail counts. If it is not
green, that is a finding of the first order — report it in full, with the
failing test names and output, and do not proceed to R5/R6 as if it were.

Also run, unconditionally:
```
cargo clippy -p aion-awl --all-targets -- -D warnings
cargo fmt --check
```

**AC-R4:** the report carries the `cargo tree -i beamr@0.19.2` output, the
`runtime_codecs` result verbatim, and both lint results.

---

## R5 — Path A: full migration (only if R0 cleared the wall)

Bump the estate chain in `Cargo.toml` in **one motion** — partial bumps split
the graph into duplicate copies (the failure class `liminal`'s own manifest
names):

- `:114` `haematite = "0.8.3"` → `"0.9.0"` (declares `beamr ^0.19.0`)
- `:118` `liminal = { package = "liminal-rs", version = "0.5.0" }` → `"0.6.0"`
  (declares `beamr ^0.19.1`, `haematite =0.9.0`)
- `:119` `liminal-sdk = "0.5.0"` → the version R0 found
- `:120` `liminal-server = "0.5.0"` → `"0.9.0"` (declares `beamr ^0.19.1`,
  `haematite =0.9.0`, `liminal ^0.6.0`)

Then resolve and witness:

```
cargo metadata --format-version 1 >/dev/null       # re-resolves the lock
grep -A1 '^name = "beamr"' Cargo.lock | grep version
grep -A1 -E '^name = "(haematite|liminal-rs)"' Cargo.lock | grep -E 'name|version'
```

Required: exactly one `beamr` row, `0.19.2` or higher; exactly one `haematite`
row; exactly one `liminal-rs` row. Duplicate rows of the last two mean the
motion was partial — fix the manifest, do not proceed.

Then the consumer-side migration. Planning measured the estate API cost as near
zero (plan §6.4: haematite 0.8.3→0.9.0 and liminal-sdk 0.5.0→0.7.0 remove or
change **zero** public item signatures; liminal-rs 0.5.0→0.6.0 removes none;
liminal-server 0.5.0→0.9.0 changes two that aion does not name). Confirm by
compiling, and fix whatever the compiler actually finds — properly, at the call
site, with no allow-attributes. The surfaces to expect breakage in, if any:

- `haematite::` — 12 distinct paths, hottest `NodeCacheBudget`,
  `HaematiteStore`, `EventStore`, `sync::membership::WriteMembership`,
  `db::respond_to_inbound_writes`, `DatabaseError`, `ClusterResponder`,
  `OutboxRowState`. Consumers: `crates/aion-store-haematite/`,
  `crates/aion-server/`, `crates/aion/`.
- `liminal_server::` — `server::listener::ServerListener`,
  `server::connection::{ConnectionSupervisor, LiminalConnectionServices, PushReplyAwaiter}`,
  `config::{ServicesConfig, LimitsConfig}`. Consumer: `crates/aion-server/`.
- `liminal_sdk::{PushClient, SdkError}` and
  `liminal::protocol::{WorkerRegistration, WorkerActivityDescriptor}` —
  `crates/aion-worker/src/runtime/liminal.rs:63-64, 481, 1430-1436`,
  `crates/aion-server/src/worker/liminal_contract_tests.rs:47-48`.

Then the full gate:
```
sh .fleet-gate.sh
```
All three legs green — lock witness, `cargo test --workspace` with **no** gleam
exclusions, `cargo clippy --workspace --all-targets -- -D warnings`.

Add a `## Unreleased` entry to `CHANGELOG.md` (the section exists at `:6`)
recording the beamr floor move and the #64 exclusion coming off gates. Do not
edit any shipped release section.

Commit: manifest + lock + `assemble.rs` + `CHANGELOG.md` + the report.

**AC-R5:** `.fleet-gate.sh` exits 0; the lock has exactly one beamr row at
≥0.19.2; the diff touches only the files named in the scope fence plus
`CHANGELOG.md`.

---

## R6 — Path B: blocked (the expected path as of 2026-08-18)

### R6.1 Prove the wall in your own tree, not from the plan

Work in a scratch copy so this tree stays clean:

```
export HOME=/home/aion
rm -rf /tmp/wf70 && mkdir -p /tmp/wf70
git archive HEAD | tar x -C /tmp/wf70
cd /tmp/wf70
```

Run all three steps and capture each output:

1. beamr → 0.19.2 alone; resolve; list beamr rows.
   Expect `0.16.3, 0.17.1, 0.19.2`.
2. plus haematite 0.9.0 / liminal 0.6.0 / liminal-sdk 0.7.0 /
   liminal-server 0.9.0; resolve; list beamr, haematite, liminal-rs rows.
   Expect beamr unchanged, haematite and liminal-rs **duplicated**.
   Then `cargo tree -i beamr@0.16.3` and `cargo tree -i beamr@0.17.1` for the
   holder chains.
3. control — same as 2 but `liminal-sdk = { version = "0.7.0",
   default-features = false }`; resolve; list beamr and liminal-rs rows.
   Expect **clean**: `0.19.2` and `0.6.0` only.

Step 3 is what isolates the cause to one edge. Include it; a blocker report
without the control is an assertion.

### R6.2 Leave this tree unmodified

Revert everything R1/R2 changed in the working tree. **No half-migration
ships**, and here that is not a style preference: `aion-cli` links `aion-awl`
(beamr 0.19.2) and `liminal-sdk` (beamr 0.16.3), which puts two BEAM
implementations — two sets of process, atom, and scheduler statics — in one
`aion` binary. See `.fleet/plan.md` §6.3.

`git status` must show only the report file as added.

### R6.3 Write the report

`.fleet/reports/migrate.md`, carrying:

1. **Verdict** — one paragraph: what is reachable, what is not, and why.
2. **R0 index row**, verbatim.
3. **The census** (R3), full table, `no change needed` rows included.
4. **The `type_chunk` decision** (R2) with the argument from beamr source, and
   the exact patch you validated in the scratch tree. The decision must be on
   record even though the patch does not land — it is the answer to fix-spec
   item 2, and it is what makes the next attempt a five-minute job.
5. **The payoff measurement** (R4) with the lock witness. State plainly that
   this is measured on the box mix that #64 excluded.
6. **The blocker**, with all three R6.1 reproductions and the two holder chains.
7. **The type wall** — `crates/aion-worker/src/runtime/liminal.rs:63, 64, 481,
   1430-1436` — showing that even setting the lock aside, a split `liminal`
   makes `PushClient::connect_with_registration` an `E0308`.
8. **The unblock, addressed to ablative-io/liminal**: publish `liminal-sdk`
   ≥0.8.0 with `liminal ^0.6.0`, `liminal-protocol ^0.7.0`,
   `liminal-server ^0.9.0`. Note that `liminal-rs 0.6.0` and
   `liminal-server 0.9.0` are already published on `beamr ^0.19.1`, that
   `liminal-sdk 0.5.0→0.7.0` changed zero public item signatures, and that
   haematite's own 0.9.0 is a pure floor bump (zero signature changes) — so the
   remaining hop is small.
9. **Gate consequence** — `.fleet-gate.sh:23-27` will fail leg 1. Say so
   explicitly, say it is the gate working, and say the gate was not touched.
10. **Issue #70 acceptance, item by item** — 1 blocked, 2 blocked, 3 blocked
    (the #64 exclusion cannot come off gates until the workspace actually runs
    ≥0.19.2), 4 blocked. With the note that acceptance item 3's *evidence* is
    already in hand from R4: the validator does accept this box's beams.

**AC-R6:** `git status` shows exactly one added file; the report contains all
ten sections; every reproduction in it has its command and its actual output.

---

## R7 — Report the outcome faithfully

Whichever path ran: state what was done, what passed, what failed, and what was
left undone and why. If `runtime_codecs` was not green, lead with that. Do not
describe a blocked migration as a completed one, and do not describe the
reachable half as "the migration" — the deliverable was the lock witness, and on
Path B it was not obtained.

Commit message on Path B:
```
fleet(#70): beamr 0.19.2 migration BLOCKED at liminal-sdk — census, payoff, and the wall
```
