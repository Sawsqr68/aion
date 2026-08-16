#!/bin/zsh
# #246 QUIET-BOX RE-RUN battery (Waffles, 2026-08-16).
# Decisive instrument per BATTERY-RED-ATTRIBUTION-PRESTAGE.md: the first battery
# ran on a contended box (3 batteries, load 36/24/20) at stale base 7109c1f6d;
# this run is on the REBASED tree (main e8c1b28bf) on a quiet box. HEAD is
# stamped into every log per the 247-leg2 template amendment.
set -u

WT=/Users/tom/Developer/ablative/stack/aion/.worktrees/246-adoption
LOGS=/Users/tom/Developer/ablative/stack/aion/gate-logs/246-adoption
HEAD_SHA=$(git -C "$WT" rev-parse HEAD) || exit 90

typeset -a LEG_CODES

run_leg() {
  local name=$1
  local logfile=$2
  shift 2
  echo "RE-GATE tree: $HEAD_SHA" > "$LOGS/$logfile"
  (
    cd "$WT" || exit 91
    "$@"
  ) >> "$LOGS/$logfile" 2>&1
  local code=$?
  echo "EXIT=$code" >> "$LOGS/$logfile"
  echo "$name=$code"
  LEG_CODES+=("$code")
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

run_leg fmt gate-fmt-rerun.log cargo fmt --all -- --check
run_leg clippy-workspace gate-clippy-workspace-rerun.log cargo clippy --workspace --all-targets -- -D warnings
run_leg clippy-per-crate gate-clippy-per-crate-rerun.log per_crate_clippy
run_leg full-battery gate-full-battery-rerun.log /bin/bash "$WT/scripts/battery.sh"

for code in "${LEG_CODES[@]}"; do
  (( code != 0 )) && exit 1
done
exit 0
