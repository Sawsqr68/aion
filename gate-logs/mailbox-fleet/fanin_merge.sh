# fanin_merge — merge each leg branch into mailbox-fleet in dispatch order
# (L1, L2), then carry any leg report the merge did not already bring.
# Run by the aion server via fleet_dev's `fanin` action: `zsh $script`.
# MERGE FIRST (ordering proven on the console-live run; do not reorder).
# A conflict is a finding about the plan's leg boundaries — the conflicting
# files are named on stdout AND stderr, the merge is aborted clean, every leg
# branch is left intact, and the non-zero exit ends the run at fanin.
WT=/Users/tom/Developer/ablative/apps/manifold/.worktrees/mailbox-fleet
TREES=/Users/tom/Developer/ablative/apps/manifold/.worktrees
cd "$WT" || { echo "fanin FAILED: cannot cd to primary tree $WT" >&2; exit 90; }

for leg in L1 L2; do
  BR=mailbox-$leg
  if git merge --no-ff -m "fleet fan-in: merge $BR" "$BR"; then
    echo "merged: $BR"
  else
    echo "MERGE CONFLICT at $BR — conflicting files:"
    git -c diff.external= diff --no-ext-diff --name-only --diff-filter=U
    {
      echo "fanin REFUSED: merge conflict merging $BR into mailbox-fleet."
      echo "conflicting files:"
      git -c diff.external= diff --no-ext-diff --name-only --diff-filter=U
      echo "The merge was aborted; the primary tree is clean and every leg branch is intact."
      echo "This is a finding about the plan's leg boundaries, not a merge to resolve by hand."
    } >&2
    git merge --abort
    exit 1
  fi
done

mkdir -p .fleet/reports
for leg in L1 L2; do
  DST=.fleet/reports/$leg.md
  SRC=$TREES/mailbox-$leg/.fleet/reports/$leg.md
  if [ -s "$DST" ]; then
    echo "report present after merge: $leg"
  elif [ -s "$SRC" ]; then
    cp "$SRC" "$DST" || {
      echo "FANIN FAILED carrying report $leg"
      echo "fanin FAILED: could not carry report for leg $leg from $SRC" >&2
      exit 1
    }
    echo "report carried from leg tree: $leg"
  else
    echo "report absent everywhere: $leg (report_check will rule)"
  fi
done
exit 0
