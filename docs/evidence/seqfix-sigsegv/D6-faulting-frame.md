# D6 — the faulting frame, named from the core dump

The crash record at `.fleet/gates/red-3.md:7183-7189` says only "signal: 11,
SIGSEGV". This file names the frame. It RETIRES the leading hypothesis:
`beamr-0.20.0/src/term/heap_borrow.rs:79` (M12) is NOT in this backtrace.

## How it was obtained

```
# /proc/sys/kernel/core_pattern is systemd-coredump on this venue, so every
# run of the preserved artifact left a core. No debugger was installed: the
# unwind below is systemd-coredump.s own, via `coredumpctl info`.
coredumpctl info --no-pager
```

## The crashing thread, verbatim (demangled below the raw frames)

```
           PID: 2942322 (recovery_declar)
           UID: 1000 (rocketfish)
           GID: 1000 (rocketfish)
        Signal: 11 (SEGV)
     Timestamp: Sun 2026-08-23 14:05:45 AEST (33s ago)
  Command Line: /home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82
    Executable: /home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82
 Control Group: /user.slice/user-1000.slice/session-915.scope
          Unit: session-915.scope
         Slice: user-1000.slice
       Session: 915
     Owner UID: 1000 (rocketfish)
       Boot ID: e2cc4ea9c2b844c0ba8600ab2f1f2a35
    Machine ID: 5b66a7f4f7114b2f91c63495c3e6962e
      Hostname: omarchy
       Storage: /var/lib/systemd/coredump/core.recovery_declar.1000.e2cc4ea9c2b844c0ba8600ab2f1f2a35.2942322.1787457945000000.zst (present)
  Size on Disk: 6.3M
       Message: Process 2942322 (recovery_declar) of user 1000 dumped core.
                
                Stack trace of thread 2942499:
                #0  0x0000562dc847dabe _RNCINvMs6_NtCsgQfI1edjipl_9hashbrown3rawINtB8_8RawTableTNtNtNtCsIgkLe5NID3_17cranelift_codegen2ir7extname16UserExternalNameNtNtBW_8entities19UserExternalNameRefEE14reserve_rehashNCINvNtBa_3map11make_hasherBS_B1X_NtNtNtCs2AWtUsOyxgP_3std4hash6random11RandomStateE0E0BY_ (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x3bccabe)
                #1  0x0000562dc844f1de _RINvMs6_NtCsgQfI1edjipl_9hashbrown3rawINtB6_8RawTableTNtNtNtCsIgkLe5NID3_17cranelift_codegen2ir7extname16UserExternalNameNtNtBU_8entities19UserExternalNameRefEE14reserve_rehashNCINvNtB8_3map11make_hasherBQ_B1V_NtNtNtCs2AWtUsOyxgP_3std4hash6random11RandomStateE0EBW_ (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x3b9e1de)
                #2  0x0000562dc844f876 _RINvMs6_NtCsgQfI1edjipl_9hashbrown3rawINtB6_8RawTableTNtNtNtCsIgkLe5NID3_17cranelift_codegen2ir7extname16UserExternalNameNtNtBU_8entities19UserExternalNameRefEE7reserveNCINvNtB8_3map11make_hasherBQ_B1V_NtNtNtCs2AWtUsOyxgP_3std4hash6random11RandomStateE0EBW_ (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x3b9e876)
                #3  0x0000562dc844f3a1 _RINvMs6_NtCsgQfI1edjipl_9hashbrown3rawINtB6_8RawTableTNtNtNtCsIgkLe5NID3_17cranelift_codegen2ir7extname16UserExternalNameNtNtBU_8entities19UserExternalNameRefEE25find_or_find_insert_indexNCINvNtB8_3map14equivalent_keyBQ_BQ_B1V_E0NCINvB34_11make_hasherBQ_B1V_NtNtNtCs2AWtUsOyxgP_3std4hash6random11RandomStateE0EBW_ (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x3b9e3a1)
                #4  0x0000562dc83f4967 _RNvMs1_NtCsgQfI1edjipl_9hashbrown3mapINtB5_7HashMapNtNtNtCsIgkLe5NID3_17cranelift_codegen2ir7extname16UserExternalNameNtNtBR_8entities19UserExternalNameRefNtNtNtCs2AWtUsOyxgP_3std4hash6random11RandomStateE6insertBT_ (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x3b43967)
                #5  0x0000562dc862d17a _RNvMs2_NtNtNtCs2AWtUsOyxgP_3std11collections4hash3mapINtB5_7HashMapNtNtNtCsIgkLe5NID3_17cranelift_codegen2ir7extname16UserExternalNameNtNtB17_8entities19UserExternalNameRefE6insertB19_ (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x3d7c17a)
                #6  0x0000562dc85d63d2 _RNvMNtNtCsIgkLe5NID3_17cranelift_codegen2ir8functionNtB2_18FunctionParameters21ensure_user_func_name (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x3d253d2)
                #7  0x0000562dc85e6ace _RNvMs2_NtNtCsIgkLe5NID3_17cranelift_codegen2ir8functionNtB5_8Function30declare_imported_user_function (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x3d35ace)
                #8  0x0000562dc7e3f452 _RNvYNtNtCs5JSSEFRlFfD_13cranelift_jit7backend9JITModuleNtNtCs7zgrISooJwP_16cranelift_module6module6Module20declare_func_in_funcCs3xUwwK9peFn_5beamr (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x358e452)
                #9  0x0000562dc7e7939d _RNvNtNtNtCs3xUwwK9peFn_5beamr3jit8compiler10ir_helpers23declare_compile_helpers (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x35c839d)
                #10 0x0000562dc7e14b71 _RNvMNtNtNtCs3xUwwK9peFn_5beamr3jit8compiler8dispatchNtB4_11JitCompiler28compile_with_module_metadata (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x3563b71)
                #11 0x0000562dc7e143a0 _RNvMNtNtNtCs3xUwwK9peFn_5beamr3jit8compiler8dispatchNtB4_11JitCompiler22compile_with_signature (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x35633a0)
                #12 0x0000562dc7e17067 _RNvMNtNtNtCs3xUwwK9peFn_5beamr3jit8compiler8dispatchNtB4_11JitCompiler7compile (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x3566067)
                #13 0x0000562dc7ce2fc9 _RNvMs_NtNtCs3xUwwK9peFn_5beamr3jit11compile_jobNtB4_14CompilationJob3run (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x3431fc9)
                #14 0x0000562dc7cd9e40 _RNCNvNtNtCs3xUwwK9peFn_5beamr3jit11compile_job26try_submit_jit_compilation0B7_ (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x3428e40)
                #15 0x0000562dc7cdc636 _RNSNvYNCNvNtNtCs3xUwwK9peFn_5beamr3jit11compile_job26try_submit_jit_compilation0INtNtNtCs4NRVxsYgnAr_4core3ops8function6FnOnceuE9call_once6vtableBc_ (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x342b636)
                #16 0x0000562dc7bc7c1e _RNvXst_NtCscdodAO9FK5_5alloc5boxedINtB5_3BoxDINtNtNtCs4NRVxsYgnAr_4core3ops8function6FnOnceuEp6OutputuNtNtBO_6marker4SendEL_EIBI_uE9call_onceCs3xUwwK9peFn_5beamr (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x3316c1e)
                #17 0x0000562dc7cf68c8 _RNvNtNtCs3xUwwK9peFn_5beamr9scheduler5dirty11worker_loop (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x34458c8)
                #18 0x0000562dc7cd9896 _RNCNvMs0_NtNtCs3xUwwK9peFn_5beamr9scheduler5dirtyNtB7_9DirtyPool16with_queue_depth0Bb_ (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x3428896)
                #19 0x0000562dc7ef2026 _RINvNtNtCs2AWtUsOyxgP_3std3sys9backtrace28___rust_begin_short_backtraceNCNvMs0_NtNtCs3xUwwK9peFn_5beamr9scheduler5dirtyNtB1f_9DirtyPool16with_queue_depth0uEB1j_ (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x3641026)
                #20 0x0000562dc7fa87d3 _RNCNCINvNtNtCs2AWtUsOyxgP_3std6thread9lifecycle15spawn_uncheckedNCNvMs0_NtNtCs3xUwwK9peFn_5beamr9scheduler5dirtyNtB18_9DirtyPool16with_queue_depth0uEs_00B1c_ (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x36f77d3)
                #21 0x0000562dc7bfa1e1 _RNvXsl_NtNtCs4NRVxsYgnAr_4core5panic11unwind_safeINtB5_16AssertUnwindSafeNCNCINvNtNtCs2AWtUsOyxgP_3std6thread9lifecycle15spawn_uncheckedNCNvMs0_NtNtCs3xUwwK9peFn_5beamr9scheduler5dirtyNtB2i_9DirtyPool16with_queue_depth0uEs_00EINtNtNtB9_3ops8function6FnOnceuE9call_onceB2m_ (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x33491e1)
                #22 0x0000562dc7f996a1 _RINvNvNtCs2AWtUsOyxgP_3std9panicking12catch_unwind7do_callINtNtNtCs4NRVxsYgnAr_4core5panic11unwind_safe16AssertUnwindSafeNCNCINvNtNtB6_6thread9lifecycle15spawn_uncheckedNCNvMs0_NtNtCs3xUwwK9peFn_5beamr9scheduler5dirtyNtB2P_9DirtyPool16with_queue_depth0uEs_00EuEB2T_ (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x36e86a1)
                #23 0x0000562dc7fc2e7b __rust_try (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x3711e7b)
                #24 0x0000562dc7fa6a73 _RNCINvNtNtCs2AWtUsOyxgP_3std6thread9lifecycle15spawn_uncheckedNCNvMs0_NtNtCs3xUwwK9peFn_5beamr9scheduler5dirtyNtB16_9DirtyPool16with_queue_depth0uEs_0B1a_ (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x36f5a73)
                #25 0x0000562dc7cdc587 _RNSNvYNCINvNtNtCs2AWtUsOyxgP_3std6thread9lifecycle15spawn_uncheckedNCNvMs0_NtNtCs3xUwwK9peFn_5beamr9scheduler5dirtyNtB1b_9DirtyPool16with_queue_depth0uEs_0INtNtNtCs4NRVxsYgnAr_4core3ops8function6FnOnceuE9call_once6vtableB1f_ (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x342b587)
                #26 0x0000562dc92c473f _RNvNvMs0_NtNtNtCs2AWtUsOyxgP_3std3sys6thread4unixNtB7_6Thread3new12thread_start (/home/rocketfish/fleet/runs/seqlegs-impl-flight1/repo/target/debug/deps/recovery_declared_body_e2e-9990dace6ae08d82 + 0x4a1373f)
                #27 0x00007f09fb14897a n/a (libc.so.6 + 0x9697a)
                #28 0x00007f09fb1cc2bc n/a (libc.so.6 + 0x11a2bc)
                
```

