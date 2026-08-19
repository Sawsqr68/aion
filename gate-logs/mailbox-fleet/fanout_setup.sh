# fanout_setup — cut one isolated worktree + branch per leg from the planned
# primary tree, carry the planner's .fleet/ briefs into each.
# Run by the aion server via fleet_dev's `fanout` action: `zsh $script`.
# REFUSES LOUDLY rather than deleting anything in its way: a leftover tree or
# branch from a prior attempt is the dispatcher's to inspect, never this
# script's to remove. Every refusal is named on stdout AND stderr.
# No dependency install step: this is a pure-Rust flight; each leg's first
# cargo invocation builds its own default target dir.
WT=/Users/tom/Developer/ablative/apps/manifold/.worktrees/mailbox-fleet
TREES=/Users/tom/Developer/ablative/apps/manifold/.worktrees
for leg in L1 L2; do
  LWT=$TREES/mailbox-$leg
  BR=mailbox-$leg
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
  git -C "$WT" worktree add -b "$BR" "$LWT" mailbox-fleet || {
    echo "FANOUT FAILED cutting $leg"
    echo "fanout FAILED at leg $leg: git worktree add -b $BR $LWT mailbox-fleet" >&2
    exit 1
  }
  cp -R "$WT/.fleet" "$LWT/.fleet" || {
    echo "FANOUT FAILED carrying .fleet into $leg"
    echo "fanout FAILED at leg $leg: could not carry .fleet into $LWT" >&2
    exit 1
  }
  echo "cut: $LWT on branch $BR with .fleet carried"
done
exit 0
