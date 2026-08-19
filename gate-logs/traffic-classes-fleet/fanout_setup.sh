# fanout_setup — cut one isolated worktree + branch per leg from the planned
# primary tree, carry the planner's .fleet/ briefs into each, npm ci each.
# Run by the aion server via fleet_dev's `fanout` action: `zsh $script`.
# REFUSES LOUDLY rather than deleting anything in its way: a leftover tree
# or branch from a prior attempt is the dispatcher's to inspect, never this
# script's to remove. Non-zero exit ends the run at fanout.
WT=/Users/tom/Developer/ablative/apps/manifold/.worktrees/transient-class
TREES=/Users/tom/Developer/ablative/apps/manifold/.worktrees
for leg in L1 L2; do
  LWT=$TREES/transient-class-$leg
  BR=transient-class-$leg
  if [ -e "$LWT" ]; then
    echo "REFUSING: $LWT already exists (prior attempt? dispatcher inspects, script deletes nothing)"
    exit 1
  fi
  if git -C "$WT" show-ref --verify --quiet "refs/heads/$BR"; then
    echo "REFUSING: branch $BR already exists (prior attempt? dispatcher inspects, script deletes nothing)"
    exit 1
  fi
  git -C "$WT" worktree add -b "$BR" "$LWT" transient-class || { echo "FANOUT FAILED cutting $leg"; exit 1; }
  cp -R "$WT/.fleet" "$LWT/.fleet" || { echo "FANOUT FAILED carrying .fleet into $leg"; exit 1; }
  mkdir -p "$LWT/.fleet/reports"
  ( cd "$LWT/surface/page" && npm ci > "$LWT/.fleet/npm-ci.log" 2>&1 ) || {
    echo "FANOUT FAILED: npm ci in $leg (see $LWT/.fleet/npm-ci.log)"
    exit 1
  }
  echo "cut: $leg -> $LWT (branch $BR)"
done
exit 0
