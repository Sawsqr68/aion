#!/usr/bin/env bash
#
# The house battery: every gate this repository has, in one command.
#
# WHY THIS EXISTS
#
# GitHub Actions is disabled on this repository, so no workflow in
# `.github/workflows/` has ever run. That turned out to matter three separate
# times, each the same shape — a correctly designed gate that executed nowhere:
#
#   * the all-features Rust gate, which existed and had never run;
#   * `wire-types-no-diff`, whose comment shows its author saw the exact trap it
#     was guarding and designed around it — in a lane that is off;
#   * the Python and TypeScript SDK proto-drift gates, which are the REMEDIATION
#     for an incident where both worker SDKs went wire-dead with green suites.
#
# Before this script there was no justfile, no Makefile, and no runnable
# equivalent, so "the battery" was whatever each person remembered to type. The
# Rust battery is what people type, and `cargo test --workspace` does not run
# pytest, does not run bun, and does not typecheck the console. Every gate
# outside Rust was therefore load-bearing and unexecuted.
#
# HOW IT REPORTS
#
# Every gate runs even if an earlier one fails — a battery that stops at the
# first failure hides the other three. Each gate's exit code is printed in a
# manifest at the end, and the script exits non-zero if ANY gate failed.
#
# A MISSING TOOLCHAIN IS A FAILURE, NOT A SKIP. A skip is how a gate becomes
# invisible, which is the exact defect this script exists to fix. If `uv` or
# `bun` is absent the battery says so and goes red; install them or run the
# gates you can and say which ones in your report.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT" || exit 1

# Gate name → exit code, in declaration order. Bash 3.2 (macOS system bash) has
# no associative arrays, so this is two parallel arrays rather than one map.
GATE_NAMES=()
GATE_CODES=()

# Runs one gate, records its exit code, and never lets a failure stop the run.
#
# `set +e` around the invocation is deliberate: `set -e` is NOT set for this
# script precisely so a red gate cannot abort the battery, and this makes that
# explicit at the call site rather than relying on it.
gate() {
  local name="$1"
  shift
  printf '\n=== %s ===\n' "$name"
  "$@"
  local code=$?
  GATE_NAMES+=("$name")
  GATE_CODES+=("$code")
  # `printf` is given the leading dashes as an ARGUMENT, not inside the format
  # string: bash's printf reads a format beginning with `--` as an option
  # terminator and refuses the call.
  printf '%s %s exit %d %s\n' '---' "$name" "$code" '---'
  return 0
}

# Runs a gate inside a directory without leaking the `cd` into later gates.
#
# The `cd` happens in a subshell rather than via `env -C`, which BSD `env` did
# not carry until recently and which would fail on an older macOS with a
# message that looks nothing like the real problem.
gate_in() {
  local name="$1"
  local dir="$2"
  shift 2
  if [ ! -d "$dir" ]; then
    printf '\n=== %s ===\n' "$name"
    printf 'MISSING DIRECTORY: %s\n' "$dir"
    GATE_NAMES+=("$name")
    GATE_CODES+=(127)
    return 0
  fi
  gate "$name" bash -c 'cd "$1" && shift && "$@"' _ "$dir" "$@"
}

# Fails the battery loudly when a required tool is absent, rather than skipping.
require_tool() {
  local tool="$1"
  local why="$2"
  if command -v "$tool" >/dev/null 2>&1; then
    return 0
  fi
  printf '\n=== toolchain: %s ===\n' "$tool"
  printf 'NOT INSTALLED. %s\n' "$why"
  printf 'This is a FAILURE, not a skip: a gate that cannot run is a gate that
proves nothing, and silently skipping it is how three gates in this repository
ended up correct and unexecuted.\n'
  GATE_NAMES+=("toolchain:$tool")
  GATE_CODES+=(127)
  return 1
}

printf 'aion house battery — repo root %s\n' "$REPO_ROOT"
printf 'HEAD %s\n' "$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"

# The battery's own failure path, executable.
#
# A reporting harness whose red path has never run is the same defect as a gate
# whose assert half has never run — and this repository has now produced three
# of those. `AION_BATTERY_SELF_TEST=1 scripts/battery.sh` runs one passing and
# one failing synthetic gate and MUST exit 1 with `1 of 2 gates failed`. If it
# ever exits 0, this script cannot report a real failure either.
if [ "${AION_BATTERY_SELF_TEST:-}" = "1" ]; then
  printf 'SELF-TEST MODE: no real gate runs. Expect exit 1 and 1 of 2 failed.\n'
  gate "self-test:passes" true
  gate "self-test:fails" false
else

# --- Rust ---------------------------------------------------------------------

gate "rust:fmt" cargo fmt --all -- --check
gate "rust:clippy" cargo clippy --workspace --all-targets -- -D warnings

