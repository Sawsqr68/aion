# report_check — the builders get the planner's treatment: every promised
# report must exist non-empty in the MERGED primary tree.
# Run by the aion server via fleet_dev's `report_check` action: `zsh $script`.
# A builder that answered eloquently and wrote nothing leaves the tree in its
# prior state — WHICH CAN GATE GREEN. This is the mechanism that catches it,
# before the gates and long before the judge.
# Missing legs are named on stdout AND on stderr (BRIEF-GATE-SPEAKS 9a89091).
WT=/Users/tom/Developer/ablative/stack/aion/.worktrees/console-live
missing=0
failed_legs=()
for leg in L1 L2; do
  f=$WT/.fleet/reports/$leg.md
  if [ ! -s "$f" ]; then
    echo "MISSING REPORT: $leg ($f)"
    failed_legs+=("$leg")
    missing=1
  else
    echo "present: $leg ($(wc -c < "$f" | tr -d ' ') bytes)"
  fi
done
if [ "$missing" -ne 0 ]; then
  echo "report_check REFUSED: no report in the merged tree from leg(s): ${failed_legs[*]}" >&2
  echo "report_check: expected each at $WT/.fleet/reports/<leg>.md" >&2
fi
exit $missing
