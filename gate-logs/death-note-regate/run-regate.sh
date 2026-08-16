#!/bin/bash
# Waffles' landing re-gate on the MERGED tree (main 15bb089da + death-note af7137336
# = merge a3cd9018). Mirrors Vesper's 5 legs + a workspace-test leg (root Cargo.toml
# moved in this branch). Subshell legs per the brace-trap law.
set -u
cd "$(dirname "$0")/../.." || exit 90
LOGDIR="gate-logs/death-note-regate"
HEAD_SHA=$(git rev-parse HEAD)
echo "REGATE HEAD=$HEAD_SHA start=$(date -u +%Y-%m-%dT%H:%M:%SZ)"

run_leg() {
  local name="$1"; shift
  local log="$LOGDIR/$name.log"
  (
    echo "LEG=$name HEAD=$HEAD_SHA start=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    "$@" 2>&1
    code=$?
    echo "LEG=$name EXIT=$code end=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    exit "$code"
  ) > "$log"
  local code=$?
  echo "LEG=$name EXIT=$code"
  return $code
}

overall=0
run_leg fmt cargo fmt --all || overall=1
git status --short > "$LOGDIR/post-fmt-status.txt"
echo "post-fmt dirty: $(wc -l < "$LOGDIR/post-fmt-status.txt" | tr -d ' ')"
run_leg test-death-note cargo test -p aion-server death_note || overall=1
run_leg test-aion-server cargo test -p aion-server || overall=1
run_leg test-workspace cargo test --workspace || overall=1
run_leg clippy-workspace cargo clippy --workspace --all-targets -- -D warnings || overall=1
run_leg clippy-p-aion-server cargo clippy -p aion-server --all-targets -- -D warnings || overall=1
echo "REGATE OVERALL=$overall end=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "$overall" > "$LOGDIR/OVERALL-EXIT.txt"
exit "$overall"
