# M13 — a link-draw-determined SIGSEGV in beamr's JIT compile path

**Status: OPEN. Not fixed by this flight, and not caused by any diff.**
Recorded here for the same reason M12 was: a gate that dies without printing a
`test result:` line goes red for a reason no diff explains, and the next round
should not have to rediscover it.

**This finding supersedes M12 as the lead for the `recovery_declared_body_e2e`
SIGSEGV. It does not retire M12 itself** — M12 records a different defect (a
non-unwinding *abort* with a printed panic message, at
`beamr-0.20.0/src/term/heap_borrow.rs:79`, on `beamr-sched-0`, intermittent
under load). This one is a silent *segmentation fault* on a
`scheduler::dirty` worker with no message at all.

## The fault, verbatim

```
     Running tests/recovery_declared_body_e2e.rs (target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82)
running 1 test
error: test failed, to rerun pass `-p aion-server --test recovery_declared_body_e2e`

Caused by:
  process didn't exit successfully: `…/recovery_declared_body_e2e-9990dace6ae08d82` (signal: 11, SIGSEGV: invalid memory reference)
```
(`.fleet/gates/red-3.md:7183-7189`.)

The process dies before the harness prints a `test result:` line, so a reader
scanning for `FAILED` finds nothing — same reading trap as M12.

## The faulting frame

```
#0  hashbrown::RawTable<(UserExternalName, UserExternalNameRef)>::reserve_rehash   <-- SIGSEGV
#4  hashbrown::HashMap<UserExternalName, UserExternalNameRef, RandomState>::insert
#6  cranelift_codegen::ir::function::FunctionParameters::ensure_user_func_name
#7  cranelift_codegen::ir::function::Function::declare_imported_user_function
#8  <cranelift_jit::backend::JITModule as cranelift_module::Module>::declare_func_in_func
#9  beamr::jit::compiler::ir_helpers::declare_compile_helpers
#10 beamr::jit::compiler::dispatch::JitCompiler::compile_with_module_metadata
#13 beamr::jit::compile_job::CompilationJob::run
#17 beamr::scheduler::dirty::worker_loop
```

Full unwind and thread census: `docs/evidence/seqfix-sigsegv/D6-faulting-frame.md`.
The map being grown is on a `Function` created microseconds earlier by
`jit_module.make_context()`. Nothing about that insert can fault on its own —
the memory it reached for was already invalid. **The JIT compile is where the
damage surfaces, not necessarily where it is done.**

## It is decided by the LINK, not by any source change

Measured on this venue, with the source held byte-constant:

| what was built | draws | runs each | crash |
|---|---|---|---|
| the preserved round-3 artifact | 1 | 20 | **20/20** |
| `HEAD`, `cargo test --workspace` (identical cargo metadata hash, size and symbol count to the artifact above) | 1 | 20 | 0/20 |
| `HEAD`, `cargo test -p aion-server --test recovery_declared_body_e2e` | 1 | 20 | 0/20 |
| base `957de0a1` | 1 | 20 | 0/20 |
| `HEAD`, relinked repeatedly from one unchanged tree | **20** | 1 | see `docs/evidence/seqfix-sigsegv/D7-build-draw-lottery.txt` |

Twenty relinks of one byte-constant tree produce **twenty distinct sha256s at
one constant byte size**. Comparing a crashing draw against a passing draw:
identical section sizes, identical symbol count (349 503), 361 015 differing
bytes confined to `.rodata` (the same string constants in a different order),
`.rela.dyn`, and 10 431 bytes of `.text`. **Same program, different layout.**

## Retired hypotheses — do not re-run these

- **Stack overflow.** `RUST_MIN_STACK` 8 MiB / 16 MiB / 64 MiB on the crashing
  artifact: rc=139 every time. beamr sets no `stack_size` on any thread.
- **glibc-visible heap corruption.** `MALLOC_CHECK_=3` and
  `GLIBC_TUNABLES=glibc.malloc.check=3 MALLOC_PERTURB_=165`: rc=139, no
  allocator diagnostic.
- **"reaching the JIT compile point is enough".** Driving
  `EngineBuilder::scheduler_jit_threshold` down to 1 — every MFA compiled on
  its first recorded call — across 2 000 workflow runs and across 25 engine
  generations in one process: green throughout.

## Where it points, and what is NOT concluded

`unsafe_code = "deny"` (root `Cargo.toml:39`) means the aion workspace
contributes no `unsafe` to this binary; `crates/aion-darwin-acl` is the only
exception and is `cfg(target_os = "macos")`-gated, with **0** matching symbols
in the crashing binary. So the undefined behaviour is in a dependency, and
beamr is the only dependency in the faulting stack.

**Which unsafe operation actually does the damage is NOT established here.** A
layout-sensitive fault localises to a *program*, not to a line. Naming a
culprit line on this much would be a guess, and M12's own closing discipline
applies.

## What the next round should do

1. Treat a `GATE-RED: workspace suite` whose only failure is
   `recovery_declared_body_e2e` dying on **signal 11 with no `test result:`
   line** as **this** defect. **Do not "fix" it by rebuilding until green** —
   a rebuild is a new draw and a green draw proves nothing.
2. **Do not attribute it to the diff under review.** It has been measured
   against `957de0a1` and against `HEAD` and it tracks neither.
3. There is no beamr release above 0.20.0 to move to (checked against the
   local crates.io index cache, fetched 2026-08-23 08:29 local), so the
   version lever is empty until beamr publishes.
4. Localising it wants a memory sanitiser: no `valgrind`, `gdb`, `lldb` or
   `eu-stack` is installed on this venue, and a `-Zsanitizer=address` build
   needs a nightly toolchain. **That is the tool gap to close before the next
   attempt** — every rung above was run without a debugger, and that is the
   ceiling this round hit.
