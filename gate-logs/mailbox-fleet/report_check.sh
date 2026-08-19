# report_check — the builders get the planner's treatment: every promised
# report must exist non-empty in the MERGED primary tree.
# Run by the aion server via fleet_dev's `report_check` action: `zsh $script`.
# Missing legs are named on stdout AND stderr (BRIEF-GATE-SPEAKS 9a89091).
#
# AMENDMENT 2026-08-17 ~09:45, MID-RUN, before this step executed — deep
# verification of the flight found the primary tree ALREADY CARRIES
# .fleet/reports/L1.md and L2.md at checkout time: a PRIOR fleet run
# (transient-class) committed its .fleet/reports onto manifold main
# (commit e70107b), so every fresh worktree inherits stale reports and the
# existence check below is VACUOUS — a builder that writes nothing would
# pass on a dead run's paper. Absence wearing a green tick.
# Fix: each report must (a) NOT be byte-identical to the pinned stale
# specimen (sha256 recorded at discovery, stale-report-specimens.sha256
# beside this script), and (b) mention this run's subject. Repo-hygiene
# follow-up at landing: untrack .fleet/ on main + gitignore it.
WT=/Users/tom/Developer/ablative/apps/manifold/.worktrees/mailbox-fleet
STALE_L1=14cad7f4180bb28f3a927fe6254e4fb328eda980522a9402c821a486eba1070e
STALE_L2=f39412563e65678dab7fec7cffd90e2580a0930487a42bdabc632346e11d837b
missing=0
failed_legs=()
for leg in L1 L2; do
  f=$WT/.fleet/reports/$leg.md
  if [ ! -s "$f" ]; then
    echo "MISSING REPORT: $leg ($f)"
    failed_legs+=("$leg")
    missing=1
    continue
  fi
  sha=$(shasum -a 256 "$f" | cut -d' ' -f1)
  if [ "$sha" = "$STALE_L1" ] || [ "$sha" = "$STALE_L2" ]; then
    echo "STALE REPORT: $leg is byte-identical to the transient-class specimen inherited from main — the builder wrote nothing"
    echo "report_check REFUSED: leg $leg report is the PRIOR RUN'S committed specimen (sha $sha), not this run's delivery" >&2
    failed_legs+=("$leg")
    missing=1
    continue
  fi
  if ! /usr/bin/grep -qi 'mailbox\|retry\|backoff\|admit' "$f"; then
    echo "OFF-SUBJECT REPORT: $leg never mentions this run's subject (mailbox/retry/backoff/admit)"
    echo "report_check REFUSED: leg $leg report matches no subject term of this flight — adjudicate before trusting" >&2
    failed_legs+=("$leg")
    missing=1
    continue
  fi
  echo "present: $leg ($(wc -c < "$f" | tr -d ' ') bytes, sha $sha, subject terms found)"
done
if [ "$missing" -ne 0 ]; then
  echo "report_check REFUSED: no genuine report in the merged tree from leg(s): ${failed_legs[*]}" >&2
  echo "report_check: expected each at $WT/.fleet/reports/<leg>.md, fresh content, on-subject" >&2
fi
exit $missing
