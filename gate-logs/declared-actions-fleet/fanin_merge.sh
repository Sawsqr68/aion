# fanin_merge — carry each leg's report back to the primary tree, then merge
# each leg branch into declared-actions in dispatch order (L1, L2, L3).
# Run by the aion server via fleet_dev's `fanin` action: `zsh $script`.
# A conflict is a finding about the plan's leg boundaries: the conflicting
# files are named on stdout, the merge is aborted clean, every leg branch is
# left intact, and the non-zero exit routes the run to `unmerged`.
WT=/Users/tom/Developer/ablative/apps/manifold/.worktrees/declared-actions
TREES=/Users/tom/Developer/ablative/apps/manifold/.worktrees
cd "$WT" || exit 90
mkdir -p .fleet/reports

# Reports first (untracked in the leg trees, so the merge does not carry
# them): even a run that goes on to conflict keeps the builders' accounts.
for leg in L1 L2 L3; do
  SRC=$TREES/declared-actions-$leg/.fleet/reports/$leg.md
  if [ -s "$SRC" ]; then
    cp "$SRC" ".fleet/reports/$leg.md" || { echo "FANIN FAILED carrying report $leg"; exit 1; }
    echo "report carried: $leg"
  else
    echo "report absent in leg tree: $leg (report_check will rule)"
  fi
done

for leg in L1 L2 L3; do
  BR=declared-actions-$leg
  if git merge --no-ff -m "fleet fan-in: merge $BR" "$BR"; then
    echo "merged: $BR"
  else
    echo "MERGE CONFLICT at $BR — conflicting files:"
    git -c diff.external= diff --no-ext-diff --name-only --diff-filter=U
    git merge --abort
    exit 1
  fi
done
exit 0
