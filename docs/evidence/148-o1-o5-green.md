# Issue #148 O1–O5 observed green evidence

Date: 2026-08-23. Shell controls ran under `/bin/sh` (dash).

## O3 Arm A / O5 reconstruction — state-area claim, synthetic non-ignoring tree

Failing world named by the control: the engine claim is placed in the synthetic worktree, making honest fanin report it as untracked dirt. The committed red is `docs/evidence/148-o3-red.md`.

Observed after relocation:

```text
PRECONDITION: synthetic tree does not ignore .aion-held-execution.json; deployed scripts are byte-identical
PASS arm-a: process_rc=0 payload_rc=0 claim=/home/aion/fleet/runs/marker-location-selfcheck-3505439/state/spawn-claims/858719ab380cbc0b0eed7efafc7dccd64cf475302bf30a3e35e93e2504aa09e5/.aion-held-execution.json
control_rc=0 (expected exact 0)
```

The claim filename is unchanged and the observed claim path is outside `repo`; fanin's shell process and JSON payload both returned exact 0.

## O3 Arm B — stray in-tree copy still refuses

Failing world named by the control: a door-1 filename exemption filters real dirt. The committed mutant red observed payload 0 where exact payload 2 was required.

Observed with the honest fanin script:

```text
PRECONDITION: synthetic tree does not ignore .aion-held-execution.json; deployed scripts are byte-identical
PASS arm-b: process_rc=0 payload_rc=2
control_rc=0 (expected exact 0)
```

The wrapper asserts fanin process rc exact 0 and payload rc exact 2.

## Confounded demonstration

An otherwise identical synthetic tree whose `.gitignore` lists the marker is not evidence in either direction:

```text
CONFOUNDED: synthetic marker check-ignore rc=0, expected exact rc 1
control_rc=42 (expected exact 42)
```

This arm is reported only as **CONFOUNDED**, neither red nor green.

## Focused Rust behavior suite

Failing worlds are stated in each test's source comment: alias/restart key divergence; missing-home fallback; global or weakened exclusion; drop leakage; lost same-execution reattach; dead-holder wedge; PID-only identity checks; live-holder fail-open; and wrong-tree marker deletion.

```text
cargo test -p aion-integration-norn --lib --no-fail-fast
running 100 tests
...
test claim_area::tests::aliases_and_restarts_derive_one_stable_claim_directory ... ok
test spawn_guard::tests::dead_and_pid_reused_incarnations_are_cleaned_at_the_new_location ... ok
test spawn_guard::tests::matching_live_incarnation_refuses_and_tree_mismatch_is_loud_and_preserved ... ok
test spawn_guard::tests::same_execution_reattaches_without_deleting_the_live_marker ... ok
test spawn_guard::tests::live_exclusion_drop_release_and_per_tree_scope_are_preserved ... ok
test result: ok. 100 passed; 0 failed
```

The restart-stability arm launches two separate test processes, asserts their PIDs differ, and asserts byte-identical claim directories. The symlink arm constructs its own symlink and asserts it is a symlink and differs from the canonical spelling before comparing keys.
