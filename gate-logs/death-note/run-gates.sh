#!/bin/bash
# Death-note branch gate battery — sequential legs, per-leg exit codes,
# summary lines echoed to stderr as well as stdout (the stderr rider:
# aion records failure reasons from stderr only, so every summary speaks
# on both streams).
#
# Defect history, kept on the record: revision 1 wrapped each leg in a
# `{ … } > log` BRACE GROUP, which runs in the current shell, so the
# `exit $code` inside it terminated the whole driver after leg 1 (caught
# by Waffles' liveness sweep at 01:44, three minutes after launch). Legs
# now run in a real subshell, whose exit ends only the subshell.
set -u
cd "$(dirname "$0")/../.." || exit 90

LOGDIR="gate-logs/death-note"
HEAD_SHA=$(git rev-parse HEAD)
say() { echo "$@"; echo "$@" >&2; }

say "=GATES= HEAD=$HEAD_SHA start=$(date -u +%Y-%m-%dT%H:%M:%SZ) load=$(sysctl -n vm.loadavg)"

run_leg() {
  local name="$1"; shift
  local log="$LOGDIR/$name.log"
  (
    echo "LEG=$name HEAD=$HEAD_SHA start=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "load_at_start=$(sysctl -n vm.loadavg)"
    "$@" 2>&1
    code=$?
    echo "LEG=$name EXIT=$code end=$(date -u +%Y-%m-%dT%H:%M:%SZ) load_at_end=$(sysctl -n vm.loadavg)"
    exit "$code"
  ) > "$log"
  local code=$?
  say "LEG=$name EXIT=$code"
  return $code
}

overall=0
run_leg fmt cargo fmt --all || overall=1
# fmt WRITES (house law: autoformat, never format-check); a dirty tree after
# fmt is a finding recorded here, committed as its own commit by the driver.
git status --short > "$LOGDIR/post-fmt-status.txt"
say "post-fmt dirty files: $(wc -l < "$LOGDIR/post-fmt-status.txt" | tr -d ' ')"

run_leg test-death-note cargo test -p aion-server death_note || overall=1
run_leg test-aion-server cargo test -p aion-server || overall=1
run_leg clippy-workspace cargo clippy --workspace --all-targets -- -D warnings || overall=1
run_leg clippy-p-aion-server cargo clippy -p aion-server --all-targets -- -D warnings || overall=1

say "=GATES= OVERALL=$overall end=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "$overall" > "$LOGDIR/OVERALL-EXIT.txt"
