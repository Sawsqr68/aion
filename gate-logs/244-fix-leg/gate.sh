#!/bin/zsh
# Server-run norn_dev gate for the #244 fix leg (Opus DO-NOT-LAND remediation).
# Derived from the #247-proven shape: JS deps installed as their own honest
# legs, fmt + BOTH clippy resolutions (workspace AND per-crate — the two
# resolve features differently and both are required), focused suites over
# every surface the slice touches (CLI tail, server children/transcripts,
# client subscribe, console), then the full standing battery.
set -u

WT=/Users/tom/Developer/ablative/stack/aion/.worktrees/244-child-visibility
LOGS=/Users/tom/Developer/ablative/stack/aion/gate-logs/244-fix-leg
mkdir -p "$LOGS" || { echo "gate setup FAILED: cannot create log directory $LOGS" >&2; exit 90; }

typeset -a LEG_RESULTS
typeset -A LEG_LOGS

run_leg() {
  local name=$1
  local logfile=$2
  shift 2
  (
    cd "$WT" || exit 91
    "$@"
  ) > "$LOGS/$logfile" 2>&1
  local code=$?
  echo "EXIT=$code" >> "$LOGS/$logfile"
  echo "$name=$code"
  LEG_RESULTS+=("$name=$code")
  LEG_LOGS[$name]=$logfile
}

per_crate_clippy() {
  local package_text
  package_text=$(cargo metadata --no-deps --format-version 1 |
    /usr/bin/python3 -c 'import json, sys; print("\n".join(sorted(p["name"] for p in json.load(sys.stdin)["packages"])))') || return $?
  local -a packages
  packages=("${(@f)package_text}")
  (( ${#packages[@]} > 0 )) || return 92

  local package code failures=0
  for package in "${packages[@]}"; do
    echo "=== cargo clippy -p $package --all-targets -- -D warnings ==="
    cargo clippy -p "$package" --all-targets -- -D warnings
    code=$?
    echo "PACKAGE_EXIT[$package]=$code"
    (( code != 0 )) && failures=1
  done
  return $failures
}

js_install() {
  local dir=$1
  (cd "$dir" && bun install)
}

console_leg() {
  local script=$1
  (cd apps/aion-ops-console && bun run "$script")
}

run_leg js-deps-console gate-js-deps-console.log js_install apps/aion-ops-console
run_leg js-deps-ts-worker gate-js-deps-ts-worker.log js_install sdks/typescript/aion-worker
run_leg fmt gate-fmt.log cargo fmt --all -- --check
run_leg clippy-workspace gate-clippy-workspace.log cargo clippy --workspace --all-targets -- -D warnings
run_leg clippy-per-crate gate-clippy-per-crate.log per_crate_clippy
run_leg aion-cli gate-aion-cli.log cargo test -p aion-cli --no-fail-fast
run_leg aion-server gate-aion-server.log cargo test -p aion-server --no-fail-fast
run_leg aion-client gate-aion-client.log cargo test -p aion-client --no-fail-fast
run_leg console-check gate-console-check.log console_leg check
run_leg console-typecheck gate-console-typecheck.log console_leg typecheck
run_leg console-test gate-console-test.log console_leg test
run_leg full-battery gate-full-battery.log /bin/bash "$WT/scripts/battery.sh"

typeset -a FAILED
for pair in "${LEG_RESULTS[@]}"; do
  (( ${pair##*=} != 0 )) && FAILED+=("$pair")
done
(( ${#FAILED[@]} == 0 )) && exit 0

{
  echo "GATE RED — ${#FAILED[@]} of ${#LEG_RESULTS[@]} legs failed:"
  for pair in "${FAILED[@]}"; do
    name=${pair%%=*}
    echo "  $name exited ${pair##*=}  (full output: $LOGS/${LEG_LOGS[$name]})"
  done
  echo "all leg results: ${LEG_RESULTS[*]}"
  echo "log directory: $LOGS"
} >&2
exit 1
