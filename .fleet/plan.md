# rb-lockfix — the plan

**Run:** `rb-lockfix-flight1`. **Rig commit:** `65f0bcac4`. **Base of the rig:** `b72a5c35`
(the repo-battery-flight1 leg tree, incl. its red evidence under `.fleet/gates/`).
**Legs:** exactly ONE, id `fix`, brief at `.fleet/legs/fix.md`. It owns the whole delivery.
**Gate entry:** `.fleet/gate-entry.sh` (committed at the rig commit; not the builder's to edit).
**Brief:** `.fleet/BRIEF.md` — R1 RED FIRST, R2 DIAGNOSE AT SOURCE, R3 FIX AT SOURCE, R4 PROVE.

This file replaces repo-battery-flight1's `plan.md`, which is recoverable in full at
`git show b72a5c35:.fleet/plan.md`. Nothing in that plan governs this wave.

**Status of this document:** every claim below marked **MEASURED** was measured by the planning
counsel in THIS working tree, at `65f0bcac4`, on this box (16 cores, `ulimit -n 65536`), with the
result lines quoted verbatim in §8. The builder does not inherit these as fact — R1/R2 are theirs
to re-measure — but they are the fastest known route to the answer, and one of them (§3) refutes
the mechanism the brief names.

---

## 0. The one-paragraph answer

The brief asks the builder to cure a teardown/lock-release ordering bug in
`git diff 856b2a863..b72a5c35 -- crates/aion`. **There is no such bug in that diff.** The engine
change is one line — `crates/aion/src/engine/api.rs:323`, `begin_close()` → `shutdown()` — and
restoring it to `begin_close()` at this tree leaves the failing test failing, identically
(**MEASURED**, §3.2). The teardown under test releases everything it owns: at the moment of the
reopen the store's `Arc` strong count is 1, `Arc::into_inner` yields the value, and no descriptor
in this process points at the lockfile (**MEASURED**, §3.3). The actual holder of the writer lock
is **a forked child of a different test in the same binary**, caught in the act: pid 2849898,
fd 566, `PPid` = the test process, empty `cmdline`, `comm` `worker::workspa` — a
`posix_spawn`ed child of
`worker::workspace_root::tests::every_accepted_printable_ascii_root_survives_the_real_parser`
(`crates/aion-server/src/worker/workspace_root.rs:772-810`, which spawns 95 shells in a loop),
still between clone and `exec`, holding an inherited copy of the lock's open file description
(**MEASURED**, §3.4). haematite's writer lock is an advisory `flock` owned by the open file
description, so the copy holds the lock; the window measured **93–150 ms**. What actually moved
the test into that window is the leg's OTHER change: `Cargo.toml:125-131` (`[profile.dev]`/
`[profile.test]`, `debug = 0`), which shrank the test binary that
`put_worker_deployment` sha256s on **every PUT** (`crates/aion-server/src/api/http/worker_deployments.rs:70`
→ `:192-208`) from a debuginfo build to 188 MiB — **MEASURED 18.64 s** for that hash at this
tree, against the 60+ s the brief records at base. The test did not lose an awaited completion.
It lost sixty seconds of incidental latency that had been hiding a race it never controlled.

