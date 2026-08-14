# report_check — the builders' deliverables, verified the same way.
# Run by the aion server via fleet_dev's `report_check` action: `zsh $script`.
# A builder's reply is a claim; the tree is the fact. Every promised report
# must exist and be non-empty; each missing leg is named on stdout.
# Non-zero exit routes the run to `unreported`.
WT=/Users/tom/Developer/ablative/apps/manifold/.worktrees/declared-actions
missing=0
for leg in L1 L2; do
  f="$WT/.fleet/reports/$leg.md"
  if [ ! -s "$f" ]; then
    echo "MISSING LEG: $leg ($f)"
    missing=1
  else
    echo "present: $leg ($(wc -c < "$f" | tr -d ' ') bytes)"
  fi
done
exit $missing