# `--no-fail-fast` IS LOAD-BEARING, and its absence cost a P0 on 2026-07-31.
#
# This script already refuses to stop at the first failing GATE, for the reason
# in the header: a battery that stops early hides the gates behind it. Without
# this flag the Rust gates broke that same rule one level down — `cargo test`
# stops at the first failing TARGET, so a single red lib test silently un-runs
# every integration target after it.
#
# That is not hypothetical. A red lib test in `aion-server` truncated a run
# before any integration target started, and the reading — "688 passed, 1
# failed" — was taken as one known exception on an otherwise green suite. It was
# a truncated run reported as a complete one, and among the targets that never
# executed was one that would have HUNG FOREVER on a P0 introduced the day
# before (every liminal worker accepting dispatches and executing none). See
# docs/design/aion-authoring/RED-MAIN-118-RECORD.md.
#
# A count is only a coverage claim if the run reached the end. Read the target
# list, not the tally.
# Runs a cargo test invocation and asserts the run REACHED THE END (#120).
#
# A test run's exit code and its tally are two different claims, and only the
# first is about completeness. `--no-fail-fast` continues past failing tests, but
# a target that dies — or a run cut short — simply stops producing output, and
# the surviving tally still reads `0 failed`. That is how "688 passed, 1 failed"
# was taken as one known exception on an otherwise green suite when it was in
# fact a truncated run reported as a complete one, hiding a P0 (see the comment
# above `rust:test`).
#
# 🔴 THE OBVIOUS CONTROL — comparing the COUNT of target-start lines against the
# count of `test result:` lines — DOES NOT WORK, and this was measured rather
# than assumed. On fully green runs the two disagree:
#
#     .evidence/118/battery-final.log        45 targets, 48 results
#     .evidence/118/shakedown-allfeatures    240 targets, 243 results
#
# Because a test that RE-EXECUTES ITS OWN BINARY as a child prints the child's
# `running N tests` / `test result:` into the parent's stdout, mid-target — e.g.
# `config::load::home_tests::aion_home_env_probe_child` from the AION_HOME work.
# A counting gate would fire on every green run; worse, a real truncation
# coinciding with N child extras would net to zero and read as complete.
#
# So assert the PAIRING, not the count: every target-start line must be followed
# by at least one `test result:` before the next target line. Child extras land
# inside a target's own block and are tolerated by construction; a truncation
# leaves a started target with nothing after it.
#
# Validated both ways before being wired in: `unpaired=0` on three real green
# logs (including both counting-mismatch cases above) and `unpaired=1` on the
# same log with one result line removed.
#
# Note the filter trap: a legitimately filtered top-level run also reports
# `filtered out > 0`, so that field cannot be used to identify child lines — the
# `posture:*` gates filter deliberately.
paired_test() {
  local log
  log=$(mktemp) || return 1
  "$@" 2>&1 | tee "$log"
  local code=${PIPESTATUS[0]}

  local counts targets unpaired
  counts=$(awk '
    /^[[:space:]]+Running |^[[:space:]]+Doc-tests / { if (started) unpaired++; started = 1; targets++ }
    /^test result:/ { started = 0 }
    END { if (started) unpaired++; print targets + 0, unpaired + 0 }
  ' "$log")
  targets=${counts%% *}
  unpaired=${counts##* }
  rm -f "$log"

  if [ "$targets" -eq 0 ]; then
    printf 'COVERAGE CONTROL FAILED: the run started ZERO test targets.\n'
    printf 'An exit code from a run that tested nothing is not a result about this code.\n'
    return 1
  fi
  if [ "$unpaired" -ne 0 ]; then
    printf 'COVERAGE CONTROL FAILED: %d of %d target(s) started and never reported a result.\n' \
      "$unpaired" "$targets"
    printf 'The run was TRUNCATED. Its tally is not a coverage claim, whatever it says.\n'
    return 1
  fi
  printf 'coverage control ok: %d targets started, %d reached a result\n' "$targets" "$targets"
  return "$code"
}

gate "rust:test" paired_test cargo test --workspace --no-fail-fast

# The all-features resolution differs from the default one — `--workspace`
# resolves some features ON that `-p <crate>` leaves off — so this is a
# genuinely different run, not a slower copy of the one above.
gate "rust:test-all-features" paired_test cargo test --workspace --all-features --no-fail-fast

# --- Build-posture controls (#119) ---------------------------------------------
#
# Two crates carry a default-OFF feature that gates an entire surface, so a
# per-package run of either compiles none of it and exits 0:
#
#   aion-worker / liminal-transport  — the whole liminal transport
#   aion-proto  / generated          — the tonic/prost stubs the hand-written
#                                      wire contracts are supposed to be
#                                      checked AGAINST
#
# Both crates now ANNOUNCE their posture through a test name, which a runner
# always prints. But `reconcile-nextest.sh` states the law this would otherwise
# break: *a pinned test denominator that nothing mechanically compares is an
# instruction, not a control.* A printed announcement that nothing checks is
# observability, not a gate — a run where the wrong arm fires reconciles
# perfectly, because every count still adds up.
#
# So the gate asserts BOTH directions: the posture-appropriate name appeared,
# AND its opposite did not. That is also what anchors the negative arms — they
# assert little on their own, and deleting one would otherwise go unnoticed.
posture_control() {
  local label="$1" expect="$2" forbid="$3"
  shift 3
  local out code
  out=$("$@" 2>&1)
  code=$?
  if [ "$code" -ne 0 ]; then
    printf '%s\n' "$out"
    printf 'POSTURE CONTROL (%s): the run itself failed with exit %d\n' "$label" "$code"
    return "$code"
  fi
  if ! printf '%s' "$out" | grep -q -- "$expect"; then
    printf 'POSTURE CONTROL FAILED (%s): expected announcement `%s` never appeared.\n' "$label" "$expect"
    printf 'The build did not have the feature posture this gate believes it ran.\n'
    return 1
  fi
  if printf '%s' "$out" | grep -q -- "$forbid"; then
    printf 'POSTURE CONTROL FAILED (%s): forbidden announcement `%s` appeared.\n' "$label" "$forbid"
    printf 'This build is NOT the posture the gate believes it is.\n'
    return 1
  fi
  printf 'posture control ok (%s): %s\n' "$label" "$expect"
  return 0
}

gate "posture:worker-default" posture_control "aion-worker, default features" \
  liminal_transport_absent_from_this_build \
  liminal_transport_compiled_here \
  cargo test -p aion-worker --lib build_coverage::

gate "posture:worker-liminal" posture_control "aion-worker, liminal-transport on" \
  liminal_transport_compiled_here \
  liminal_transport_absent_from_this_build \
  cargo test -p aion-worker --lib --features liminal-transport build_coverage::

gate "posture:proto-default" posture_control "aion-proto ALONE, default features" \
  generated_stubs_absent_from_this_build \
  generated_stubs_compiled_into_this_build \
  cargo test -p aion-proto --lib build_coverage::

gate "posture:proto-generated" posture_control "aion-proto, generated on" \
  generated_stubs_compiled_into_this_build \
  generated_stubs_absent_from_this_build \
  cargo test -p aion-proto --lib --features generated build_coverage::

# --- Ops console --------------------------------------------------------------

if require_tool bun "The ops console is a separate bun project; without it the console's typecheck and tests cannot run."; then
  gate_in "console:typecheck" apps/aion-ops-console bun run typecheck
  gate_in "console:test" apps/aion-ops-console bun test
fi

# --- Python SDKs --------------------------------------------------------------
#
# These carry the proto-drift gates that are the remediation for the wire-dead
# SDK incident. `cargo test` does not run them; nothing did until this script.

if require_tool uv "The Python SDKs and their proto-drift gates are uv projects."; then
  gate_in "python:aion-worker" sdks/python/aion-worker uv run --extra dev python -m pytest tests/ -q
  gate_in "python:aion-client" sdks/python/aion-client uv run --extra dev python -m pytest tests/ -q
fi

# --- TypeScript SDKs ----------------------------------------------------------

if command -v bun >/dev/null 2>&1; then
  gate_in "ts-worker:proto-drift" sdks/typescript/aion-worker bun run check:proto
  gate_in "ts-worker:typecheck" sdks/typescript/aion-worker bun run typecheck
  gate_in "ts-worker:test" sdks/typescript/aion-worker bun test
fi

fi # end of the real-gates branch

# --- Manifest -----------------------------------------------------------------

printf '\n================ BATTERY MANIFEST ================\n'
failed=0
index=0
while [ "$index" -lt "${#GATE_NAMES[@]}" ]; do
  name="${GATE_NAMES[$index]}"
  code="${GATE_CODES[$index]}"
  if [ "$code" -eq 0 ]; then
    printf '  PASS  %-28s exit %s\n' "$name" "$code"
  else
    printf '  FAIL  %-28s exit %s\n' "$name" "$code"
    failed=$((failed + 1))
  fi
  index=$((index + 1))
done
printf '=================================================\n'

if [ "${#GATE_NAMES[@]}" -eq 0 ]; then
  printf 'NO GATES RAN. That is a failure of this script, not a clean tree.\n'
  exit 1
fi

printf '%d of %d gates failed.\n' "$failed" "${#GATE_NAMES[@]}"
if [ "$failed" -ne 0 ]; then
  exit 1
fi
exit 0
