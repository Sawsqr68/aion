# Pilot gate script — manifold declared-faces (K6 leg 1), brief R7.
# Run by the aion server via the norn_dev `gates` action: `zsh $script`.
# Four gates, every exit captured directly to gate-logs/declared-faces/,
# never through a pipeline. Non-zero exit = gates_red route.
WT=/Users/tom/Developer/ablative/apps/manifold/.worktrees/declared-faces
PAGE=$WT/surface/page
LOGS=$WT/gate-logs/declared-faces
mkdir -p "$LOGS" || exit 90
cd "$PAGE" || exit 91

npm run build > "$LOGS/gate-build.log" 2>&1
build_exit=$?
echo "EXIT=$build_exit" >> "$LOGS/gate-build.log"

node scripts/composition-test.mjs > "$LOGS/gate-composition.log" 2>&1
comp_exit=$?
echo "EXIT=$comp_exit" >> "$LOGS/gate-composition.log"
# Standing flake protocol (Waffles, pilot dispatch 2026-08-14): a composition
# failure on this loaded box is re-run solo ONCE before any verdict. Both
# logs are kept — a real red fails twice; a flake passes with its first red
# preserved as evidence. Verified live: the untouched-tree baseline red
# ("editor takes the focus") passed 630/630 on solo re-run.
if [ "$comp_exit" -ne 0 ]; then
  node scripts/composition-test.mjs > "$LOGS/gate-composition-rerun.log" 2>&1
  comp_exit=$?
  echo "EXIT=$comp_exit" >> "$LOGS/gate-composition-rerun.log"
  echo "composition first run RED, solo re-run exit=$comp_exit (both logs kept)"
fi

cd "$WT" || exit 92
bash scripts/loc-gate.sh > "$LOGS/gate-loc.log" 2>&1
loc_exit=$?
echo "EXIT=$loc_exit" >> "$LOGS/gate-loc.log"

cd "$PAGE" || exit 93
node scripts/dist-gate.mjs > "$LOGS/gate-dist.log" 2>&1
dist_exit=$?
echo "EXIT=$dist_exit" >> "$LOGS/gate-dist.log"

echo "build=$build_exit composition=$comp_exit loc=$loc_exit dist=$dist_exit"
if [ "$build_exit" -eq 0 ] && [ "$comp_exit" -eq 0 ] && [ "$loc_exit" -eq 0 ] && [ "$dist_exit" -eq 0 ]; then
  exit 0
fi
exit 1
