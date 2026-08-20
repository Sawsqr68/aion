# Leg `fix` — the teardown writer-lock regression (rb-lockfix)

**Run:** `rb-lockfix-flight1`. **This is the run's ONLY leg.** It owns the entire delivery.
**Tree:** aion at rig commit `65f0bcac4`; base of the rig = `b72a5c35` (the repo-battery-flight1
leg tree whose gate went red at round 3). The base that tree was cut from is `856b2a863`.
**Fleet brief:** `.fleet/BRIEF.md` — the authority. Every one of its requirements is restated in
full below, by name.
**Plan:** `.fleet/plan.md` — the diagnosis plan, with the planning counsel's measurements. This
brief is self-contained; the plan carries the anchor ledger and the verbatim result lines.
**Gate entry:** `.fleet/gate-entry.sh` — already committed, **not yours to edit**.
**Your report:** `.fleet/reports/lockfix.md` (create it). `.fleet/reports/build.md`,
`capture.md` and `fix-rounds.md` are prior runs' artifacts — read `fix-rounds.md`, it is the leg's
own account of why the code you are looking at changed; do not edit any of them.

---

## 0. Read this first — five facts that will save you a day

1. **The brief names a mechanism that measurement refutes.** `.fleet/BRIEF.md:25-31` proposes that
   this tree "makes engine/server-state teardown return before the store writer lock is actually
   released". The planning counsel measured that hypothesis to destruction (§2). The brief
   anticipated this and licences you explicitly: *"Follow the evidence if it points elsewhere; do
   not follow this brief off a cliff"* (`:30-31`), and R2's second branch (`:43-45`). **Re-measure
   before you accept either the brief's mechanism or the counsel's.** §3 is that re-measurement and
   it is a requirement, not a suggestion.
2. **The failing test builds no engine.** `state_with_leaf` →
   `ServerState::from_parts_with_namespace_store` (`crates/aion-server/src/state.rs:343-359`) →
   `from_parts_with_control_stores` (`:361-371`), whose own doc says it retains the control-plane
   contracts *"without constructing the full engine boot"*. No `Engine`, therefore no
   `Engine::drop`, in the test that fails.
3. **haematite never waits for the writer lock.** It is an advisory `flock` acquired with a single
   `try_lock` (`haematite-0.9.0/src/db/lock.rs:1-27`, `:166`), owned by the **open file
   description**, released when the last handle to that description closes. The brief's line "at
   base the second server state WAITS for the first state teardown to release the writer lock"
   (`.fleet/BRIEF.md:19-20`) is mechanically impossible. At base the reopen succeeded on its first
   and only attempt because ~60 s had elapsed.
4. **The leg diff has a second half the brief's diff command never shows.**
   `git diff 856b2a863..b72a5c35 -- crates/aion` is one commit (`0f3aff9d`) and one behavioural
   line. The rest of the causal surface is `Cargo.toml:125-131` — `[profile.dev]`/`[profile.test]`
   `debug = 0` (commit `70adba5d`, `.fleet/reports/fix-rounds.md:3-9`).
5. **You may not touch the live estate.** No servers, no listeners, no databases outside your own
   tempdirs, nothing outside this working tree.

---

## 1. The diff you were told to read, read

`git diff 856b2a863..b72a5c35 -- crates/aion`: 6 files, +1477/−1453, **one commit** —
`0f3aff9d fix(gate): release engine task descriptors on drop`. Its stated purpose
(`.fleet/reports/fix-rounds.md:11-17`): the `aion-server` lib target was exhausting the process
file-descriptor limit (`failed to start the engine-task runtime: Too many open files (os error
24)`) because `Engine::drop` left every released engine's Tokio runtime and I/O driver owned for
the life of the test process.

The **only** behavioural line in the whole diff:

```
crates/aion/src/engine/api.rs:323   (inside `impl Drop for Engine`, :318-324)
-        self.runtime.engine_tasks().begin_close();
+        self.runtime.engine_tasks().shutdown();
```

Everything else is (a) the doc rewrite around that line — `api.rs:71-83`, `:110-115`, `:131-146`,
`:158-161` — (b) one added `#[cfg(test)]` accessor,
`EngineTaskRuntime::owns_runtime` (`crates/aion/src/runtime/engine_tasks.rs:442-449`), (c) one
added regression test, `engine_drop_releases_the_descriptor_owning_task_runtime`
(`crates/aion/src/engine/api/api_tests/shutdown.rs:339-356`), and (d) test relocation out of
`api.rs` and `engine_tasks.rs` into `api/api_tests{,/lifecycle.rs,/shutdown.rs}` and
`engine_tasks/tests.rs`.

