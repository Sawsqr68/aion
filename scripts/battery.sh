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
gate "rust:test" cargo test --workspace

# The all-features resolution differs from the default one — `--workspace`
# resolves some features ON that `-p <crate>` leaves off — so this is a
# genuinely different run, not a slower copy of the one above.
gate "rust:test-all-features" cargo test --workspace --all-features

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