## What that stack says, frame by frame

| # | frame | reading |
|---|---|---|
| 17 | `beamr::scheduler::dirty::worker_loop` | a beamr DIRTY-CPU worker thread, not a normal scheduler |
| 14–13 | `beamr::jit::compile_job::try_submit_jit_compilation::{closure}` → `CompilationJob::run` | the work item is a JIT compilation job |
| 12–10 | `beamr::jit::compiler::dispatch::JitCompiler::{compile, compile_with_signature, compile_with_module_metadata}` | inside beamr's cranelift driver, under the `Arc<Mutex<JITModule>>` guard taken at `beamr-0.20.0/src/jit/compiler/dispatch.rs` (`compile_with_module_metadata`, `lock_or_recover(self.module.as_ref())`) |
| 9 | `beamr::jit::compiler::ir_helpers::declare_compile_helpers` | `beamr-0.20.0/src/jit/compiler/ir_helpers.rs:33-44` — declaring `beamr_jit_alloc_tuple` and its siblings into a FRESH `ctx.func` |
| 8–6 | `cranelift_jit::JITModule::declare_func_in_func` → `Function::declare_imported_user_function` → `FunctionParameters::ensure_user_func_name` | cranelift-codegen 0.134.3 |
| 5–0 | `HashMap::<UserExternalName, UserExternalNameRef>::insert` → hashbrown `find_or_find_insert_index` → `reserve` → `reserve_rehash` | **SIGSEGV here** |

