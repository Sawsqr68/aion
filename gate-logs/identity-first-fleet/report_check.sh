# report_check — the builders get the planner's treatment: every promised
# report must exist non-empty in the MERGED primary tree.
# Run by the aion server via fleet_dev's `report_check` action: `zsh $script`.
# Each missing leg is named on stdout; non-zero exit ends the run at
# check_reports.
WT=/Users/tom/Developer/ablative/apps/manifold/.worktrees/identity-first
missing=0
for leg in L1 L2; do
  f=$WT/.fleet/reports/$leg.md
  if [ ! -s "$f" ]; then
    echo "MISSING REPORT: $leg ($f)"
    missing=1
  else
    echo "present: $leg ($(wc -c < "$f" | tr -d ' ') bytes)"
  fi
done
exit $missing