The bodies of `EngineTaskRuntime::shutdown` (`engine_tasks.rs:451-476`), `gate_and_abort`
(`:500-514`), `begin_close` (`:516-526`), `shutdown_runtime_and_join` (`:543-573`) and
`Drop for EngineTaskRuntime` (`:575-628`) are **unchanged**. Note the direction of the one line
that did change: `shutdown()` gates, aborts, and then **blocks on a joiner thread** until the owned
runtime's drop has cancelled every task and waited out every in-flight poll; `begin_close()` gates,
aborts and returns. The change adds waiting to teardown. The brief's hypothesis predicts the
opposite sign.

---

## 2. The measured record (planning counsel, this tree, this box)

Re-measure it. The verbatim result lines are in `.fleet/plan.md:§8`.

| # | Experiment | Result |
|---|---|---|
| 1 | Full `aion-server` lib suite, clean tree, ×2 | `1155 passed; 1 failed` in 23.31 s and 22.59 s — `http_put_survives_a_second_server_state_over_the_same_store`, `DataDirLocked` on its own tempdir |
| 2 | `api.rs:323` reverted to `begin_close()`, full rebuild, full suite | **Still `1155 passed; 1 failed`, 22.60 s.** The engine line is not the trigger and removing it is not a cure |
| 3 | Probe at the drop site: `Arc::strong_count(&first_store)`; `Arc::into_inner`; `/proc/self/fd` scan for the lockfile | `strong_count = 1`; `was_last_reference = true`; **no descriptor in this process points at the lockfile**. The teardown released everything it owns, `Database::drop` (synchronous, `haematite-0.9.0/src/db.rs:541-559`) had already run |
| 4 | Probe scanning `/proc/*/fd` on each failed `try_lock` | Holder = **pid 2849898, fd 566, `PPid` = the test process, `cmdline` empty, `comm` `worker::workspa`, `Threads: 1`** — a cloned, not-yet-`exec`ed child of `worker::workspace_root::tests::every_accepted_printable_ascii_root_survives_the_real_parser` (`crates/aion-server/src/worker/workspace_root.rs:772-810`, 95 shell spawns in a loop). Lock free after **92.9 / 147.4 / 150.0 ms** |
| 5 | Same probe, but waiting for the lock before reopening | Full suite **`1156 passed; 0 failed`**, twice (22.10 s, 22.54 s) |
| 6 | The two tests alone, `--test-threads=2` | Green, 9.45 s — in a small process the clone→`exec` window is microseconds |
| 7 | `capture_uses_build_identity_and_running_executable_sha256` in isolation; test binary size | **18.64 s**; **188 MiB**. The PUT handler runs the same `std::fs::read` + `Sha256` over the whole running binary on **every** PUT (`crates/aion-server/src/api/http/worker_deployments.rs:70` → `:192-208`) |

**The causal chain those measurements support**

1. The failing test opens a haematite store, PUTs once, drops everything, reopens the same dir
   (`crates/aion-server/src/api/http/worker_deployments_tests.rs:127-178`).
2. The PUT hashes the entire test binary. At base that binary carried debuginfo, so the hash cost
   60+ s (`.fleet/BRIEF.md:18-19` records the over-60s notice and the 125 s suite). `Cargo.toml:125-131`
   (`debug = 0`) shrank it to 188 MiB ⇒ 18.64 s, so the reopen moved from ~60 s into the run to
   ~19 s into a 22 s run.
3. ~19 s in is the middle of the `workspace_root` / `worker::supervisor` phase, which is spawning
   child processes in bursts (`crates/aion-worker/src/shell/action.rs:167` →
   `crates/aion-worker/src/process/contained.rs:373-388`).
4. Every spawn clones the parent's descriptor table. `flock` is owned per **open file description**,
   so the child holds the data-dir writer lock until it `exec`s (the descriptor is `CLOEXEC`; `exec`
   is what frees it) or exits. In a 6757-thread, 188 MiB process under 16-way load that window
   measured **93–150 ms**.
5. The reopen's single `try_lock` (`haematite-0.9.0/src/db/lock.rs:166`) lands inside that window
   and returns `DataDirLocked`.

