# CAUSE — the SIGSEGV in `recovery_declared_body_e2e`, named from measurement

**Named cause:** latent undefined behaviour in the compiled test program,
surfacing inside **beamr 0.20.0's cranelift JIT compile path** on a
`scheduler::dirty` worker thread — `beamr-0.20.0/src/jit/compiler/ir_helpers.rs`
`declare_compile_helpers` → `cranelift_jit::JITModule::declare_func_in_func` →
`cranelift_codegen::ir::function::FunctionParameters::ensure_user_func_name` →
`HashMap::insert` → hashbrown `RawTable::reserve_rehash` (**SIGSEGV**).
Full backtrace and frame-by-frame reading: `D6-faulting-frame.md`.

**Whether the crash appears is decided by the BUILD DRAW, not by the source.**
Relinking one unchanged tree produces a different binary every time
(`D7-build-draw-lottery.txt`), and one of those binaries — the one
`.fleet/gates/red-3.md:7189` names — faults 20/20 while others pass 20/20.

**Branch taken: F3** (`.fleet/plan.md` §S4.5 / leg brief §3.4), earned by the
measurements below, with F1 ruled out by measurement and F2's version lever
established to be empty.

---

## 1. What was measured

| rung | question | receipt | result |
|---|---|---|---|
| **D1** | does the preserved round-3 artifact actually crash? | `D1-armA-preserved-binary.txt` | **20/20 SIGSEGV** (rc=139), ~11.2 s each. Arm A, redone honestly |
| **D2** | does base `957de0a1` crash? | `D2-armB-base-957de0a1.txt` | **0/20** — 20/20 pass, 1.5 s each |
| **D3** | does `HEAD`'s own source crash, built two ways? | `D3-build-mode-discrimination.txt` | `cargo test --workspace` build (**the gate's own build, same cargo metadata hash `9990dace6ae08d82`, same 141 682 248 bytes, same 349 503 symbols as the crashing artifact**): **0/20**. `cargo test -p aion-server --test …` build: **0/20** |
| **D4** | fmt reflow vs changelog-embed, each alone | `D4-split-942c80e3.txt` | ordered split run; **not discriminating** — see §3 |
| **D5** | isolated vs under `cargo test --workspace --no-fail-fast` with `ulimit -n 65536` | `D5-isolation-vs-load.txt` | see that file |
| **D6** | name the faulting frame | `D6-faulting-frame.md` | beamr's JIT compile path. **`heap_borrow.rs:79` (M12) is NOT in the stack — M12 is retired as this crash's lead** |
| **D7** | is the crash a property of the SOURCE or of ONE BUILD? | `D7-build-draw-lottery.txt` | **twenty independent relinks of one byte-constant tree → twenty different binaries.** See totals in that file |

Two further hypotheses were tested and **killed**, so they are not left hanging:

- **Stack overflow on a beamr worker.** `RUST_MIN_STACK` at 8 MiB, 16 MiB and
  64 MiB: **rc=139 every time**. beamr spawns its threads with no
  `stack_size` call (`grep -rn stack_size beamr-0.20.0/src/` → no hits), so
  `RUST_MIN_STACK` governs and raising it eightfold changes nothing.
- **glibc-detectable heap corruption.** `MALLOC_CHECK_=3` and
  `GLIBC_TUNABLES=glibc.malloc.check=3 MALLOC_PERTURB_=165`: **rc=139, no
  allocator diagnostic**. The corruption is not one glibc's own checker sees.

---

## 2. Why the flight diff is NOT the cause — F1 ruled out

`.fleet/FINDINGS-SEQFIX.md` states: "**THE FLIGHT'S OWN DIFF (957de0a1 →
1000bd36) INTRODUCES THE CRASH.**" That claim does not survive measurement.

1. **The same source produces both outcomes.** D3's arm W is built by the
   gate's own command from `HEAD`'s source and carries the identical cargo
   metadata hash, byte size and symbol count as the crashing artifact. It
   passes 20/20. D7 then shows why: **the link is not reproducible** — twenty
   relinks of one unchanged tree gave twenty distinct sha256s at one constant
   size.
2. **The command O1 names does not reproduce it.**
   `cargo test -p aion-server --test recovery_declared_body_e2e` on the
   **pre-fix** tree exits 0 in 1.56 s (`RED-FIRST.md`).
3. **The diff contains nothing that could do it.** Under `crates/` the only
   paths in `957de0a1..HEAD` that are linked into this binary are the eight
   `crates/aion/src/runtime/` files and the `include_str!`'d changelog.
   `git diff -w 957de0a1..HEAD -- crates/aion/src/runtime/` is import
   reordering, closure/`where`-clause wrapping, and one helper **inside
   `#[cfg(test)] mod tests`** — no semantic change in anything the rlib
   exports. `Cargo.lock`'s only change is the new workspace member
   `aion-fleet-plan` (`clap`, `serde`, `serde_json`, `tempfile`, `thiserror`);
   **no third-party version moved**, and `aion-server` does not depend on it.
