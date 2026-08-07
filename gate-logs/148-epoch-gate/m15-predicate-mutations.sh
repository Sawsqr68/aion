#!/bin/zsh
# m15: RUN the three predicate mutations the source comment claims, rather than
# reasoning about them. Each one is applied with the literal prior text, the
# module's tests are run to a FILE, and the file is restored and proven
# byte-identical before the next.
#
# NO set -e: red test runs are the EXPECTED outcome and must not abort the
# script mid-mutation (that abort is how a previous run left a mutation live).
SP=/private/tmp/claude-501/-Users-tom-Developer-ablative-stack-aion/5849e0d8-4802-4869-8e0f-9f7fd187e198/scratchpad
F=/Users/tom/Developer/ablative/stack/aion/crates/aion/src/runtime/nif_continue_as_new.rs
PY=/opt/homebrew/bin/python3.13
# 🔴 The baseline is PINNED, not read off the tree: a script that blesses
# whatever bytes it finds cannot notice it is starting from a corrupted file.
EXPECTED=3de2e869131ca5c1ffa9504da6f55b5aa7b7c626
BASE=$(shasum "$F" | awk '{print $1}')
if [[ "$BASE" != "$EXPECTED" ]]; then
  echo "ABORT: tree is not at the pinned baseline ($BASE != $EXPECTED)"
  exit 1
fi
echo "BASELINE $BASE (pinned)"

# Vacuity control: the baseline must be GREEN, or a red mutation measures nothing.
cargo test -p aion-rs --lib runtime::nif_continue_as_new > "$SP/m15-baseline.log" 2>&1
if [[ $? -ne 0 ]]; then
  echo "ABORT: baseline is RED — no mutation result is interpretable"
  grep -E "^test result" "$SP/m15-baseline.log"
  exit 1
fi
grep -E "^test result" "$SP/m15-baseline.log"

run_one () {
  local name="$1" target="$2" replacement="$3"
  echo "=== $name ==="
  $PY "$SP/mutate.py" "$F" apply "$target" "$replacement" || { echo "$name: NON-RUN"; return; }
  cargo test -p aion-rs --lib runtime::nif_continue_as_new > "$SP/m15-$name.log" 2>&1
  local code=$?
  echo "$name EXIT=$code"
  grep -E "^test result" "$SP/m15-$name.log" || echo "$name: NO VERDICT LINE"
  echo "--- failing tests ---"
  sed -n '/^failures:$/,/^test result/p' "$SP/m15-$name.log" | grep -E "^    " | sort -u
  mv "$F.orig" "$F"
  local now=$(shasum "$F" | awk '{print $1}')
  if [[ "$now" == "$BASE" ]]; then
    echo "$name RESTORED byte-identical"
  else
    echo "$name RESTORE MISMATCH $now"
    exit 1
  fi
}

run_one "a-every-error-ends" \
'        Err(error) => matches!(
            error,
            NifContextError::Durability(DurabilityError::EngineTaskEpochClosed { .. })
        ),' \
'        Err(_) => true,'

run_one "b-wrong-variant" \
'DurabilityError::EngineTaskEpochClosed { .. }
        ),' \
'DurabilityError::HistoryShape { .. }
        ),'

run_one "c-ok-narrowed-to-complete" \
'        Ok(_) => true,' \
'        Ok(ContinuationOutcome::Complete) => true,
        Ok(_) => false,'

echo "=== FINAL $(shasum "$F" | awk '{print $1}') (baseline $BASE) ==="
