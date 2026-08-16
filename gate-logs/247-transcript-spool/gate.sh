#!/bin/zsh
# Server-run norn_dev gate for #247 transcript spool.
# Derived from BRIEF.md Verification floor: clippy ×2 + fmt, aion-worker,
# outage/recovery + observability server tests, and the full standing battery.
set -u

WT=/Users/tom/Developer/ablative/stack/aion/.worktrees/247-transcript-spool
LOGS=/Users/tom/Developer/ablative/stack/aion/gate-logs/247-transcript-spool
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

# A fresh worktree carries no node_modules; the battery's five JS gates
# (console ×2, ts-worker ×3) need their dependencies installed first or
# every one fails Cannot-find-module — the exact face that reddened the
# #246 gate. Own legs so a failed install is its own honest red.
js_install() {
  local dir=$1
  (cd "$dir" && bun install)
}

run_leg js-deps-console gate-js-deps-console.log js_install apps/aion-ops-console
run_leg js-deps-ts-worker gate-js-deps-ts-worker.log js_install sdks/typescript/aion-worker
run_leg fmt gate-fmt.log cargo fmt --all -- --check
run_leg clippy-workspace gate-clippy-workspace.log cargo clippy --workspace --all-targets -- -D warnings
run_leg clippy-per-crate gate-clippy-per-crate.log per_crate_clippy
run_leg aion-worker gate-aion-worker.log cargo test -p aion-worker --no-fail-fast
run_leg outage-recovery-observability gate-outage-recovery-observability.log cargo test -p aion-server --no-fail-fast
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
