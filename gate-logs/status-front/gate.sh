#!/bin/zsh
# Server-run norn_dev gate for the status-truth front (F1/F2/F3 fix).
# Every leg log opens with the tree HEAD (self-attributing logs).
# Closure: status projection = engine (aion-rs) + server (aion-server) +
# store crates; battery scope follows the closure, so the full standing
# battery is the final leg.
set -u

WT=/Users/tom/Developer/ablative/stack/aion/.worktrees/status-truth
LOGS=/Users/tom/Developer/ablative/stack/aion/gate-logs/status-front
mkdir -p "$LOGS" || { echo "gate setup FAILED: cannot create log directory $LOGS" >&2; exit 90; }

typeset -a LEG_RESULTS
typeset -A LEG_LOGS

run_leg() {
  local name=$1
  local logfile=$2
  shift 2
  echo "HEAD=$(git -C "$WT" rev-parse HEAD)" > "$LOGS/$logfile"
  (
    cd "$WT" || exit 91
    "$@"
  ) >> "$LOGS/$logfile" 2>&1
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

# A fresh worktree carries no node_modules; the battery's JS gates need
# their dependencies installed first — own legs so a failed install is
# its own honest red.
js_install() {
  local dir=$1
  (cd "$dir" && bun install)
}

run_leg js-deps-console gate-js-deps-console.log js_install apps/aion-ops-console
run_leg js-deps-ts-worker gate-js-deps-ts-worker.log js_install sdks/typescript/aion-worker
run_leg fmt gate-fmt.log cargo fmt --all -- --check
run_leg clippy-workspace gate-clippy-workspace.log cargo clippy --workspace --all-targets -- -D warnings
run_leg clippy-per-crate gate-clippy-per-crate.log per_crate_clippy
run_leg aion-server gate-aion-server.log cargo test -p aion-server --no-fail-fast
run_leg aion-rs gate-aion-rs.log cargo test -p aion-rs --no-fail-fast
run_leg stores gate-stores.log cargo test -p aion-store -p aion-store-haematite -p aion-store-libsql --no-fail-fast
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
