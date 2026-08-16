#!/bin/zsh
# Server-run norn_dev gate for the manifold #210 backpressure lane.
# Manifold's own Rust-side house legs (the K9-proven pair for crates/
# changes): full workspace tests + the loc gate. Waffles' desk re-runs
# merged gates and walks the scratch estate before landing — this gate
# refuses early, it does not replace his.
set -u

WT=/Users/tom/Developer/ablative/apps/manifold/.worktrees/m210-attach-backpressure
LOGS=/Users/tom/Developer/ablative/stack/aion/gate-logs/m210-attach-backpressure
mkdir -p "$LOGS" || exit 90

typeset -a LEG_CODES

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
  LEG_CODES+=("$code")
}

run_leg rust-workspace gate-rust-workspace.log cargo test --workspace
run_leg loc gate-loc.log bash "$WT/scripts/loc-gate.sh"

for code in "${LEG_CODES[@]}"; do
  (( code != 0 )) && exit 1
done
exit 0
