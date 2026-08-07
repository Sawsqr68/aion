#!/bin/zsh
# R22 formal evidence session: all four gates + EIGHT mutations, on a
# measured-quiet box. Every rule here was purchased:
# - BOX EXCLUSIVITY IS A GATE PRECONDITION (estate ruling 2026-08-07): the
#   1-minute load average is measured before the session and recorded beside
#   every result; the session REFUSES to start oversubscribed. A refusal needs
#   no assertion-polarity classification; a discount does.
# - NO set -e: red test runs are the EXPECTED outcome of a mutation.
# - Baselines PINNED, not read off the tree.
# - Baseline module run proven GREEN before any mutation.
# - Restores via mutate.py restore (writes bytes, fresh mtime, forces rebuild).
# - The session ENDS with a green re-run of the REBUILT tree.
SP=/private/tmp/claude-501/-Users-tom-Developer-ablative-stack-aion/5849e0d8-4802-4869-8e0f-9f7fd187e198/scratchpad
F=/Users/tom/Developer/ablative/stack/aion/crates/aion/src/runtime/nif_continue_as_new.rs
FT=/Users/tom/Developer/ablative/stack/aion/crates/aion/src/runtime/nif_continue_as_new_tests.rs
PY=/opt/homebrew/bin/python3.13
EXPECTED_F=8431b5729df2d773a47ba36ecf5aac3723b17a92
EXPECTED_FT=938c41e70eefb5746ee32036a3cd3eb69bc552a8
NCPU=$(sysctl -n hw.ncpu)

load_now () { sysctl -n vm.loadavg | awk '{print $2}' }

# Ruled preconditions (estate-wide, 2026-08-07), BOTH of equal rank — a gate
# with two preconditions can be blocked by the one nobody is watching:
# 1. LOAD: refuse when 1-min load > cores, both measured at run time, THREE
#    CONSECUTIVE SAMPLES — one sample of a moving gauge is one sample.
# 2. DISK: refuse below the floor. Band floor 25 GiB + 3 GiB margin = 28 GiB
#    free on /System/Volumes/Data; the refusal cites free · floor · margin.
DISK_FLOOR_GIB=25
DISK_MARGIN_GIB=3
free_gib () { df -k /System/Volumes/Data | awk 'NR==2 {printf "%.2f", $4/1048576}' }
for sample in 1 2 3; do
  LOAD=$(load_now)
  FREE=$(free_gib)
  echo "PRECONDITION sample ${sample}: load=${LOAD} ncpu=${NCPU} disk_free=${FREE}GiB"
  if (( $(echo "$LOAD > $NCPU" | bc -l) )); then
    echo "REFUSED: box is oversubscribed (load ${LOAD} > ${NCPU} cores) — no evidence taken"
    exit 9
  fi
  if (( $(echo "$FREE < $DISK_FLOOR_GIB + $DISK_MARGIN_GIB" | bc -l) )); then
    echo "REFUSED: disk below floor (free ${FREE} GiB · floor ${DISK_FLOOR_GIB} GiB · margin ${DISK_MARGIN_GIB} GiB) — no evidence taken"
    exit 9
  fi
  [[ $sample -lt 3 ]] && sleep 10
done

BASE=$(shasum "$F" | awk '{print $1}')
BASET=$(shasum "$FT" | awk '{print $1}')
if [[ "$BASE" != "$EXPECTED_F" || "$BASET" != "$EXPECTED_FT" ]]; then
  echo "ABORT: tree not at pinned baselines ($BASE / $BASET)"
  exit 1
fi
if [[ -e "$F.orig" ]]; then
  echo "ABORT: a backup already exists — a previous run did not restore"
  exit 1
fi
echo "BASELINES $BASE / $BASET (pinned)"

