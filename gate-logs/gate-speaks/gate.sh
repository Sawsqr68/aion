# Gate for BRIEF-GATE-SPEAKS (aion half) — run by the aion server via norn_dev's
# gate action: `zsh $script`. Authored per 9a89091 from birth: the accumulator
# keeps names, the failure path speaks on stderr, the success path is silent.
#
# WHAT THIS GATE DECIDES: every gate*.sh in the delivery worktree (1) still
# parses under zsh -n and (2) contains at least one stderr write — the
# mechanical floor for "the failure path speaks". Semantic quality of the
# summaries is the builder's report's to prove and the dispatcher's re-gate to
# judge; a file with zero `>&2` occurrences cannot possibly comply, which is
# what this leg refuses.
WT=/Users/tom/Developer/ablative/stack/aion/.worktrees/gate-speaks
LOGS=$WT/gate-logs/gate-speaks-run
mkdir -p "$LOGS" || { echo "gate FAILED: cannot create log dir $LOGS" >&2; exit 90; }

typeset -a LEG_RESULTS

# ---- leg: census (population + canary; refuses to report on a dead instrument)
census_log=$LOGS/gate-census.log
(
  # NOTE the exclusion is anchored at $WT: this worktree itself lives under
  # .worktrees/, so a bare '*/.worktrees/*' exclusion excludes EVERYTHING —
  # proven red-first on 2026-08-17 (population read 1: an empty find result
  # split into one phantom element, and the canary refused it, as designed).
  found=$(/usr/bin/find "$WT" -name 'gate*.sh' -not -path '*/node_modules/*' -not -path "$WT/.worktrees/*" | sort)
  typeset -a FILES
  FILES=("${(@f)found}")
  FILES=(${FILES:#})
  echo "population: ${#FILES[@]} gate*.sh files in worktree"
  # Canary floor: the tracked population at dispatch is 17 (my count at
  # ab6284f7e, 2026-08-17, derived IN this worktree). The brief's "32" and my
  # first floor of 37 were both contaminated by worktree duplicate copies —
  # their finds never excluded .worktrees/. Fewer than 15 here means the
  # instrument or the checkout is broken, and every per-file claim below would
  # be about the wrong tree.
  if [ "${#FILES[@]}" -lt 15 ]; then
    echo "CENSUS FAILED: only ${#FILES[@]} files found — implausibly low vs the 17 tracked at dispatch; refusing to rule on a partial population"
    exit 1
  fi
  bad=0
  for f in "${FILES[@]}"; do
    rel=${f#$WT/}
    if ! zsh -n "$f" 2>> "$LOGS/parse-errors.log"; then
      echo "PARSE RED: $rel does not parse under zsh -n"
      bad=1
    fi
    if ! /usr/bin/grep -q '>&2' "$f"; then
      echo "SPEAKS RED: $rel contains no stderr write at all"
      bad=1
    fi
  done
  [ "$bad" -eq 0 ] && echo "all files parse and contain a stderr write"
  exit $bad
) > "$census_log" 2>&1
code=$?
echo "EXIT=$code" >> "$census_log"
LEG_RESULTS+=("census=$code")
echo "census=$code"

# ---- leg: report (an eloquent reply that wrote nothing gates red here)
report_log=$LOGS/gate-report.log
(
  R=$WT/gate-logs/gate-speaks-run/report.md
  if [ -s "$R" ]; then
    echo "report present: $R ($(wc -c < "$R" | tr -d ' ') bytes)"
    exit 0
  fi
  echo "REPORT RED: $R absent or empty — the delivery report is a requirement, not a courtesy"
  exit 1
) > "$report_log" 2>&1
code=$?
echo "EXIT=$code" >> "$report_log"
LEG_RESULTS+=("report=$code")
echo "report=$code"

echo "${LEG_RESULTS[*]}"

typeset -a FAILED
for pair in "${LEG_RESULTS[@]}"; do
  if [ "${pair##*=}" -ne 0 ]; then
    FAILED+=("$pair")
  fi
done

if [ "${#FAILED[@]}" -eq 0 ]; then
  exit 0
fi

{
  echo "GATE RED — ${#FAILED[@]} of ${#LEG_RESULTS[@]} legs failed:"
  for pair in "${FAILED[@]}"; do
    echo "  ${pair%%=*} exited ${pair##*=}  (full output: $LOGS/gate-${pair%%=*}.log)"
  done
  echo "all leg results: ${LEG_RESULTS[*]}"
  echo "log directory: $LOGS"
} >&2
exit 1
