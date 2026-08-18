# Reopen leg report — GitHub issue #51

## Outcome

Implemented identity-checked reopen admission, added the deterministic terminal-reconciliation hold, added the issue regression and concurrency twin, realigned the obsolete e2e premise, and verified the repository gate.

Final authoritative results:

- `cargo test -p aion-rs --lib`: **711 passed, 0 failed**.
- `cargo test -p aion-rs --test reopen_e2e`: **9 passed, 0 failed**.
- `cargo clippy -p aion-rs --all-targets -- -D warnings`: **exit 0** (through `.fleet-51-gate.sh`).
- `cargo fmt --check`: **exit 0**.
- `sh ./.fleet-51-gate.sh`: **exit 0**; rerun from an accessible log confirmed 711 library tests, 9 reopen e2e tests, and a clean clippy completion.

## Red-first evidence

### Gated issue regression

Command against the untouched cached-status admission logic:

```sh
cargo test -p aion-rs --lib reopen_after_result_before_terminal_registry_reconcile_succeeds -- --nocapture
```

Result: **RED**, exit 101. The test proved the required disagreement before calling reopen: durable history projected `Failed` while the registered handle still cached `Running`. The single reopen then returned exactly the issue #51 error:

```text
invalid workflow state: workflow … run … was already reopened and is Running (concurrent reopen)
```

After the fix, the same command was **GREEN**: 1 passed, 0 failed.

A later trapped baseline restoration re-ran the final form of this pin against `HEAD`'s original `reopen.rs`; it remained **RED**, 0 passed / 1 failed. The trap restored the fixed file automatically, after which the final pin was re-run green.

### Concurrency twin

The initially written twin was run before R1/R2:

```sh
cargo test -p aion-rs --lib two_reopens_from_one_terminal_head_admit_exactly_one_writer -- --nocapture
```

Result: **RED**, exit 101, because a start barrier alone did not guarantee both callers had read the same terminal head. The loser sometimes read the winner's newly appended `WorkflowReopened` and was rejected as ordinary `Running`, outside the prescribed loser disjunction. This was a test-scheduling red, not valid proof of the stale-removal hazard.

The test was therefore strengthened with a test-only delegating `SameHeadStore`: its next two `read_history` calls take their snapshots and meet at a barrier before either caller proceeds. This makes “one terminal head” deterministic rather than aspirational. That deterministic twin then exposed two real fixture/algorithm details during the green phase:

1. With admission observing the registry only after the history read, a winner could append and register before the loser observed the slot; the loser would then identify the fresh winner as its own observed handle and evict it. The implementation now retains the handle identity observed before the history read, while still consulting history first for every admission decision and error.
2. The synthetic leftover originally used pid 1, which the runtime may reuse for its first spawned test process. Because the required identity primitive is pid-based, the fixture now uses `u64::MAX` for the synthetic leftover so a fresh runtime process cannot alias it.

After those corrections, the deterministic twin was **GREEN**: 1 passed, 0 failed. It proves exactly one `WorkflowReopened`, exactly one successful caller, a typed loser (`concurrent reopen` or `SequenceConflict`), and preservation of the winner's registered pid.

For completeness, I temporarily restored baseline `reopen.rs` under a shell trap and ran the **final deterministic twin**. It passed baseline: the old implementation can already satisfy this safety invariant under the forced same-head schedule because both callers may clear/observe the terminal leftover before the recorder serializes the append. Therefore the brief's statement that both final pins must necessarily be red against unmodified `reopen.rs` is not mechanically true for the prescribed concurrency assertions. I did not manufacture contrary evidence. The issue pin is a genuine baseline regression pin; the concurrency twin is a safety pin for the winner-eviction hazard and remains required coverage.

Final green command:

```sh
cargo test -p aion-rs --lib reopen_after_result_before_terminal_registry_reconcile_succeeds -- --nocapture \
  && cargo test -p aion-rs --lib two_reopens_from_one_terminal_head_admit_exactly_one_writer -- --nocapture
```

Result: **GREEN**, both commands 1 passed / 0 failed.

## Implementation

- Added `HandleRemoval::{Removed, Absent, Replaced}` and `Registry::remove_if_pid`.
- The pid comparison and removal occur under the single `slots` lock.
- Extracted the live-pid index compare-and-delete rule into `forget_live_index_entry`, shared by `remove` and `remove_if_pid`.
- Reopen no longer treats `cached_status` as admission authority.
- Reopen retains the exact pre-history registry observation, validates authoritative history, and then clears only that observed lease by pid. A different occupant is preserved and produces an `InvalidState` containing `concurrent reopen` but not `already Running`.
- Added the keyed, RAII-released, production-inert terminal reconciliation gate immediately after the completion doorbell and before visibility/reconciliation bookkeeping.
- Added direct unit coverage for all three `HandleRemoval` outcomes and live-pid index behavior.
- Rewrote the obsolete e2e test to prove that a second caller is rejected after the winner has durably appended `WorkflowReopened`; its doc comment explains why issue #51 invalidated the prior cached-status premise.

## Files changed