# ---- Gates, each with the load at its start recorded ----
echo "== GATE fmt --check (load $(load_now)) =="
cargo fmt --check > "$SP/r22b-fmt.log" 2>&1; echo "fmt-check EXIT $?"
echo "== GATE clippy -p (load $(load_now)) =="
cargo clippy -p aion-rs --all-targets -- -D warnings > "$SP/r22b-clippy-p.log" 2>&1; echo "clippy-p EXIT $?"
echo "== GATE clippy --workspace (load $(load_now)) =="
cargo clippy --workspace --all-targets -- -D warnings > "$SP/r22b-clippy-ws.log" 2>&1; echo "clippy-ws EXIT $?"
for n in 1 2; do
  echo "== GATE suite run $n (load $(load_now)) =="
  cargo test -p aion-rs > "$SP/r22b-suite-$n.log" 2>&1
  echo "suite-$n EXIT $?  verdict-lines $(grep -c '^test result' "$SP/r22b-suite-$n.log")"
  grep "^test result" "$SP/r22b-suite-$n.log" | grep -v " 0 failed" || echo "suite-$n: NO FAILURES IN ANY BLOCK"
done

# ---- Mutation session ----
echo "== MUTATION BASELINE (load $(load_now)) =="
cargo test -p aion-rs --lib runtime::nif_continue_as_new > "$SP/r22b-mut-baseline.log" 2>&1
if [[ $? -ne 0 ]]; then
  echo "ABORT: mutation baseline RED"
  grep -E "^test result" "$SP/r22b-mut-baseline.log"
  exit 1
fi
grep -E "^test result" "$SP/r22b-mut-baseline.log"

run_one () {
  local name="$1" target="$2" replacement="$3"
  echo "=== $name (load $(load_now)) ==="
  $PY "$SP/mutate.py" "$F" apply "$target" "$replacement" || { echo "$name: NON-RUN"; return; }
  cargo test -p aion-rs --lib runtime::nif_continue_as_new > "$SP/r22b-mut-$name.log" 2>&1
  local code=$?
  echo "$name EXIT=$code"
  grep -E "^test result" "$SP/r22b-mut-$name.log" || echo "$name: NO VERDICT LINE"
  sed -n '/^failures:$/,/^test result/p' "$SP/r22b-mut-$name.log" | grep -E "^    runtime" | sort -u
  $PY "$SP/mutate.py" "$F" restore || { echo "$name RESTORE FAILED"; exit 1; }
  local now=$(shasum "$F" | awk '{print $1}')
  [[ "$now" == "$BASE" ]] && echo "$name RESTORED byte-identical" || { echo "$name RESTORE MISMATCH $now"; exit 1; }
}

# predicate-arm mutations
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
# gate mutations (M5 = the reviewer's pre-remediation form)
run_one "M5-reviewer-is-ok" 'if outcome_must_end_the_process(&recorded) {' 'if recorded.is_ok() {'
run_one "M6-never-fires"    'if outcome_must_end_the_process(&recorded) {' 'if false {'
run_one "M7-always-fires"   'if outcome_must_end_the_process(&recorded) {' 'if true {'
# R22's arm-skip mutations — the two the old test could not see
run_one "R22-terminalonly-skip" 'if outcome_must_end_the_process(&recorded) {' 'if outcome_must_end_the_process(&recorded)
        && !matches!(&recorded, Ok(ContinuationOutcome::TerminalOnly(_)))
    {'
run_one "R22-complete-skip" 'if outcome_must_end_the_process(&recorded) {' 'if outcome_must_end_the_process(&recorded)
        && !matches!(&recorded, Ok(ContinuationOutcome::Complete))
    {'

echo "=== FINAL SHA $(shasum "$F" | awk '{print $1}') (baseline $BASE) ==="
echo "== FINAL GREEN RE-RUN OF THE REBUILT TREE (load $(load_now)) =="
cargo test -p aion-rs --lib runtime::nif_continue_as_new > "$SP/r22b-final-green.log" 2>&1
FINAL=$?
echo "FINAL EXIT=$FINAL"
grep -E "^test result" "$SP/r22b-final-green.log"
[[ $FINAL -ne 0 ]] && { echo "🔴 SESSION NOT CLOSED"; exit 1; }
echo "SESSION CLOSED GREEN (load at close $(load_now))"