**The diagnostic signature was real and its reading was inverted.** The test did get faster when it
started failing — but the wait that vanished was not an awaited completion. It was `std::fs::read`
plus `Sha256` over a debuginfo build. The wait was camouflage, not correctness.

---

## 3. Your requirements

### R1 — RED FIRST *(from `.fleet/BRIEF.md:35-38`, restated in full)*

> Reproduce at this tree: run the full aion-server lib suite (nofile raised; the gate script shows
> how), commit the failing output verbatim as red evidence in its own commit. The isolation-green
> observation is part of the record; note it.

```
ulimit -n 65536
cargo test -p aion-server --lib                     # expect 1155 passed; 1 failed
cargo test -p aion-server --lib -- --exact \
  api::http::worker_deployments::tests::http_put_survives_a_second_server_state_over_the_same_store
```

Commit the failing output **verbatim**, in a commit of its own, before any source change. Note the
isolation result beside it.

### R2 — DIAGNOSE AT SOURCE *(from `.fleet/BRIEF.md:40-45`, restated in full)*

> Name the exact change in the engine/api or engine_tasks diff
> (`git diff 856b2a863..b72a5c35 -- crates/aion`) that alters teardown/lock-release ordering, and
> write the causal chain into the report. If you conclude the TEST model is wrong instead, prove it
> against base behavior (base waits and passes — that is the contract as currently tested) and
> carry the proof.

The exact change is named in §1: `crates/aion/src/engine/api.rs:323`, `begin_close()` →
`shutdown()`. You must then **measure whether it carries the failure**, not assume it. Minimum
evidence set, each run at your hands and recorded in the report:

- **E1** — revert `api.rs:323` to `begin_close()` in place, rebuild, run the full lib suite.
  Restore the file with `git checkout --` afterwards. (Counsel: still red, 22.60 s.)
- **E2** — instrument the drop site of the failing test
  (`crates/aion-server/src/api/http/worker_deployments_tests.rs:155-158`): print
  `Arc::strong_count(&first_store)`, take `Arc::into_inner(first_store)`, and walk `/proc/self/fd`
  for a readlink equal to `<data_dir>/writer.lock`. (Counsel: 1, `true`, no match.)
- **E3** — on each failed acquisition, scan `/proc/*/fd` for that lockfile and print the holder's
  `cmdline` and `status` (`Name`, `State`, `PPid`, `Threads`). Keep scanning until the holder has a
  name. (Counsel: a cloned, pre-`exec` child of the workspace-root fork storm; lock free in
  93–150 ms.) **Beware:** the scan will also list *your own probe's* open handle on the lockfile
  under `self` / `thread-self` / your pid. That is not a leak. Do not report it as one.

Diagnostic instrumentation is throwaway. Either revert it before the commit, or commit it in an
explicitly-labelled diagnostic-evidence commit that a later commit reverts — never leave it in the
tree you hand to the gate.

The brief's parenthetical "base waits and passes — that is the contract as currently tested" is
**wrong on the mechanism** and your proof must say so with the anchor: `haematite-0.9.0/src/db/lock.rs:166`
is a bare `try_lock`; there is no wait on any path. What is true is the *contract*: at base the
reopen succeeds, and it must keep succeeding. That contract is what you preserve.

### R3 — FIX AT SOURCE *(from `.fleet/BRIEF.md:47-55`, restated in full)*

> Restore the teardown contract: teardown-complete must not report before the store writer lock is
> released. Do NOT revert the leg wholesale — its battery/workflow work is wanted; cure the
> ordering. Walls, absolute: no test deletion, no assertion weakening, no timeout inflation, no
> added sleeps as synchronization, no lint suppressions in any spelling, no ignore attributes, no
> file over 500 lines of code, mod.rs re-exports only, thiserror for library errors, never
> `.unwrap()`/`.expect()` in library code, red evidence as separate commits.

**The first sentence is under a typed refusal from the planning counsel** (`.fleet/plan.md:§4`):
there is no teardown that reports complete before releasing the lock — measured at E1/E2/E3 above.
Every other clause of R3 binds you unchanged: fix at source, do not revert the leg wholesale, and
every wall in that list.

**Ruling gate.** After R2:

- **If your measurements confirm §2** — adopt the refusal, say so in the report with your own
  evidence, and build the cure in §4.
- **If your measurements contradict §2** — if reverting `api.rs:323` turns the suite green at your
  hands, or `strong_count > 1`, or the holder is inside this process — then the counsel is wrong.
  Abandon §4, cure the ordering as the brief anticipated, and carry the contradicting measurement
  in the report. **The plan does not get to be right; the measurement does.**

