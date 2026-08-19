# Fleet gate script — manifold mailbox retry-budget (R1 + R2/R3).
# Run by the aion server via fleet_dev's `gates` action: `zsh $script`.
#
# AUTHORED PER BRIEF-GATE-SPEAKS (9a89091) FROM BIRTH: the accumulator keeps
# NAMES, and the failure path speaks on stderr, naming every red leg, its
# code, and the log holding its full output. The success path stays silent.
#
# BASELINE MEASURED BEFORE DISPATCH (the 21b9ea9a lesson — two of its three
# reds were this harness's own unmeasured baseline, not the delivery):
# fmt/clippy/test were run on the freshly cut mailbox-fleet worktree at
# manifold main d021100 before this gate could fire, and the dispatch was
# held until that baseline was green. A red here is therefore the DELIVERY.
#
# SCOPE, STATED: the brief's three requirements all live in crate
# manifold-node (src/kernel/ for R1, src/mailbox/ for R2+R3), so this gate
# runs fmt + clippy + the crate's test suite. --test-threads=2 is deliberate:
# the mailbox suite has a known contention-sensitive flake on main (~6% at
# default parallelism, board #152/#175); bounded threads is the measured
# mitigation, not a guess. The dispatcher's landing re-gate runs the standing
# battery on the final tree — this gate refuses early, it does not replace it.
WT=/Users/tom/Developer/ablative/apps/manifold/.worktrees/mailbox-fleet
LOGS=$WT/gate-logs/mailbox-fleet
mkdir -p "$LOGS" || { echo "gate FAILED: cannot create log dir $LOGS" >&2; exit 90; }

typeset -a LEG_RESULTS

run_leg() {
  # run_leg <name> <command...>
  local name=$1
  shift
  ( cd "$WT" && "$@" ) > "$LOGS/gate-$name.log" 2>&1
  local code=$?
  echo "EXIT=$code" >> "$LOGS/gate-$name.log"
  LEG_RESULTS+=("$name=$code")
  echo "$name=$code"
}

run_leg fmt    cargo fmt -p manifold-node -- --check
run_leg clippy cargo clippy -p manifold-node --all-targets -- -D warnings
run_leg test   cargo test -p manifold-node -- --test-threads=2

# RED-FIRST EVIDENCE LEG — the brief demands the failing test land as its own
# commit before the fix. Mechanical check: each leg branch's history (now
# merged) must contain at least one commit whose message marks the red step.
# Adjudication of WHICH test went red stays with the judge and the dispatcher;
# this leg only refuses the structural absence of any red commit at all.
red_log=$LOGS/gate-redfirst.log
(
  cd "$WT" || exit 90
  total=$(git log --oneline --grep='red' -i mailbox-L1 mailbox-L2 --not main -- 2>/dev/null | wc -l | tr -d ' ')
  echo "red-marked commits across leg branches: $total"
  git log --oneline --grep='red' -i mailbox-L1 mailbox-L2 --not main -- 2>/dev/null
  if [ "$total" -eq 0 ]; then
    echo "RED-FIRST ABSENT: no commit on either leg branch marks the red step"
    exit 1
  fi
  exit 0
) > "$red_log" 2>&1
red_code=$?
echo "EXIT=$red_code" >> "$red_log"
LEG_RESULTS+=("redfirst=$red_code")
echo "redfirst=$red_code"

echo "${LEG_RESULTS[*]}"

typeset -a FAILED
for pair in "${LEG_RESULTS[@]}"; do
  if [ "${pair##*=}" -ne 0 ]; then
    FAILED+=("$pair")
  fi
done

if [ "${#FAILED[@]}" -eq 0 ]; then
  exit 0
fi

{
  echo "GATE RED — ${#FAILED[@]} of ${#LEG_RESULTS[@]} legs failed:"
  for pair in "${FAILED[@]}"; do
    echo "  ${pair%%=*} exited ${pair##*=}  (full output: $LOGS/gate-${pair%%=*}.log)"
  done
  echo "all leg results: ${LEG_RESULTS[*]}"
  echo "log directory: $LOGS"
} >&2
exit 1
