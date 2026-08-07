#!/bin/zsh
# R21 obligation: re-run ALL SIX mutations (3 predicate-arm + 3 gate) with the
# FIXED harness, against the post-R21-fix bytes. Protocol, each rule purchased:
# - NO set -e: red test runs are the EXPECTED outcome and must not abort the
#   script mid-mutation (that abort is how a run once left a mutation live).
# - The baseline is PINNED, not read off the tree.
# - Baseline suite proven GREEN before any mutation (vacuity control).
# - Restores go through mutate.py `restore`, which WRITES bytes (fresh mtime →
#   forces the rebuild). mv-restore is what served Review 21 a stale mutant
#   binary and five phantom "nondeterministic" failures.
# - The session ENDS with a green re-run of the REBUILT tree — the only proof
#   the binary matches the source again.
SP=/private/tmp/claude-501/-Users-tom-Developer-ablative-stack-aion/5849e0d8-4802-4869-8e0f-9f7fd187e198/scratchpad
F=/Users/tom/Developer/ablative/stack/aion/crates/aion/src/runtime/nif_continue_as_new.rs
PY=/opt/homebrew/bin/python3.13
EXPECTED=ef9a764b8fd637c5471eba2055c71279f512dca6
BASE=$(shasum "$F" | awk '{print $1}')
if [[ "$BASE" != "$EXPECTED" ]]; then
  echo "ABORT: tree is not at the pinned baseline ($BASE != $EXPECTED)"
  exit 1
fi
if [[ -e "$F.orig" ]]; then
  echo "ABORT: a backup already exists — a previous run did not restore"
  exit 1
fi
echo "BASELINE $BASE (pinned)"

cargo test -p aion-rs --lib runtime::nif_continue_as_new > "$SP/r21mut-baseline.log" 2>&1
if [[ $? -ne 0 ]]; then
  echo "ABORT: baseline is RED — no mutation result is interpretable"
  grep -E "^test result" "$SP/r21mut-baseline.log"
  exit 1
fi
echo "BASELINE GREEN:"
grep -E "^test result" "$SP/r21mut-baseline.log"

run_one () {
  local name="$1" target="$2" replacement="$3"
  echo "=== $name ==="
  $PY "$SP/mutate.py" "$F" apply "$target" "$replacement" || { echo "$name: NON-RUN"; return; }
  cargo test -p aion-rs --lib runtime::nif_continue_as_new > "$SP/r21mut-$name.log" 2>&1
  local code=$?
  echo "$name EXIT=$code"
  grep -E "^test result" "$SP/r21mut-$name.log" || echo "$name: NO VERDICT LINE"
  echo "--- failing tests ---"
  sed -n '/^failures:$/,/^test result/p' "$SP/r21mut-$name.log" | grep -E "^    " | sort -u
  $PY "$SP/mutate.py" "$F" restore || { echo "$name RESTORE FAILED"; exit 1; }
  local now=$(shasum "$F" | awk '{print $1}')
  if [[ "$now" == "$BASE" ]]; then
    echo "$name RESTORED byte-identical (bytes written, mtime fresh)"
  else
    echo "$name RESTORE MISMATCH $now"
    exit 1
  fi
}

# --- predicate-arm mutations (m15) ---
run_one "a-every-error-ends" \
'        Err(error) => matches!(
            error,
            NifContextError::Durability(DurabilityError::EngineTaskEpochClosed { .. })
        ),' \
'        Err(_) => true,'

run_one "b-wrong-variant" \
'DurabilityError::EngineTaskEpochClosed { .. })
        ),' \
'DurabilityError::HistoryShape { .. })
        ),'

run_one "c-ok-narrowed-to-complete" \
'        Ok(_) => true,' \
'        Ok(ContinuationOutcome::Complete) => true,
        Ok(_) => false,'

# --- gate mutations (M5/M6/M7; M5 is the reviewer's exact pre-remediation form) ---
run_one "M5-reviewer-is-ok" 'if outcome_must_end_the_process(&recorded) {' 'if recorded.is_ok() {'
run_one "M6-never-fires"    'if outcome_must_end_the_process(&recorded) {' 'if false {'
run_one "M7-always-fires"   'if outcome_must_end_the_process(&recorded) {' 'if true {'

echo "=== FINAL SHA $(shasum "$F" | awk '{print $1}') (baseline $BASE) ==="

# The closing control: rebuild + green on the restored tree. Without this the
# binary is unproven and the whole session is one mv away from Review 21.
cargo test -p aion-rs --lib runtime::nif_continue_as_new > "$SP/r21mut-final-green.log" 2>&1
FINAL=$?
echo "FINAL GREEN RE-RUN EXIT=$FINAL"
grep -E "^test result" "$SP/r21mut-final-green.log"
if [[ $FINAL -ne 0 ]]; then
  echo "🔴 SESSION NOT CLOSED: the rebuilt tree is not green"
  exit 1
fi
echo "SESSION CLOSED GREEN"