### R4 — PROVE *(from `.fleet/BRIEF.md:57-61`, restated in full)*

> Full aion-server lib suite green TWICE consecutively, plus one full
> `cargo test --workspace --no-fail-fast` green, all at your hands, result lines committed in the
> report `.fleet/reports/lockfix.md`. The restored behavior may lawfully make the test slow again
> (~60s+) — slow and correct beats fast and lying.

Plus, because the gate runs them and nothing lands without them:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 4. The cure (the confirming branch of the ruling gate)

### 4.1 The property

A store reopen must survive a **transient** holder of the data-dir lock — a descriptor copy held by
a concurrently-cloned child for the length of its clone→`exec` window — while still failing loudly
against a **genuine** second live writer, which is what the lock exists for
(`haematite-0.9.0/src/db/lock.rs:1-27`).

That is one thing: the acquisition gets a **ruled patience**. Re-attempt until the acquire succeeds
or the ruled span elapses, then return the same typed error, unchanged.

### 4.2 Where

`crates/aion-store-haematite/src/store.rs`:

- `HaematiteStore::open` — `:337-345`. Production boot call site:
  `crates/aion-server/src/state.rs:2281`.
- `HaematiteStore::open_or_create` — `:297-318` (routes into `open` when `config.json` exists).

`create` / `create_with_shard_count` are **out of scope**: a directory with no `config.json` has no
prior lock for a child to have inherited, so there is no transient holder to wait out. Say that in
the report rather than leaving it unexplained.

### 4.3 Shape

- The patience is a **required parameter**. No invented default, in code or in a config schema.
  The precedent is in the same file for the same reason — `store.rs:285-291`: *"`node_cache_budget`
  is REQUIRED and has no default here … the byte ceiling is the caller's deployment decision."*
- The retry cadence is part of the same ruling, carried in the same argument (one required type,
  both fields, named for what they are) so no call site can rule half of it.
- Retry **only** on `DatabaseError::DataDirLocked`. Every other error returns on the first attempt,
  unchanged, unwrapped, unswallowed.
- Retrying that one variant is safe by haematite's own construction, and the report must cite it:
  `haematite-0.9.0/src/db/startup.rs:105-108` (create takes the lock **before** `config.json` is
  written, and a lock-acquire failure deliberately skips the cleanup) and `:175` (open takes the
  lock, **then** starts shards). A failed acquire has started nothing and written nothing.
  `adopt_node_cache_budget`, which `open` runs first (`store.rs:342`), is idempotent by its own
  contract (`store.rs:320-331`).
- Production boot (`crates/aion-server/src/state.rs:2281`) rules the patience from the operator
  config surface, following whatever precedent that config crate already uses for storage knobs.
  **If no precedent-compliant home exists there, stop and write a typed refusal in the report**
  rather than inventing a default in a config schema.
- Tests rule it explicitly at their call sites. A test naming its own patience is explicit — and it
  is the opposite of a hidden sleep. Say so at the site, in a comment, so no reviewer reads it as
  wall 4.

### 4.4 Blast radius

`open` / `open_or_create` have **37 call sites across 19 files** outside the implementing module:
`crates/aion-server/src/{state.rs, cluster.rs, worker/outbox_dispatcher.rs,
observability/instrumented_store.rs, api/http/worker_deployments_tests.rs}`,
`crates/aion-server/tests/*` (×9), `crates/aion/tests/*` (×3),
`crates/aion-store-haematite/tests/*` (×2). Every one rules the patience. Several of them
(`deploy_restart_e2e.rs`, `namespace_restart.rs`, `outbox_transport_e2e.rs`,
`recovery_declared_body_e2e.rs`, `graceful_shutdown_park_e2e.rs`) are the same
reopen-after-close shape in the same process-spawning binaries — the same latent race, currently
unfired. The acquisition-boundary cure retires all of them at once.

### 4.5 The regression test

A test that fails without the cure and passes with it, at the acquisition boundary — not at the
symptom. The honest shape: hold the lock on `<data_dir>/writer.lock` through a **second descriptor**
while a reopen runs, release it inside the ruled patience, and assert the reopen acquires; then
assert that a holder outliving the patience still returns `DataDirLocked` unchanged. Both halves,
because the second is the invariant the first must not have eaten.

No `#[ignore]`, ever. If it genuinely needs a runtime the box may not have, gate it at runtime per
CLAUDE.md: read an env var, emit a `tracing::info!` skip line, return `Ok(())`.

