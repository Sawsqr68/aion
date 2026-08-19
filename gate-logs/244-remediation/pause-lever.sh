#!/bin/zsh
# THE PAUSE LEVER for #244-rem (decision recorded 06:2xZ, Waffles concurring):
# at the 12G watcher fire, SIGSTOP the ENTIRE process tree of norn 25274 —
# the norn pid alone leaves its cargo children burning disk. SIGCONT the same
# list to resume (pass "resume"). Costs: paused wall-clock only; target
# survives; session resumable.
set -u
MODE="${1:-pause}"
ROOT=25274

descendants() {
  local pid=$1
  echo "$pid"
  for child in $(pgrep -P "$pid" 2>/dev/null); do
    descendants "$child"
  done
}

PIDS=($(descendants $ROOT))
if [[ "$MODE" == "resume" ]]; then
  SIG="CONT"
else
  SIG="STOP"
fi
# Stop parents first for STOP (no re-spawn race), children first for CONT.
if [[ "$SIG" == "STOP" ]]; then
  LIST=(${PIDS[@]})
else
  LIST=($(echo "${PIDS[@]}" | tr ' ' '\n' | tail -r))
fi
for p in "${LIST[@]}"; do
  kill -"$SIG" "$p" 2>/dev/null && echo "$SIG -> $p"
done
echo "done: $SIG sent to ${#LIST[@]} pids"
ps -o pid,stat,command -p "${PIDS[@]}" 2>/dev/null | head -20