So: R2 is answered on its second branch ("if you conclude the TEST model is wrong instead, prove
it against base behavior"), R3's literal premise is refused as measurably false (§4), and the cure
moves to the lock-acquisition boundary (§5), where it fixes 37 call sites' worth of the same
latent race instead of one test's symptom.

---

## 1. The diff, read at the changed lines

`git diff 856b2a863..b72a5c35 -- crates/aion` is 6 files, +1477/−1453, and **exactly one
commit**: `0f3aff9d fix(gate): release engine task descriptors on drop`. Its rationale is on the
record at `.fleet/reports/fix-rounds.md:11-17` — the gate's `aion-server` lib target hit
`Too many open files (os error 24)` because `Engine::drop` left each engine-task epoch's Tokio
runtime (and its I/O driver descriptors) owned for the life of the test process.

Line by line, what changed that can touch teardown ordering:

| Anchor (THIS tree) | Before (`856b2a863`) | After (`b72a5c35`) |
|---|---|---|
| `crates/aion/src/engine/api.rs:323` | `self.runtime.engine_tasks().begin_close();` | `self.runtime.engine_tasks().shutdown();` |
| `crates/aion/src/engine/api.rs:71-83` | doc: drop "gates and aborts, it does not await" | doc: drop closes the epoch with `shutdown`, runtime drop isolated on a joiner thread |
| `crates/aion/src/engine/api.rs:110-115` | doc: async-context objection justifies `begin_close` | doc: timer-wheel shutdown "remains safe before the joined engine-task shutdown below" |
| `crates/aion/src/engine/api.rs:131-146` | doc: "an engine released without `shutdown` still holds its runtime" | doc: the scheduler leak stands; the engine-task executor "is joined below so its I/O driver cannot accumulate process descriptors" |
| `crates/aion/src/engine/api.rs:158-161` | doc: step 1's `begin_close()` silences the detached appender | doc: "the engine-task epoch closure" silences it |
| `crates/aion/src/runtime/engine_tasks.rs:442-449` | — | **added** `#[cfg(test)] owns_runtime()` |
| `crates/aion/src/engine/api/api_tests/shutdown.rs:339-356` | — | **added** `engine_drop_releases_the_descriptor_owning_task_runtime` |
| `crates/aion/src/engine/api.rs:789`, `crates/aion/src/runtime/engine_tasks.rs:630` | inline `mod tests { … }` | extracted to `api/api_tests{,/lifecycle.rs,/shutdown.rs}` and `engine_tasks/tests.rs` |

Everything else in those 1477/1453 lines is test relocation. `EngineTaskRuntime::shutdown`
(`crates/aion/src/runtime/engine_tasks.rs:451-476`), `gate_and_abort` (`:500-514`), `begin_close`
(`:516-526`), `shutdown_runtime_and_join` (`:543-573`) and `Drop for EngineTaskRuntime`
(`:575-628`) are all **unchanged bodies** — the leg changed which of them `Engine::drop` calls,
and nothing else.

Read on its own terms, that one line moves teardown in the **safe** direction: `shutdown()` gates,
aborts, then blocks on a joiner thread until the owned runtime's drop has cancelled every task and
waited out every in-flight poll (`engine_tasks.rs:451-457`, `:543-573`). `begin_close()` gates and
aborts and returns (`:516-526`). The brief's hypothesis — "this tree makes teardown return before
the store writer lock is actually released" — predicts the opposite sign from what the line does.
That is the first reason to measure before coding, and §3.2 is that measurement.

**And there is a second changed file outside `crates/aion` that the brief's diff command never
shows.** `git diff 856b2a863..b72a5c35 -- Cargo.toml` is the round-1 fix
(`.fleet/reports/fix-rounds.md:3-9`, commit `70adba5d`):

```
Cargo.toml:125-131
[profile.dev]
debug = 0
incremental = false

[profile.test]
debug = 0
incremental = false
```

Everything else in the leg diff is `.fleet/` records and `workflows/repo-battery/**`. So the
whole causal surface between base and this tree is: **one engine call site, and the build
profile.** §3 measures both.

---

## 2. What the failing test actually does

`crates/aion-server/src/api/http/worker_deployments_tests.rs:127-178`
(`api::http::worker_deployments::tests::http_put_survives_a_second_server_state_over_the_same_store`):

1. `:129-137` — `tempfile::tempdir()`, then `HaematiteStore::open_or_create(path)` → the **create**
   branch (`crates/aion-store-haematite/src/store.rs:297-318`), which takes the A4 writer lock.
2. `:138-139` — `state_with_leaf(first_store.clone())` →
   `ServerState::from_parts_with_namespace_store` (`crates/aion-server/src/state.rs:343-359`) →
   `from_parts_with_control_stores` (`:361-371`). Read its own doc line: *"the explicit
   embedder/test seam for retaining both control-plane contracts from one durable leaf **without
   constructing the full engine boot**"*. **This state contains no `Engine`.** No `Engine::drop`
   runs anywhere in this test. That alone should have moved the diagnosis off the engine diff.
3. `:141-153` — one `PUT /worker-deployments/shells`. The handler
   (`crates/aion-server/src/api/http/worker_deployments.rs:63-111`) calls `capture_binary_identity()`
   at `:70`, which at `:192-208` does `std::fs::read(std::env::current_exe())` and `Sha256::digest`
   over the **entire running test binary**, on every PUT, uncached.
4. `:155-157` — `drop(first_router); drop(first_state); drop(first_store);`.
5. `:159-165` — `HaematiteStore::open_or_create(path)` again → the **open** branch → `Database::open`
   → A4 `try_lock`. This is the line that returns `DataDirLocked`.

The store side of step 5, with anchors in the vendored dependency
(`~/.cargo/registry/src/index.crates.io-*/haematite-0.9.0/`):

- `src/db/lock.rs:1-27` — the lock is an **advisory OS `flock`** over `<data_dir>/writer.lock`,
  *"acquired non-blocking"*, *"held exactly as long as this value (and any file descriptions it
  forked to) lives"*, and *"release happens when the last handle to the open file description
  closes"*.
