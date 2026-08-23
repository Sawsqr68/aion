# Issue #148 real-binary and gate observations

Date: 2026-08-23.

## Cross-process reattach — real Norn binary

Command:

```text
NORN_BIN=/home/aion/.cargo/bin/norn cargo test -p aion-integration-norn --test cross_process_reattach_e2e -- --nocapture
```

Observed after explicitly pinning the helper's isolated `AION_HOME`:

```text
test replacement_worker_reattaches_the_surviving_norn_session ... ok
test result: ok. 1 passed; 0 failed
```

This was not the runtime-gated silent skip.

## issue36 known downstream failure — marker lookup succeeded

Command used the same real `NORN_BIN`. The run completed with `reply: adopted` and the relocated marker was read successfully. It then reached the pre-existing, out-of-scope assertion:

```text
assertion `left == right` failed: recovery must record exactly one adoption offer for the dangling attempt
left: 0
right: 1
```

Observed test result: exact process exit 101, 0 passed, 1 failed. This is the base failure named in the builder instructions and was not changed.

## Authoritative gate

`/bin/sh .fleet/gate-entry.sh` passed venue/toolchain, wall, deliverable, formatting, and this crate's clippy surface. The full workspace clippy step then stopped on an unrelated pre-existing file outside this flight's wall:

```text
crates/aion/src/runtime/nif_determinism.rs:734:5
clippy::too_many_lines: live_and_replayed_activity_serve_identical_now_sequences (107/100)
GATE-RED: clippy
```

`git diff` for that file is empty in this leg. Focused verification is green:

```text
cargo clippy -p aion-integration-norn --all-targets -- -D warnings
Finished `dev` profile ...
```
