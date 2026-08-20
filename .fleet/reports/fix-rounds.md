# Fix rounds

## 2026-08-20 — RED round 1: bounded all-target build artifacts

The gate reached the workspace integration-test link stage after compiling the full target matrix, then failed to write Cargo dependency metadata with `No space left on device (os error 28)`. The concurrent `rust-lld` jobs subsequently terminated with bus errors. These were consequences of the 98.5 GiB generated `target/` tree exhausting the landing-desk filesystem, not test assertions or Rust diagnostics.

Changed the root Cargo profiles so both development/all-target compilation and test compilation omit debug information and incremental caches. Neither setting removes a workspace member, target, lint, or test; they reduce only generated artifact volume while preserving the gate's exact commands and exercised code.

Source fix: commit `70adba5d` (`fix(gate): bound all-target build artifacts`). Cargo accepted the profile configuration, `cargo fmt --all --check` completed, and `cargo test --workspace --no-run` compiled every workspace test executable in 4m 16s. The rebuilt `target/` tree was 14 GiB rather than the pre-fix 98.5 GiB. This local compilation verifies the named storage failure at its source; the fleet gate remains the authority for the next measured result.

## 2026-08-20 — RED round 2: release engine-task runtime descriptors

The gate's `aion-server` library target exhausted the process file-descriptor limit. The first engine construction failures reported `failed to start the engine-task runtime: Too many open files (os error 24)`, after which unrelated filesystem and database tests cascaded because they could no longer open files.

The source leak was `Engine::drop`: it gated and aborted the engine-task epoch with `begin_close`, but deliberately left the epoch's Tokio runtime and I/O driver owned by the retained runtime handle. Server tests construct and release many engines without explicit shutdown, so each released engine kept those descriptors for the rest of the test process. `Engine::drop` now calls the existing joined `EngineTaskRuntime::shutdown` path, which is safe from an async host because the Tokio runtime is dropped on a plain joiner thread. A focused regression holds the epoch handle across `drop(engine)` and asserts that the runtime slot is empty afterward.

The touched oversized inline test modules were extracted into ordinary submodules so every modified or added Rust file remains below 500 lines of code; no tests, lints, targets, or runtime checks were suppressed. Source fix: commit `0f3aff9d` (`fix(gate): release engine task descriptors on drop`). Focused Engine Drop and engine-task lifecycle tests completed, and `cargo clippy -p aion-rs --lib --tests -- -D warnings` completed. A diagnostic run of the full server library target under an explicit 1024-descriptor ceiling improved from 829 passed / 327 failed before the reachable-drop fix to 1129 passed / 27 failed afterward; that run is stress evidence only, not a gate result. The fleet gate remains the authority for the next measured result.