- `src/db/lock.rs:166` — the acquisition is `file.try_lock()`. **There is no wait, no retry and no
  backoff anywhere on this path.**
- `src/db.rs:541-559` — `Drop for Database` is synchronous: sync schedulers, executor, router,
  scheduler, then the fields (the lock `File`) drop.
- `src/db/error.rs:422-428` — the message the test prints.

That kills the brief's stated base mechanism outright: *"at base the second server state WAITS for
the first state teardown to release the writer lock, then proceeds"* (`.fleet/BRIEF.md:19-20`).
haematite cannot wait. It calls `try_lock` once and returns. At base the second open **succeeded on
its first and only attempt**, because by the time it ran, ~60 s had passed and the lock was free.
Base did not wait for correctness. Base got lucky, on a clock it did not control.

---

## 3. The measured causal chain

Four experiments, in the order they were run. Verbatim result lines in §8.

### 3.1 Reproduction — the brief's numbers hold

`ulimit -n 65536; cargo test -p aion-server --lib`, clean tree, twice:
`1155 passed; 1 failed` in **23.31 s**, then `1155 passed; 1 failed` in **22.59 s**. Same test,
same error, a fresh tempdir path each time. **MEASURED, 2/2.**

### 3.2 Attribution — the engine line is NOT the trigger

`crates/aion/src/engine/api.rs:323` reverted in place to `begin_close()` (base behaviour), full
rebuild, full lib suite: **`1155 passed; 1 failed` in 22.60 s — the same test, the same error.**
**MEASURED.** The file was restored with `git checkout --` immediately afterwards; the tree this
plan is committed from is clean.

That is decisive on its own: *the only engine change between base and this tree does not produce
the failure and its removal does not cure it.* R3's "cure the ordering" has no ordering to cure.

### 3.3 The teardown under test releases everything it owns

Probe compiled into the test between `:157` and `:159` (then reverted): print
`Arc::strong_count(&first_store)`, replace `drop(first_store)` with `Arc::into_inner(first_store)`,
then walk `/proc/self/fd` for any descriptor whose readlink is the lockfile.

```
PROBE strong_count = 1
PROBE was_last_reference = true
(no /proc/self/fd entry for writer.lock)
```

**MEASURED.** No surviving `Arc`, no surviving descriptor in this process, and `Database::drop`
(synchronous, `db.rs:541-559`) had therefore already run to completion — **before** the reopen.
The teardown contract R3 asks to restore is already held at this tree.

### 3.4 The holder is a forked child of a concurrent test

Same probe extended: on each failed `try_lock`, scan `/proc/*/fd` for the lockfile and print the
holder's `cmdline` and `status`, then retry until acquired.

```
PROBE holder pid "2849898" fd "566" cmdline [] status ["Name:\tworker::workspa",
      "State:\tR (running)", "PPid:\t2838586", "Threads:\t1"]
PROBE lock free after 147.384551ms (3 attempts)
```

(and, on the previous run, `pid "2838280" fd "565" cmdline []`, `lock free after 149.952797ms`;
on the run before that, `lock free after 92.941257ms`.) `PPid 2838586` is the test process itself,
whose own `status` in the same dump reads `Threads: 6757`. The entries the same scan printed for
`self` / `thread-self` / our own pid were **the probe's own open handle on the lockfile**, held
across the scan — not a leak; do not misread them.

