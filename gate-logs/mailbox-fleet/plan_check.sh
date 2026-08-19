# plan_check — the planner's deliverable, verified by the machine.
# Run by the aion server via fleet_dev's `plan_check` action: `zsh $script`.
# The planner's reply is a claim; the tree is the fact. Every promised file
# must exist and be non-empty; each missing one is named on stdout AND on
# stderr (BRIEF-GATE-SPEAKS 9a89091).
WT=/Users/tom/Developer/ablative/apps/manifold/.worktrees/mailbox-fleet
missing=0
for f in "$WT/.fleet/plan.md" "$WT/.fleet/legs/L1.md" "$WT/.fleet/legs/L2.md"; do
  if [ ! -s "$f" ]; then
    echo "MISSING: $f"
    echo "plan_check: MISSING $f" >&2
    missing=1
  else
    echo "present: $f ($(wc -c < "$f" | tr -d ' ') bytes)"
  fi
done
[ "$missing" -ne 0 ] && echo "plan_check REFUSED: the planner promised files it did not write (see names above)" >&2
exit $missing
