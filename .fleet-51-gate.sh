#!/bin/sh
# fleet-51 landing gate — GitHub issue #51 of ablative-io/aion:
# "Reopen misreports a single reopen as concurrent when terminal reconciliation
#  lags the result future".
#
# POSIX sh. Run from the repository root:   sh ./.fleet-51-gate.sh
# Exits 0 only if every step below passes. On any failure it names the failing
# command and exits nonzero.
#
# ---------------------------------------------------------------------------
# THE TOOLCHAIN PATH IS BOUND EXPLICITLY, ON PURPOSE.
#
# Tonight's earlier gate went red for a reason that had nothing to do with the
# code: it ran with an unbound PATH and `cargo` was simply not on it. cargo
# lives under $HOME/.cargo/bin on this box, so that directory is prepended here
# rather than assumed to be inherited from the invoking shell's profile. A gate
# whose red means "the harness could not find the compiler" is worse than no
# gate, because it is indistinguishable from a real failure. The `command -v`
# probe below turns that class of red into an explicit, self-describing one.
# ---------------------------------------------------------------------------
PATH="$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin"
export PATH

if ! command -v cargo >/dev/null 2>&1; then
    echo "FAIL: cargo not found on PATH ($PATH)" >&2
    echo "failing command: command -v cargo" >&2
    exit 1
fi

# ===========================================================================
# DECLARED EXCLUSION — gleam-dependent e2e targets, excluded BY NAME.
#
# WHY: the box toolchain is gleam 1.18.1 / OTP 27. That pair emits a
# gleam_stdlib beam which beamr's module validator refuses to load. Measured at
# HEAD bb3b7705 on this box, unmodified tree:
#
#   Load { reason: "runtime rejected deployed module
#   `gleam@dynamic@decode$691ac8d3...` after 31 staged registrations: runtime
#   error: BEAM module validation failed: instruction 392: Y register index 5
#   is outside frame size 1" }
#
# That is issue #64, the fix is owned UPSTREAM in beamr, and it is entirely
# unrelated to issue #51. These targets are covered by the Mac landing battery,
# which has a gleam/OTP pair beamr accepts.
#
# 🔴 `activity_retry_e2e` IS ONE OF THEM, AND IS THEREFORE EXCLUDED.
#
# It is named separately because the issue-#51 investigation cites it
# (crates/aion/tests/activity_retry_e2e.rs:224 — the result-then-reopen
# sequence), so its absence here must not read as an oversight. Measured at
# HEAD on this box it is RED BEFORE ANY CHANGE IS MADE: 8 passed, 3 failed —
#   exhausted_retries_fail_the_run_and_reopen_continues_the_trail
#   in_process_dispatch_miss_is_typed_recorded_and_fails_the_run
#   retryable_flakes_cost_retries_not_the_run
# all three dying on the #64 validator error above. Note that
# `gleam_test_support::skip_if_unavailable()` does NOT rescue them: it only
# probes `gleam --version`, gleam IS installed here, so the skip never fires and
# the run proceeds into beamr's refusal. Putting it in the gate would pin the
# gate permanently red for a cause this change cannot fix and cannot regress.
#
# The other gleam-dependent targets, excluded by name for the same #64 cause:
#   adoption_parked_timer_e2e            adoption_parked_timer_xnode_e2e
#   aion_active_active_showcase          aion_cluster_failover_showcase
#   aion_on_haematite_showcase           awl_bc4_differential
#   awl_retry_counting_e2e               child_await_query_e2e
#   deploy_persistence_e2e               direct_failure_parity_e2e
#   direct_multi_step_distribute_e2e     engine_reload
#   engine_reload_recovery               engine_reload_stress
#   example_query_reentry                examples_e2e
#   hello_world_e2e                      multi_step_distribute_e2e
#   nested_implicit_child_decode_e2e     nested_workflows_e2e
#   order_saga_e2e                       recovery_e2e
#   reopen_timer_rearm_e2e               sdk_internal_privacy
#   sdk_sleep_query_e2e                  settled_fanout_e2e
#   ss5_failover_demo                    two_awl_workflows_e2e
#   workflow_timeout_e2e
#
# WHAT REPLACES IT: `reopen_e2e` (step 2 below). It is the reopen regression
# battery — it owns `reopen_clears_the_lingering_handle_of_a_run_that_failed_
# while_resident` and `reopen_still_rejects_a_genuinely_running_concurrent_
# reopen`, the two tests this change's admission rewrite touches most directly.
# It is gleam-FREE: it builds its package from the checked-in Erlang-compiled
# fixture crates/aion/tests/fixtures/aion_fixture_workflow.beam, so beamr's
# validator accepts it. Measured green at HEAD on this box: 9 passed in 0.54s.
# ===========================================================================

run_step() {
    step_name="$1"
    shift
    echo "=== $step_name ==="
    echo "+ $*"
    if ! "$@"; then
        echo >&2
        echo "FAIL: $step_name" >&2
        echo "failing command: $*" >&2
        exit 1
    fi
    echo "ok: $step_name"
    echo
}

# Step 1 — the in-lib unit tests. THIS is the step that adjudicates the fix.
# The two issue-#51 pins live in-lib, in crates/aion/src/lifecycle/reopen_tests.rs:
#   reopen_after_result_before_terminal_registry_reconcile_succeeds
#   two_reopens_from_one_terminal_head_admit_exactly_one_writer
# They drive a real terminal transition and a real reopen with no gleam at all.
# Baseline at HEAD on this box: 706 passed, 0 failed.
run_step "aion-rs lib tests" \
    cargo test -p aion-rs --lib

# Step 2 — the gleam-free reopen regression battery (see the exclusion note).
# Baseline at HEAD on this box: 9 passed, 0 failed.
run_step "aion-rs reopen_e2e" \
    cargo test -p aion-rs --test reopen_e2e

# Step 3 — lints. Repository law: clippy must be clean at -D warnings, and
# #[allow]/#[expect]/#[ignore]/_var renames are bypasses, not fixes.
run_step "aion-rs clippy (-D warnings)" \
    cargo clippy -p aion-rs --all-targets -- -D warnings

echo "PASS: all gate steps succeeded"
exit 0