So the holder is a child process of the test binary, `State: R`, one thread, **empty `cmdline`**
and a `comm` inherited from the forking thread (`worker::workspa`) — i.e. cloned and not yet
`exec`ed. `crates/aion-server/src/worker/workspace_root.rs:772-810` runs
`for code in 0x20u8..=0x7Eu8` and executes a real shell per accepted character through
`aion_worker::shell::ShellAction` (`crates/aion-worker/src/shell/action.rs:167` →
`crates/aion-worker/src/process/contained.rs:373-388`, `process_group(0)`, `kill_on_drop`).
Ninety-five spawns in a burst, concurrent with our test. Each clone copies the parent's descriptor
table; `flock` ownership is per **open file description**, so every copy holds the lock until the
child `exec`s (the descriptor is `CLOEXEC`, so `exec` is what frees it) or dies. In a 6757-thread,
188 MiB process under 16-way test load, that window measured **93–150 ms**.

Corroboration: when the probe simply **waited** for the lock before reopening, the full suite went
green — `1156 passed; 0 failed` twice, 22.10 s and 22.54 s (**MEASURED**). The contract the test
asserts is true; it is asserted 100 ms too early.

Counter-check on scale: the same two tests run as a pair
(`--test-threads=2`, `http_put_survives…` + `every_accepted_printable_ascii…`) pass in 9.45 s
(**MEASURED**) — in a small, few-thread process the clone→`exec` window is microseconds. The race
needs the full suite's thread count and load, which is exactly why the brief records
isolation-green, suite-red.

### 3.5 What moved the test into the window

`capture_binary_identity` (`worker_deployments.rs:192-208`) reads and hashes the whole running
executable on every PUT. At this tree that binary is **188 MiB** and the isolated hash test
(`capture_uses_build_identity_and_running_executable_sha256`, the same code path) takes **18.64 s**
(**MEASURED**; `sha2` is unoptimised in a dev profile). The failing test's reopen therefore lands
~19 s into a 22 s suite — squarely inside the `workspace_root`/supervisor spawn phase.

At base the same read+hash ran over a **debuginfo** binary, and the brief records the consequence
directly: *"the test itself runs 60+ seconds (cargo prints the over-60s notice; suite 125s)"*
(`.fleet/BRIEF.md:18-19`). Sixty-plus seconds of sha256 pushed the reopen past the fork phase.
`Cargo.toml:125-131` removed the debuginfo, the hash got ~3× cheaper, and the reopen moved into
the storm.

**The diagnostic signature in the brief is real and its reading was inverted.** The test did get
faster when it started failing — but the vanished wait was not an awaited completion. It was
`std::fs::read` + `Sha256` over a debuginfo build. The wait was never the correctness; it was the
camouflage.

---

## 4. Typed refusal

**REFUSAL — R3, first sentence, as written.** `.fleet/BRIEF.md:47-48` directs: *"Restore the
teardown contract: teardown-complete must not report before the store writer lock is released."*
That defect does not exist at this tree and cannot be restored, because it was never lost:

- the state under test builds **no engine** (`crates/aion-server/src/state.rs:361-371`), so no
  engine teardown runs in the failing test at all;
- at the instant of the reopen the store's strong count is **1**, `Arc::into_inner` succeeds, and
  this process holds **no** descriptor on the lockfile (§3.3);
- reverting the sole engine teardown change (`crates/aion/src/engine/api.rs:323`) leaves the test
  failing identically (§3.2);
- the lock is held by **another process** — a not-yet-`exec`ed child forked by a concurrent test
  (§3.4).

Writing code to "make teardown-complete wait for the lock" would therefore be writing a wait into
a path that already waits, to compensate for a descriptor this process does not own. It would not
cure the failure; it would move the sleep.

