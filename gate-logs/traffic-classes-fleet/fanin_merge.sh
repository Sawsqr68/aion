# fanin_merge — merge each leg branch into transient-class in dispatch order
# (L1, L2), then carry any leg report the merge did not already bring.
# Run by the aion server via fleet_dev's `fanin` action: `zsh $script`.
# MERGE FIRST: the leg branches track their own .fleet/reports/<id>.md, so
# copying a report into the primary before merging creates an untracked
# collision git refuses to overwrite (fleet run 2's fanin failed exactly
# there; this is the fixed order that resumed it green).
# A conflict is a finding about the plan's leg boundaries: the conflicting
# files are named on stdout, the merge is aborted clean, every leg branch is
# left intact, and the non-zero exit ends the run at fanin.
WT=/Users/tom/Developer/ablative/apps/manifold/.worktrees/transient-class
TREES=/Users/tom/Developer/ablative/apps/manifold/.worktrees
cd "$WT" || exit 90

for leg in L1 L2; do
  BR=transient-class-$leg
  if git merge --no-ff -m "fleet fan-in: merge $BR" "$BR"; then
    echo "merged: $BR"
  else
    echo "MERGE CONFLICT at $BR — conflicting files:"
    git -c diff.external= diff --no-ext-diff --name-only --diff-filter=U
    git merge --abort
    exit 1
  fi
done

mkdir -p .fleet/reports
for leg in L1 L2; do
  DST=.fleet/reports/$leg.md
  SRC=$TREES/transient-class-$leg/.fleet/reports/$leg.md
  if [ -s "$DST" ]; then
    echo "report present after merge: $leg"
  elif [ -s "$SRC" ]; then
    cp "$SRC" "$DST" || { echo "FANIN FAILED carrying report $leg"; exit 1; }
    echo "report carried from leg tree: $leg"
  else
    echo "report absent everywhere: $leg (report_check will rule)"
  fi
done
exit 0