4. **There is no `unsafe` in the aion workspace to blame.** `unsafe_code =
   "deny"` at `Cargo.toml:39`. The only `unsafe` in `crates/` is
   `crates/aion-darwin-acl`, which `crates/aion-server/Cargo.toml:124-127`
   gates behind `cfg(target_os = "macos")` — `nm | grep -c darwin_acl` on the
   crashing binary returns **0**. The benchmark's `#[allow(unsafe_code)]` is a
   separate binary. **Every line of `unsafe` in the crashing process belongs to
   a dependency.**

**A benign layout permutation flipping a passing binary to a faulting one is
the definition of latent UB.** `.fleet/plan.md` §S2.1 predicted exactly this
signature; what it did not know is that the perturbation is supplied by the
build itself, not by the diff.

---

## 3. Why D4's split cannot discriminate, and is reported anyway

D4 was ordered as "reflow-only, and changelog-embed-only, each ≥20 runs."
It was run (`D4-split-942c80e3.txt`). Its arms are **one build draw each**, and
D7 measures the draw distribution directly. A single draw per arm cannot tell a
perturbation apart from the lottery, and no number of runs *of one binary*
repairs that — D1 and D3 already show the per-run rate is 20/20 or 0/20 with
nothing in between. The rung's honest answer is: **it does not discriminate,
and D7 is the reason.**

---

## 4. V10's contradiction — RECONCILED

`.fleet/plan.md` §S3.1 calls this "the most informative unknown in this
flight": `942c80e3`'s own commit message closes with "Battery on this tree:
fmt 0, clippy -D warnings 0, workspace suite 0", and round 3's gate then got
SIGSEGV on **byte-identical source**.

**Both are true and they are two draws of the same lottery.**
`.fleet/gates/red-3.md` contains **473 `Compiling` lines beginning at
`proc-macro2`** — round 3's gate built the workspace **from cold**, so it
linked a *new* binary, not the one whose battery `942c80e3` reported.
D7 shows that a fresh link is a fresh draw. Nothing needs to be assumed away:
the green battery was real, the red gate was real, and the artifacts differed.

---

## 5. F2 — the version lever is empty

`.fleet/plan.md` §S4.5/F2 requires establishing "(a) that beamr is the faulting
frame and (b) whether a compatible release fixes it".

- **(a) — established.** D6 names the frame inside beamr's JIT driver.
- **(b) — there is nothing to move to.** The local crates.io index cache
  (`~/.cargo/registry/index/index.crates.io-*/.cache/be/am/beamr`, fetched
  **2026-08-23 08:29 local**, i.e. this morning) lists **0.20.0 as the newest
  published beamr**. The floor `Cargo.toml:96` pins is already the top of the
  registry. There is no compatible release to bump to and therefore no ruling
  to take on a bump.

**And the in-boundary workaround F2 prefers does not exist honestly here.**
The only lever aion holds over whether beamr's JIT compile point is reached is
`RuntimeConfig::jit_threshold`, and this repository's own documentation of that
field (`crates/aion/src/runtime/config.rs`, `jit_threshold`, the "🔴 What a
large value does, and what it does NOT do" block) states in terms that a large
value is **not** an off-switch and "**must not be documented as one**", because
`record_call` saturates and compiles on the far side of the horizon. Setting it
would be masking, would invent a value the builder was not given
(`CLAUDE.md`: *NO ARBITRARY LIMITS / NO ASSUMED DEFAULTS*), and would
contradict the field's own doc comment. **It was not done.**

---

## 6. What the JIT compile path alone does NOT explain

Reaching beamr's JIT compile point is **not sufficient** to fault. Measured on
a clean draw of this tree with a scratch probe that drove
`EngineBuilder::scheduler_jit_threshold` (`crates/aion/src/engine/builder.rs`,
`scheduler_jit_threshold`) down to **1**, so every MFA compiles on its first
recorded call:

| configuration | result |
|---|---|
| threshold 1, 1 workflow run | pass |
| threshold 1, 200 runs | pass |
| threshold 1, 2 000 runs | pass (5.8 s) |
| threshold 1, 20 runs × **25 engine generations** in one process | pass (2.9 s) |
| threshold default (1000), same shapes | pass |

So the JIT compile is where the damage **surfaces**, not where it is done.
The probe was scratch and is not committed; its numbers are recorded here
because a rung that came back inconvenient is a finding, not a reason to skip
ahead.

One further observation from that probe, recorded because it is a real
measurement and not otherwise written down anywhere in this tree: **beamr
scheduler threads are not reclaimed when an engine generation is dropped.**
Twenty-five sequential generations at `scheduler_threads(2)` peaked at **858
live threads** (`/proc/<pid>/status`, `Threads:`, sampled at 50 ms). That is
~34 threads retained per generation. It is not shown to cause this crash — the
crashing process held 64 threads across two generations — and it is reported
here rather than acted on, because acting on it is outside O1's cause and O2's
verification.

---

## 7. The consequence for the gate — stated plainly

Because the outcome is drawn at link time, **a `GATE-GREEN` on this tree is
evidence about the draw the gate happened to build, not a proof that the defect
is gone.** `.fleet/gates/red-3.md` is a red draw of a source that is otherwise
green; this round's gate runs are green draws of a source that can still draw
red. That is the honest reading, and it is why this file exists rather than a
fix commit.

**The sentence the round was given:** *a crash that hides is worse than one
that shows.* This one hides behind the link step. Naming that is the fix that
was available; manufacturing one in aion's source was not.
