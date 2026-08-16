#!/bin/zsh
# Server-run norn_dev gate for libsql-retirement.
# Derived from BRIEF.md Battery: workspace tests, aion-server integrations,
# clippy ×2, tree-wide fmt, and the GETTING-STARTED documentation gate.
set -u

WT=/Users/tom/Developer/ablative/stack/aion/.worktrees/libsql-retirement
LOGS=/Users/tom/Developer/ablative/stack/aion/gate-logs/libsql-retirement
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

run_leg fmt gate-fmt.log cargo fmt --all -- --check
run_leg clippy-workspace gate-clippy-workspace.log cargo clippy --workspace --all-targets -- -D warnings
run_leg clippy-per-crate gate-clippy-per-crate.log per_crate_clippy
run_leg workspace-tests gate-workspace-tests.log cargo test --workspace --no-fail-fast
run_leg aion-server-integrations gate-aion-server-integrations.log cargo test -p aion-server --tests --no-fail-fast
run_leg doc-pointer gate-doc-pointer.log cargo test -p aion-cli --test doc_pointer_gate

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
