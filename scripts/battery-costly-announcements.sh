# Costly-proof announcement gate for scripts/battery.sh.
#
# Five tests used `#[ignore]`, which the house rules ban outright. Among them
# were the CROSS-NODE kill-9 failover proof and the PARKED-TIMER resume proof —
# the two tests that establish the durability claim — so the headline number
# excluded them and nobody reading it could tell.
#
# They are now runtime-gated (`AION_RUN_COSTLY_TESTS=1` to opt in) and each
# announces, on the real stderr handle, WHICH PROOF did not happen. Default
# behaviour is deliberately unchanged: they still do not run. What changed is
# that the run says so.
#
# 🔴 THE HONEST COST, AND WHY THIS GATE IS NOT OPTIONAL. A runtime-gated skip
# returns `Ok(())`, so it counts as a PASS — the total now includes tests that
# proved nothing, which is worse than `#[ignore]`'s separate tally. That trade is
# only acceptable while the announcement is CHECKED. Delete this gate and the
# conversion becomes a downgrade.
#
# 🔴 THE PATTERN MUST MATCH AN ANNOUNCEMENT, NOT THE TOKEN. Found by the
# inverted control, which is the only reason it is not still here: grepping the
# bare token `COSTLY_PROOF_SKIPPED` also matches CARGO'S OWN DIAGNOSTICS, because
# a dead-code warning quotes the source line that defines the constant —
# `pub const SKIP_TOKEN: &str = "COSTLY_PROOF_SKIPPED";`. And that warning
# appears exactly when a call site is removed, so the count was inflated in
# precisely the case the gate exists to detect. The anchored
# `^COSTLY_PROOF_SKIPPED (PROVES NOTHING):` cannot match a quoted source line,
# which compilers indent.
#
# 🔴 A BASELINE IS ONLY A CONTROL IF IT IS INDEPENDENT OF WHAT IT MEASURES.
# The second defect the inverted control found, and the one that would have
# shipped a FALSE PASS. With the pattern fixed, this gate derived its expected
# count by grepping the number of `skip_unless_opted_in(` call sites and
# comparing it to the number of announcements — on the reasoning that a derived
# baseline beats a hardcoded list. It reported `4 call sites, 4 announcements`
# and PASSED while a durability proof was being skipped silently, because
# deleting the call site DELETED ITS OWN EXPECTATION. Two numbers moving
# together, agreeing perfectly about nothing.
#
# So the list below is deliberate. Enumerate the POPULATION — nothing else in
# the tree states which suites own a costly proof — and never the ANSWER. Adding
# a sixth costly proof does require an edit here; that is the price of an
# expectation the code under test cannot move. The non-vacuity guard still
# matters for the same reason it always does: an empty list would assert nothing
# while reporting success, so zero checked suites is a failure.
costly_announcements() {
  # The declared list of suites that carry a costly proof. This IS the
  # enumeration, and it has to be: nothing else in the tree says which suites
  # own one. What must never be enumerated is the ANSWER — see below.
  local targets="lsub5b_osproc_failover_e2e lsub5b_osproc_kill9_failover_e2e \
                 parked_timer_osproc_kill9_failover_e2e stacked_dev_live_e2e \
                 new_scaffold_build"
  local target out found failures=0 checked=0

  for target in $targets; do
    out=$(cargo test -q -p aion-cli --test "$target" 2>&1)
    found=$(printf '%s' "$out" | grep -c '^COSTLY_PROOF_SKIPPED (PROVES NOTHING):')
    checked=$((checked + 1))
    if [ "$found" -eq 0 ]; then
      printf 'COSTLY-ANNOUNCEMENT GATE FAILED: `%s` announced NOTHING.\n' "$target"
      printf 'Its costly proof is being skipped silently — the exact invisibility\n'
      printf '`#[ignore]` was banned for. Either it now runs (move it off this list)\n'
      printf 'or its runtime gate lost its announcement.\n'
      failures=$((failures + 1))
    else
      printf '  %-42s announced %d\n' "$target" "$found"
    fi
  done

  if [ "$checked" -eq 0 ]; then
    printf 'NON-VACUITY GUARD FAILED: the target list is empty, so this gate\n'
    printf 'asserted nothing while reporting success.\n'
    return 1
  fi
  if [ "$failures" -ne 0 ]; then
    printf '%d of %d costly suite(s) failed to announce.\n' "$failures" "$checked"
    return 1
  fi
  printf 'costly announcements ok: %d suite(s), every one announced its skipped proof\n' "$checked"
  # 🔴 SAY THE DIRECTION OUT LOUD. A gated skip PASSES, so converting these off
  # `#[ignore]` RAISED the workspace pass total (3952 -> 3956: +5 now-running
  # announcers, -1 unrelated network failure) while coverage did not move at
  # all. A reader diffing the two totals concludes coverage improved. It did
  # not — honesty improved, and the true proven count is LOWER than either
  # total implies, because these five passes are known non-proofs.
  #
  # A METRIC THAT MOVES OPPOSITE TO THE TRUTH IT ENCODES IS WORSE THAN ONE THAT
  # DOES NOT MOVE. The gate cannot fix the harness's arithmetic, so it states
  # the correction next to the number, every run, where the reader is.
  printf '⚠️  those %d suite(s) PASSED WITHOUT PROVING ANYTHING. Subtract %d from any\n' \
    "$checked" "$checked"
  printf '    pass total before reading it as coverage — making a gap VISIBLE raises\n'
  printf '    the count, so a rising total here is not a coverage win.\n'
  return 0
}