### 4.6 Explicitly forbidden non-cures

| Tempting | Why not |
|---|---|
| Revert `api.rs:323` to `begin_close()` | Measured not to cure (E1); reinstates the descriptor leak `0f3aff9d` fixed (`.fleet/reports/fix-rounds.md:11-17`); R3 forbids reverting the leg's wanted work |
| Revert `Cargo.toml:125-131` (restore debuginfo) | Cures nothing — re-hides the race behind a 60-second sha256 — and re-breaks the round-1 disk failure (98.5 GiB `target/`, `.fleet/reports/fix-rounds.md:3-9`). R4's "slow and correct" licences a **real wait**, not restored camouflage |
| Narrow the clone→`exec` window at the spawn site | Unavailable: `crates/aion-worker/src/process/contained.rs:373-388` already uses `std`'s `posix_spawn`-eligible path and `std` already sets `CLOEXEC`. A descriptor copy exists between clone and `exec` regardless — that is `fork` semantics, not a repo defect |
| Cache the executable hash in `capture_binary_identity` | Out of scope, and not a cure — it makes the reopen *earlier*. Record the observed cost (18.6 s per PUT in a dev profile; `worker_deployments.rs:192-208` re-reads and re-hashes the whole binary every call) and raise it separately |
| Touch the test's assertions, its tempdir shape, its ordering, or `#[ignore]` it | R3's walls 1, 2, 3, 4, 6. The assertions stay byte-identical; the only new thing the test may carry is the patience it hands the constructor, which is a real acquire |
| Retry with an inflated patience until it passes | Wall 3 (no timeout inflation) wearing a different hat. If the measured window and the ruled patience disagree, report the disagreement |

---

## 5. Walls — all of them, by name

From `.fleet/BRIEF.md:50-55` and `CLAUDE.md`. Every one binds every commit you make.

1. **No test deletion.**
2. **No assertion weakening.**
3. **No timeout inflation.**
4. **No added sleeps as synchronization.** A bounded wait on a real lock, which returns the instant
   the lock frees, is not a sleep — say so at the site so a reviewer does not have to guess.
5. **No lint suppressions in any spelling.** `#[allow(...)]`, `#[expect(...)]`, `#[deny(...)]`,
   `_var` renames and `#[cfg(any())]` dead-code hiding are bypasses, not fixes. If clippy fires,
   fix the code.
6. **No ignore attributes.** `#[ignore]` on a test is a bypass. Runtime gating per CLAUDE.md
   instead: env var + `tracing::info!` skip line + `Ok(())`.
7. **No file over 500 lines of code** (excluding tests, comments, whitespace). Break it into
   modules. For reference at this tree: `api.rs` 792 lines, `engine_tasks.rs` 631,
   `api_tests/lifecycle.rs` 466, `api_tests/shutdown.rs` 356, `engine_tasks/tests.rs` 504 — the
   leg already split these; keep them split.
8. **`mod.rs` re-exports only** — `pub mod` declarations and re-exports, nothing else. Logic goes in
   named files.
9. **`thiserror` for library errors.** `anyhow` only in the binary (`aion-cli`).
10. **Never `.unwrap()` or `.expect()` in library code.** Mutex/lock poison handled explicitly and
    mapped to a typed error (the pattern is right there at
    `crates/aion/src/runtime/engine_tasks.rs:444-448`).
11. **Red evidence as separate commits.**
12. **No arbitrary limits / no assumed defaults.** The patience is a required parameter (§4.3).
13. **No backwards compatibility.** Replace, don't add alongside — no compat shim constructor kept
    beside the new signature, no `#[deprecated]`.
14. **Do not touch the live estate.** No servers, no listeners, nothing outside this tree and your
    own tempdirs.
15. **No silent failures, no lazy code, no shortcuts** (CLAUDE.md). Every `Result` handled, logged
    or propagated. Failure paths tested, not just happy ones.
16. **Load-bearing invariants** (CLAUDE.md): type-erased events; the determinism boundary; **single
    writer per workflow — never call `EventStore::append` directly**; status is a projection;
    content-hash module namespacing. Nothing in this cure should go near them — if your cure does,
    stop and say why in the report before you write it.

---

## 6. Order of work