**The fault is a segmentation fault while GROWING A FRESH, EMPTY HASH TABLE.**
`ctx` comes from `jit_module.make_context()` two statements earlier, so the map
being grown was created microseconds before and holds at most a handful of
entries. Nothing about the insert itself can fault; the memory it reaches for
was already invalid. The JIT compile is where the damage SURFACES, not
necessarily where it is DONE.

## What this retires

`docs/evidence/venue/M12-beamr-heap-borrow-abort.md` records
`beamr-0.20.0/src/term/heap_borrow.rs:79` as an OPEN, intermittent,
non-unwinding ABORT. Both `.fleet/FINDINGS-SEQFIX.md` ("plausibly around the
beamr 0.20.0 floor") and `.fleet/plan.md` §S1/C9 nominated it as the leading
hypothesis for THIS crash. It is not:

- M12 is an **abort** (`thread caused non-unwinding panic. aborting.`) with a
  printed panic message; this is a silent **SIGSEGV**, signal 11, no message.
- M12's frame is `term/heap_borrow.rs:79` on `beamr-sched-0`; this frame is
  `jit/compiler/ir_helpers.rs` on a `scheduler::dirty` worker.
- M12 reproduces 1-in-3 under `--workspace` and 0-in-12 isolated; this
  reproduces **20/20 isolated** on the preserved artifact (D1).

M12's own closing finding — "**not caused by this flight**" — was right about
its own defect, and is right about this one too, for a different reason and
with a different receipt. See `docs/evidence/seqfix-sigsegv/CAUSE.md`.

## Thread census at the moment of the crash

```
64 threads total in the core:
  35  beamr scheduler::dirty::worker_loop      (dirty-CPU pool)
  17  beamr scheduler threads
   5  tokio multi_thread workers
   7  other (io_uring ring thread, test main, ...)
```

The test boots TWO full server generations in one process
(`crates/aion-server/tests/recovery_declared_body_e2e.rs`, module header:
generation 1 `build_with_store`, then generation 2 `build_with_store` over
the same database), so two beamr schedulers' pools are represented. It is
the workspace's heaviest concurrent exercise of this seam.