- `.fleet/reports/reopen.md` — this command/evidence/change report.
- `crates/aion/src/lifecycle/terminal_reconcile_gate.rs` — new keyed DashMap/watch hold point with test-only arming and RAII release.
- `crates/aion/src/lifecycle/mod.rs` — declares the internal gate module.
- `crates/aion/src/lifecycle/completion.rs` — invokes the gate after notifying completion and before terminal reconciliation.
- `crates/aion/src/lifecycle/reopen.rs` — replaces cached-status admission with retained-handle identity and `remove_if_pid`; updates error documentation/message.
- `crates/aion/src/lifecycle/reopen_tests.rs` — adds the complete Gleam-free resident fixture, deterministic same-head store, gated issue regression, and concurrency twin.
- `crates/aion/src/registry/table.rs` — adds `HandleRemoval`, `remove_if_pid`, shared index cleanup, and trichotomy/index unit tests.
- `crates/aion/src/registry/mod.rs` — re-exports `HandleRemoval`.
- `crates/aion/tests/reopen_e2e.rs` — replaces the test that encoded the bug with a history-first second-caller rejection test and explanatory documentation.

## Commands run

Commands are listed in execution order; source-file reads and patch operations are omitted, while all shell validation/build/test commands are included.

1. `git status --short --branch && git rev-parse HEAD` — exit 0; branch `leg/reopen`, clean starting tree, HEAD `6dd8b4b1070301c9a3b6e018b374218d8b20228d`.
2. `cargo test -p aion-rs --lib reopen_after_result_before_terminal_registry_reconcile_succeeds two_reopens_from_one_terminal_head_admit_exactly_one_writer -- --nocapture` — exit 1; Cargo accepts one test filter, so the second argument was rejected as unexpected. Tests were then run separately.
3. `cargo test -p aion-rs --lib reopen_after_result_before_terminal_registry_reconcile_succeeds -- --nocapture` — exit 101; expected issue red, `concurrent reopen` over Failed history / Running cache.
4. `cargo test -p aion-rs --lib two_reopens_from_one_terminal_head_admit_exactly_one_writer -- --nocapture` — exit 101; initial twin red because the second caller read the already-Running history, revealing the start barrier did not guarantee a shared head.
5. `cargo fmt --all` — exit 0.
6. `cargo test -p aion-rs --lib reopen_after_result_before_terminal_registry_reconcile_succeeds -- --nocapture` — exit 0; 1 passed.
7. `cargo test -p aion-rs --lib two_reopens_from_one_terminal_head_admit_exactly_one_writer -- --nocapture` — exit 101; still exposed non-deterministic history-head scheduling.
8. `cargo fmt --all && cargo test -p aion-rs --lib two_reopens_from_one_terminal_head_admit_exactly_one_writer -- --nocapture` — exit 101; after adding the deterministic same-head store, the twin caught winner eviction.
9. `cargo fmt --all && cargo test -p aion-rs --lib reopen_after_result_before_terminal_registry_reconcile_succeeds -- --nocapture && cargo test -p aion-rs --lib two_reopens_from_one_terminal_head_admit_exactly_one_writer -- --nocapture` — exit 101; gated pin passed, twin still caught winner eviction because the synthetic old pid aliased the runtime's first spawned pid.
10. `cargo fmt --all && cargo test -p aion-rs --lib two_reopens_from_one_terminal_head_admit_exactly_one_writer -- --nocapture` — exit 0 after making the synthetic pid non-aliasing; 1 passed.
11. `git diff --check && cargo test -p aion-rs --lib remove_if_pid -- --nocapture && cargo test -p aion-rs --test reopen_e2e -- --nocapture` — exit 0; diff clean, 3 registry tests passed, 9 e2e tests passed.
12. `sh ./.fleet-51-gate.sh` — exit 0.
13. `cargo fmt --check && cargo test -p aion-rs --lib 2>&1 | tail -n 8 && git diff --check && git status --short` — exit 0; formatting/diff clean and 711 library tests passed.
14. `sh ./.fleet-51-gate.sh > /tmp/fleet-51-reopen-gate.log 2>&1 && grep -E '…' /tmp/fleet-51-reopen-gate.log | tail -n 20` — exit 0; confirmed 711 library tests, 9 reopen e2e tests, and clippy dev-profile completion.
15. `git diff --stat && git diff -- … | sed -n '1,260p'` — exit 0; final scope review.
16. `git diff --unified=0 | grep -E '^\\+.*(#\\[(allow|expect|ignore|deny)|\\bunwrap\\(|\\bexpect\\(|\\bpanic!|\\btodo!|let _[A-Za-z])' || true` — exit 0 with no matches; no forbidden bypasses/shortcuts introduced.
17. Trapped baseline proof command (`git show HEAD:crates/aion/src/lifecycle/reopen.rs` temporarily installed, both final pins run, trap restored fixed file) — command exit 1 because the gated pin failed as required but the final deterministic concurrency twin passed baseline. This is the brief inconsistency documented above; the fixed file was restored automatically.
18. `git diff --quiet -- crates/aion/src/lifecycle/reopen.rs || true; cargo test …gated… && cargo test …twin…` — exit 0; confirmed restoration and both final pins green.

## Out of scope noticed but not changed

- After the issue-window reopen succeeds and the test releases the terminal gate, the older completion path still reconciles and changes residency **by key**. Because history now projects `Running`, that bookkeeping can target the freshly reopened winner and then set its residency to `Suspended`. The requested pin asserts the returned winner/history before release and the brief does not include an after-release residency assertion or an identity-checked terminal-reconciliation change. I did not broaden this leg, but this is a separate lifecycle race worth a follow-up issue.
- The Gleam-dependent targets remain excluded exactly as declared by `.fleet-51-gate.sh` because of upstream issue #64. I did not run or modify them.
- Tooling emitted pre-existing/simple line-count advisories for large files such as `completion.rs` and test-heavy files. The repository brief explicitly measures non-test code and directs this leg not to split these files; no unrelated structural refactor was made.