| Phase | Deliverable | Gate before moving on |
|---|---|---|
| **P0** | R1 red evidence, own commit; isolation result noted | The failing lines are committed verbatim |
| **P1** | R2 evidence set E1/E2/E3 at your hands | The lock holder has a **name** |
| **P2** | Ruling-gate decision written into `.fleet/reports/lockfix.md` | Which branch, on what evidence |
| **P3** | The cure (§4.2/§4.3) + all 37 call sites (§4.4) | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean |
| **P4** | The regression test (§4.5), both halves | It fails without the cure and passes with it — demonstrate both |
| **P5** | R4 proof: lib suite green ×2, `cargo test --workspace --no-fail-fast` green ×1 | Result lines pasted verbatim into the report |
| **P6** | `.fleet/reports/lockfix.md` complete | Red lines, causal chain, refuted hypothesis named, branch taken, cure + sweep, regression test, R4 lines, any typed refusal |

Diagnostic instrumentation never survives into P3+. `git status --porcelain` must be empty of it at
every commit that is not an explicitly-labelled diagnostic-evidence commit.

---

## 7. Refusal protocol

If a requirement cannot be honoured, do not silently narrow it and do not work around it. Write a
**typed refusal** in `.fleet/reports/lockfix.md`: name the requirement (R-number and the sentence),
state the measurement or constraint that makes it unhonourable, name what you did instead, and what
that substitute does and does not buy. The counsel's refusal of R3's first sentence
(`.fleet/plan.md:§4`) is the worked example of the form.

Two known candidates:

- **R3's first sentence**, if your measurements confirm §2 — adopt the counsel's refusal in your own
  words, with your own evidence.
- **§4.3's config home**, if `crates/aion-server`'s config surface has no precedent-compliant place
  to rule the patience — refuse rather than invent a default, and say what the operator surface
  would need.

---

## 8. Anchor ledger — everything this brief leans on, verified at THIS tree

| Claim | Anchor |
|---|---|
| The one behavioural line in the engine diff | `crates/aion/src/engine/api.rs:323` (in `Drop`, `:318-324`) |
| `shutdown` gates, aborts, joins; `begin_close` does not | `crates/aion/src/runtime/engine_tasks.rs:451-476`, `:500-514`, `:516-526`, `:543-573` |
| `Drop for EngineTaskRuntime` backstop | `crates/aion/src/runtime/engine_tasks.rs:575-628` |
| Test-only accessor added by the leg | `crates/aion/src/runtime/engine_tasks.rs:442-449` |
| The leg's own regression test (do not delete) | `crates/aion/src/engine/api/api_tests/shutdown.rs:339-356` |
| `clear_engine_seams` runs only from `Engine::shutdown`, and why | `crates/aion/src/engine/api.rs:658-669`, `:746-754` |
| The build-profile half of the leg diff | `Cargo.toml:121-131` (comment + both profiles; commit `70adba5d`) |
| The failing test | `crates/aion-server/src/api/http/worker_deployments_tests.rs:127-178` |
| Its state builds no engine | `crates/aion-server/src/state.rs:343-359`, `:361-371` |
| The PUT hashes the whole running binary, every call | `crates/aion-server/src/api/http/worker_deployments.rs:70`, `:192-208` |
| The concurrent fork storm | `crates/aion-server/src/worker/workspace_root.rs:772-810` |
| The spawn path it uses | `crates/aion-worker/src/shell/action.rs:167`; `crates/aion-worker/src/process/contained.rs:373-388` |
| Writer lock = advisory `flock` on the open file description, `try_lock`, no wait | `haematite-0.9.0/src/db/lock.rs:1-27`, `:166` |
| `Drop for Database` is synchronous | `haematite-0.9.0/src/db.rs:541-559` |
| Lock taken before any create/open mutation ⇒ retry is safe | `haematite-0.9.0/src/db/startup.rs:105-108`, `:175` |
| The error text | `haematite-0.9.0/src/db/error.rs:422-428` |
| Cure sites | `crates/aion-store-haematite/src/store.rs:297-318`, `:337-345` |
| Required-parameter precedent | `crates/aion-store-haematite/src/store.rs:285-291` |
| `open`'s config-adoption idempotence | `crates/aion-store-haematite/src/store.rs:320-331`, `:342` |
| Production boot call site | `crates/aion-server/src/state.rs:2281` |
| Why the leg changed `Engine::drop` at all | `.fleet/reports/fix-rounds.md:11-17` |
| Why the leg changed the build profile at all | `.fleet/reports/fix-rounds.md:3-9` |

Re-read every one before you lean on it. That rule bound the counsel and it binds you.
