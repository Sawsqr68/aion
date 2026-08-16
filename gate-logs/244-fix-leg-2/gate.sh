#!/bin/zsh
# Server-run norn_dev gate for the #244 fix leg 2 (Opus pass-2 DO-NOT-LAND remediation:
# NEW-B1 generation gating + NEW-2 empty-backfill notice).
# Scope rationale: the pass-2 delta is confined to aion-cli (tail/live.rs, tail.rs and
# their tests). The reviewer's landing requirement names "aion-cli suite + e2e at
# minimum"; Waffles' full re-gate follows a LAND verdict and covers the world. This
# gate covers everything the delta can invalidate: fmt, BOTH clippy resolutions of the
# touched crate (workspace AND -p — the two resolve features differently), the full
# aion-cli suite, and the tail e2e explicitly.
set -u

WT=/Users/tom/Developer/ablative/stack/aion/.worktrees/244-child-visibility
LOGS=/Users/tom/Developer/ablative/stack/aion/gate-logs/244-fix-leg-2
mkdir -p "$LOGS" || { echo "gate setup FAILED: cannot create log directory $LOGS" >&2; exit 90; }

typeset -a LEG_RESULTS
typeset -A LEG_LOGS

# TEMPLATE AMENDMENT (post-run, delta-review follow-up 2026-08-16): every leg
# log now opens with the tree's HEAD so the log is self-attributing. Leg 2 ran
# WITHOUT this stamp (provenance mitigated in the delta review by mtimes +
# clean worktree + reviewer re-runs at tip); every future copy carries it.
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

run_leg fmt gate-fmt.log cargo fmt --all -- --check
run_leg clippy-workspace gate-clippy-workspace.log cargo clippy --workspace --all-targets -- -D warnings
run_leg clippy-aion-cli gate-clippy-aion-cli.log cargo clippy -p aion-cli --all-targets -- -D warnings
run_leg aion-cli gate-aion-cli.log cargo test -p aion-cli --no-fail-fast
run_leg tail-e2e gate-tail-e2e.log cargo test -p aion-cli --test tail_follow_e2e -- --nocapture

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