**What is honoured instead, under the brief's own escape clauses** — `.fleet/BRIEF.md:29-31`
("Follow the evidence if it points elsewhere; do not follow this brief off a cliff") and R2's
second branch (`:43-45`, "If you conclude the TEST model is wrong instead, prove it against base
behavior … and carry the proof"): the proof is §2 and §3, including the correction that base's
pass is not a wait (haematite `try_lock`s once, `lock.rs:166`) but a coincidence of latency. The
substitute cure is §5. R1, R2, R4 are honoured as written. R3's remaining clauses — *fix at
source*, *do not revert the leg wholesale*, and every wall in its list — bind the substitute cure
unchanged.

---

## 5. The cure

### 5.1 What it must be

A store reopen must be able to survive a **transient** holder of the data-dir lock: a descriptor
copy that a concurrently-forked child holds for the length of its clone→`exec` window. It must
still fail loudly against a **genuine** second live writer, which is the invariant the lock exists
for (haematite `src/db/lock.rs:1-27`; Aion's invariant 3, single writer per workflow, rides on it).

Those two together mean exactly one thing: the acquisition gets a **ruled patience**. Not a sleep,
not a retry count — a patience: re-attempt the acquire until it succeeds or the ruled span
elapses, then return the same typed error unchanged.

### 5.2 Where

`crates/aion-store-haematite/src/store.rs`:

- `HaematiteStore::open` — `:337-345` (the `open` branch is where a just-released dir is
  re-acquired; `crates/aion-server/src/state.rs:2281` is the production boot call);
- `HaematiteStore::open_or_create` — `:297-318` (routes to `open` when `config.json` exists).

`create` / `create_with_shard_count` are **out of scope**: a directory with no `config.json` has no
prior lock for a child to have inherited, so there is no transient holder to wait out. Say so in
the report rather than silently leaving them.

### 5.3 Shape, and the wall it must not break

`NO ARBITRARY LIMITS / NO ASSUMED DEFAULTS` (CLAUDE.md) forbids inventing the value. The crate
already has the precedent for exactly this shape, in the same file, for the same reason —
`store.rs:285-291`: *"`node_cache_budget` is REQUIRED and has no default here … the byte ceiling is
the caller's deployment decision."* Follow it: the patience is a **required parameter**, ruled by
every caller. Cadence between attempts is part of the same ruling, not a constant the builder
picks — carry it in the same argument (one required type with both fields, named for what it is)
so no call site can rule half of it.

- Production boot (`crates/aion-server/src/state.rs:2281`) rules it from the operator config
  surface, following whatever precedent that config crate already uses for storage knobs. If no
  precedent-compliant home exists there, **stop and record a typed refusal in the report** rather
  than inventing a default in a config schema.
- Tests rule it explicitly at their call sites. A test naming its own patience is explicit, not
  arbitrary — and it is the opposite of a hidden sleep.

Retry only on `DatabaseError::DataDirLocked`. Every other error returns on the first attempt,
unchanged. Retrying that one variant is **safe by haematite's own construction**, and cite it:
`src/db/startup.rs:105-108` (create takes the lock *before* `config.json` is written and skips its
cleanup when the acquire loses) and `:175` (open takes the lock, *then* starts shards). A failed
acquire has started nothing and written nothing. `adopt_node_cache_budget`, which
`HaematiteStore::open` runs first (`store.rs:342`), is idempotent by its own contract
(`store.rs:320-331`), so re-running it across attempts changes nothing.

### 5.4 Why this and not the smaller-looking options

| Candidate | Verdict |
|---|---|
| Revert `api.rs:323` to `begin_close()` | **Forbidden and useless.** Measured not to cure (§3.2); reinstates the descriptor leak `0f3aff9d` fixed (`.fleet/reports/fix-rounds.md:11-17`); R3 forbids reverting the leg's wanted work. |
| Revert `Cargo.toml:125-131` (restore debuginfo) | **Forbidden.** It cures nothing — it re-hides the race behind a 60-second sha256 — and it re-breaks the round-1 disk failure (98.5 GiB `target/`, `.fleet/reports/fix-rounds.md:3-9`). "Slow and correct" is R4's licence for a *real* wait, not for restoring camouflage. |
| Narrow the clone→`exec` window at the spawn site | **Unavailable.** `crates/aion-worker/src/process/contained.rs:373-388` already goes through `std`'s `posix_spawn`-eligible path; `CLOEXEC` is already set by `std` on the lockfile. A descriptor copy exists between clone and `exec` no matter what — that is the semantics of `fork`, and it cannot be closed from this repo. |
| Cache the executable hash in `capture_binary_identity` | **Out of scope, and not a cure** — it makes the reopen *earlier*, not safer. Worth recording as an observed cost (18.6 s per PUT in a dev profile; `worker_deployments.rs:192-208` re-reads and re-hashes the whole binary on every call) and raising separately. Do not fold it into this wave. |
| Change the test | **Forbidden by R3's walls** — no test deletion, no assertion weakening, no timeout inflation, no added sleeps as synchronization. The test's assertions stay byte-identical; only the patience it hands to the constructor is new, and that is a real acquire, not a sleep. |

### 5.5 Blast radius, named up front

`HaematiteStore::open` / `open_or_create` have **37 call sites across 19 files** outside the
implementing module (`crates/aion-server/src/{state.rs,cluster.rs,worker/outbox_dispatcher.rs,observability/instrumented_store.rs,api/http/worker_deployments_tests.rs}`,
`crates/aion-server/tests/*` ×9, `crates/aion/tests/*` ×3, `crates/aion-store-haematite/tests/*` ×2).
Every one of them must rule the patience. That is mechanical, it is the price of the
no-assumed-defaults wall, and it is also the point: several of those call sites
(`deploy_restart_e2e.rs`, `namespace_restart.rs`, `outbox_transport_e2e.rs`,
`recovery_declared_body_e2e.rs`, `graceful_shutdown_park_e2e.rs`) are the same reopen-after-close
shape in the same process-spawning binaries, i.e. the same latent race, currently unfired. Curing
at the acquisition boundary retires all of them at once. Curing at one test's teardown retires
none.

---

## 6. The leg's work plan

One leg, `fix`. Phases are sequential; each one's evidence is committed before the next begins.

- **P0 — Reproduce (R1).** Full `aion-server` lib suite at this tree, `ulimit -n 65536`, twice.
  Commit the failing output verbatim as red evidence, **in its own commit**, and record the
  isolation-green observation beside it (isolation green is part of the record per R1).
- **P1 — Diagnose (R2).** Re-run §3.2/§3.3/§3.4 independently. The probes are throwaway
  instrumentation: they are reverted before P3, and the tree must be clean of them at every commit
  that is not explicitly a diagnostic-evidence commit. Land the causal chain in
  `.fleet/reports/lockfix.md`, naming the diff line examined (`crates/aion/src/engine/api.rs:323`),
  the measurement that clears it, and the holder actually caught.
- **P2 — Ruling gate.** If P1 confirms §3, adopt §4's refusal and §5's cure and say so in the
  report. If P1 contradicts §3 — the engine line *does* carry the failure at the builder's hands —
  abandon §5, cure the ordering as the brief anticipated, and carry the contradicting measurement.
  Either way the report records which branch was taken and on what evidence. **The plan does not
  get to be right; the measurement does.**
- **P3 — Cure (R3).** §5.2/§5.3. Then update all 37 call sites (§5.5).
- **P4 — Regression proof.** A test that fails without the cure and passes with it, at the
  acquisition boundary — the honest one holds the lock on a `writer.lock` descriptor from a second
  descriptor while a reopen runs, and asserts the reopen acquires within its ruled patience and
  that a holder outliving the patience still returns `DataDirLocked` unchanged. No `#[ignore]`, no
  runtime-gated skip unless the harness genuinely needs one (and then per CLAUDE.md: env var +
  `tracing::info!` skip line + `Ok(())`, never `#[ignore]`).
- **P5 — Prove (R4).** `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D
  warnings`; full `aion-server` lib suite **green twice consecutively**; one
  `cargo test --workspace --no-fail-fast` green. All at the builder's hands, result lines committed
  verbatim in `.fleet/reports/lockfix.md`. R4's licence stands: if the restored behaviour is slower,
  slow and correct beats fast and lying.
- **P6 — Report.** `.fleet/reports/lockfix.md` carries: the R1 red lines, the R2 causal chain with
  the refuted hypothesis named, the P2 branch taken, the cure and its call-site sweep, the P4
  regression test, the R4 result lines, and any typed refusal.

Gate scope is fixed by `.fleet/gate-entry.sh` and is not the builder's to edit.

---

## 7. Walls (they bind the plan and the builder equally)

From `.fleet/BRIEF.md:50-55` and CLAUDE.md, restated so no one has to go looking:

1. No test deletion. 2. No assertion weakening. 3. No timeout inflation. 4. No added sleeps as
synchronization. 5. **No lint suppressions in any spelling** — `#[allow]`, `#[expect]`, `#[deny]`,
`_var` renames, `#[cfg(any())]` dead-code hiding are bypasses, not fixes. 6. **No ignore
attributes.** 7. No file over 500 lines of code. 8. `mod.rs` is re-exports and `pub mod` only.
9. `thiserror` for library errors (`anyhow` only in the binary). 10. **Never `.unwrap()` or
`.expect()` in library code**; lock poison handled explicitly and mapped to a typed error.
11. Red evidence as separate commits. 12. No arbitrary limits / no assumed defaults. 13. No
backwards-compatibility shims — replace, don't add alongside. 14. Do not touch the live estate.

Two of these have teeth in this specific cure: **(4)** — the patience is a real acquire that
returns the instant the lock frees (measured 93–150 ms), not a fixed sleep, and the report must say
so at the site; and **(12)** — the patience is a required parameter with no invented default, per
the `node_cache_budget` precedent at `store.rs:285-291`.

---

## 8. Evidence appendix — verbatim result lines, planning counsel's hands

Box: 16 cores, 31 GiB, `ulimit -n 65536`, this working tree at `65f0bcac4`.

```
# 3.1 clean reproduction, run 1
test api::http::worker_deployments::tests::http_put_survives_a_second_server_state_over_the_same_store ... FAILED
Error: Backend("haematite database error: data dir is locked by another live writer (writer lock held at
  /tmp/.tmpJ5qNsM/worker-deployments.db/writer.lock); a second writer would corrupt the shard WALs — use
  ReadOnlyDatabase to observe")
test result: FAILED. 1155 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 23.31s

# 3.1 clean reproduction, run 2
Error: Backend("haematite database error: data dir is locked by another live writer (writer lock held at
  /tmp/.tmpvCYy6I/worker-deployments.db/writer.lock); …")
test result: FAILED. 1155 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.59s

# 3.2 attribution: crates/aion/src/engine/api.rs:323 reverted to begin_close() — STILL RED
test api::http::worker_deployments::tests::http_put_survives_a_second_server_state_over_the_same_store ... FAILED
test result: FAILED. 1155 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.60s

# 3.3 the teardown releases what it owns
PROBE strong_count = 1
PROBE was_last_reference = true
   (/proc/self/fd scan for writer.lock: no match)

# 3.4 the holder, caught
PROBE holder pid "2849898" fd "566" cmdline [] status ["Name:\tworker::workspa", "State:\tR (running)",
  "PPid:\t2838586", "Threads:\t1"]
PROBE holder pid "2838280" fd "565" cmdline []
PROBE lock free after 92.941257ms (3 attempts)
PROBE lock free after 149.952797ms (3 attempts)
PROBE lock free after 147.384551ms (3 attempts)
   (test process own status in the same dump: Threads: 6757)

# 3.4 waiting for the lock turns the suite green, twice
test result: ok. 1156 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.10s
test result: ok. 1156 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.54s

# 3.4 counter-check: the two tests alone, 2 threads — green, small process, no window
test worker::workspace_root::tests::every_accepted_printable_ascii_root_survives_the_real_parser ... ok
test api::http::worker_deployments::tests::http_put_survives_a_second_server_state_over_the_same_store ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1154 filtered out; finished in 9.45s

# 3.5 the vanished latency
target/debug/deps/aion_server-8f387436cc071bf6 = 188 MiB
test result: ok. 1 passed; … finished in 18.64s
  (api::http::worker_deployments::tests::capture_uses_build_identity_and_running_executable_sha256,
   the same std::fs::read + Sha256 the PUT handler runs at worker_deployments.rs:70)
```

All probe instrumentation was reverted with `git checkout --`; `git status --porcelain` is empty
apart from this wave's `.fleet/` deliverables.

---

## 9. What would falsify this plan

Stated so the builder can kill it cheaply rather than inherit it:

1. **§3.2 fails to reproduce** — if reverting `api.rs:323` turns the suite green at the builder's
   hands, the engine line *is* load-bearing and §4's refusal is wrong. Take P2's second branch.
2. **§3.3 shows `strong_count > 1`** — then something in-process does retain the store and the
   diagnosis moves back inside this repo's teardown. Name the holder and cure it there.
3. **The holder scan finds no external process, repeatedly, while the lock stays held** — then the
   descriptor copy story is wrong and the holder is something neither of us has named; keep
   scanning (`/proc/*/fd`, `State`, `PPid`) until it has a name. A cure aimed at an unnamed holder
   is a guess.
4. **The patience cure does not turn the suite green twice** — then the window is not bounded the
   way §3.4 measured, and the ruling in §5.3 is the wrong shape. Report it as measured; do not
   inflate the patience until it passes. An inflated patience is a timeout inflation wearing a
   different hat, and R3's third wall names it.
