# Fleet gate script — manifold transient traffic class (K21), third fleet run.
# Run by the aion server via fleet_dev's `gates` action: `zsh $script`.
# The two batteries the K21 package names, on the MERGED tree: the full Rust
# battery (both legs' zones meet at the compose door) and the full page
# battery (K8's proven four). Every exit captured directly to
# gate-logs/transient-class/, never through a pipeline. Non-zero exit ends
# the run at gate. Waffles' desk re-runs the batteries and adds the
# mandatory real-door scratch-estate walk before landing — this gate refuses
# early, it does not replace his.
WT=/Users/tom/Developer/ablative/apps/manifold/.worktrees/transient-class
PAGE=$WT/surface/page
LOGS=$WT/gate-logs/transient-class
mkdir -p "$LOGS" || { echo "gate setup FAILED: cannot create log directory $LOGS" >&2; exit 90; }

cd "$WT" || { echo "gate setup FAILED: cannot enter worktree $WT before rust leg" >&2; exit 91; }
cargo test --workspace > "$LOGS/gate-rust.log" 2>&1
rust_exit=$?
echo "EXIT=$rust_exit" >> "$LOGS/gate-rust.log"

cd "$PAGE" || { echo "gate setup FAILED: cannot enter page directory $PAGE before build leg" >&2; exit 92; }
npm run build > "$LOGS/gate-build.log" 2>&1
build_exit=$?
echo "EXIT=$build_exit" >> "$LOGS/gate-build.log"

comp_log=gate-composition.log
node scripts/composition-test.mjs > "$LOGS/gate-composition.log" 2>&1
comp_exit=$?
echo "EXIT=$comp_exit" >> "$LOGS/gate-composition.log"
# Standing flake protocol (Waffles, pilot dispatch 2026-08-14, carried per
# the fleet package): a composition failure on this loaded box is re-run
# solo ONCE before any verdict. Both logs are kept — a real red fails twice;
# a flake passes with its first red preserved as evidence.
if [ "$comp_exit" -ne 0 ]; then
  comp_log=gate-composition-rerun.log
  node scripts/composition-test.mjs > "$LOGS/gate-composition-rerun.log" 2>&1
  comp_exit=$?
  echo "EXIT=$comp_exit" >> "$LOGS/gate-composition-rerun.log"
  echo "composition first run RED, solo re-run exit=$comp_exit (both logs kept)"
fi

cd "$WT" || { echo "gate setup FAILED: cannot enter worktree $WT before loc leg" >&2; exit 93; }
bash scripts/loc-gate.sh > "$LOGS/gate-loc.log" 2>&1
loc_exit=$?
echo "EXIT=$loc_exit" >> "$LOGS/gate-loc.log"

cd "$PAGE" || { echo "gate setup FAILED: cannot enter page directory $PAGE before dist leg" >&2; exit 94; }
node scripts/dist-gate.mjs > "$LOGS/gate-dist.log" 2>&1
dist_exit=$?
echo "EXIT=$dist_exit" >> "$LOGS/gate-dist.log"

echo "rust=$rust_exit build=$build_exit composition=$comp_exit loc=$loc_exit dist=$dist_exit"
if [ "$rust_exit" -eq 0 ] && [ "$build_exit" -eq 0 ] && [ "$comp_exit" -eq 0 ] && [ "$loc_exit" -eq 0 ] && [ "$dist_exit" -eq 0 ]; then
  exit 0
fi

typeset -a LEG_RESULTS FAILED
LEG_RESULTS=("rust=$rust_exit" "build=$build_exit" "composition=$comp_exit" "loc=$loc_exit" "dist=$dist_exit")
for pair in "${LEG_RESULTS[@]}"; do
  (( ${pair##*=} != 0 )) && FAILED+=("$pair")
done

{
  echo "GATE RED — ${#FAILED[@]} of ${#LEG_RESULTS[@]} legs failed:"
  for pair in "${FAILED[@]}"; do
    case ${pair%%=*} in
      rust) logfile=gate-rust.log ;;
      build) logfile=gate-build.log ;;
      composition) logfile=$comp_log ;;
      loc) logfile=gate-loc.log ;;
      dist) logfile=gate-dist.log ;;
    esac
    echo "  ${pair%%=*} exited ${pair##*=}  (full output: $LOGS/$logfile)"
  done
  echo "all leg results: ${LEG_RESULTS[*]}"
  echo "log directory: $LOGS"
} >&2
exit 1
