# Fix rounds

## 2026-08-20 — RED round 1: bounded all-target build artifacts

The gate reached the workspace integration-test link stage after compiling the full target matrix, then failed to write Cargo dependency metadata with `No space left on device (os error 28)`. The concurrent `rust-lld` jobs subsequently terminated with bus errors. These were consequences of the 98.5 GiB generated `target/` tree exhausting the landing-desk filesystem, not test assertions or Rust diagnostics.

Changed the root Cargo profiles so both development/all-target compilation and test compilation omit debug information and incremental caches. Neither setting removes a workspace member, target, lint, or test; they reduce only generated artifact volume while preserving the gate's exact commands and exercised code.

Source fix: commit `70adba5d` (`fix(gate): bound all-target build artifacts`). Cargo accepted the profile configuration, `cargo fmt --all --check` completed, and `cargo test --workspace --no-run` compiled every workspace test executable in 4m 16s. The rebuilt `target/` tree was 14 GiB rather than the pre-fix 98.5 GiB. This local compilation verifies the named storage failure at its source; the fleet gate remains the authority for the next measured result.
