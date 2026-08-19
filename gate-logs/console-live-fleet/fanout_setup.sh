# fanout_setup — cut one isolated worktree + branch per leg from the planned
# primary tree, carry the planner's .fleet/ briefs into each, install the
# console's deps in each.
# Run by the aion server via fleet_dev's `fanout` action: `zsh $script`.
# REFUSES LOUDLY rather than deleting anything in its way: a leftover tree or
# branch from a prior attempt is the dispatcher's to inspect, never this
# script's to remove. Every refusal is named on stdout AND stderr.
WT=/Users/tom/Developer/ablative/stack/aion/.worktrees/console-live
TREES=/Users/tom/Developer/ablative/stack/aion/.worktrees
for leg in L1 L2; do
  LWT=$TREES/console-live-$leg
  BR=console-live-$leg
  if [ -e "$LWT" ]; then
    echo "REFUSING: $LWT already exists (prior attempt? dispatcher inspects, script deletes nothing)"
    echo "fanout REFUSED at leg $leg: worktree path $LWT already exists" >&2
    exit 1
  fi
  if git -C "$WT" show-ref --verify --quiet "refs/heads/$BR"; then
    echo "REFUSING: branch $BR already exists (prior attempt? dispatcher inspects, script deletes nothing)"
    echo "fanout REFUSED at leg $leg: branch $BR already exists" >&2
    exit 1
  fi
  git -C "$WT" worktree add -b "$BR" "$LWT" console-live || {
    echo "FANOUT FAILED cutting $leg"
    echo "fanout FAILED at leg $leg: git worktree add -b $BR $LWT console-live" >&2
    exit 1
  }
  cp -R "$WT/.fleet" "$LWT/.fleet" || {
    echo "FANOUT FAILED carrying .fleet into $leg"
    echo "fanout FAILED at leg $leg: could not carry .fleet into $LWT" >&2
    exit 1
  }
  mkdir -p "$LWT/.fleet/reports"
  ( cd "$LWT/apps/aion-ops-console" && bun install > "$LWT/.fleet/bun-install.log" 2>&1 ) || {
    echo "FANOUT FAILED: bun install in $leg (see $LWT/.fleet/bun-install.log)"
    echo "fanout FAILED at leg $leg: bun install, log at $LWT/.fleet/bun-install.log" >&2
    exit 1
  }
  echo "cut: $leg -> $LWT (branch $BR)"
done
exit 0
