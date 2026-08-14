# plan_check — the planner's deliverable, verified by the machine.
# Run by the aion server via fleet_dev's `plan_check` action: `zsh $script`.
# Every promised file must exist and be non-empty; each missing one is named
# on stdout. Non-zero exit routes the run to `unplanned`.
WT=/Users/tom/Developer/ablative/apps/manifold/.worktrees/declared-actions
missing=0
for f in "$WT/.fleet/plan.md" "$WT/.fleet/legs/L1.md" "$WT/.fleet/legs/L2.md"; do
  if [ ! -s "$f" ]; then
    echo "MISSING: $f"
    missing=1
  else
    echo "present: $f ($(wc -c < "$f" | tr -d ' ') bytes)"
  fi
done
exit $missing
