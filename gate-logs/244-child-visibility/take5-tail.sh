#!/bin/bash
# take5-tail: the legs take-4 never reached (killed at gates:costly-announced in
# the 2026-08-16 disk emergency), plus the haematite solo adjudication run.
# Same tree (b0214c0ef rebased onto 051b547a6), same leg semantics as
# scripts/battery.sh (posture_control and the paired-coverage control copied
# faithfully). Take-4's log holds the first 8 legs' verdicts; this log holds the
# rest. The two artifacts together are the #244 landing battery.
set -u
REPO_ROOT="/Users/tom/Developer/ablative/stack/aion/.worktrees/244-child-visibility"
cd "$REPO_ROOT" || exit 90

GATE_NAMES=()
GATE_CODES=()

gate() {
  local name="$1"; shift
  printf '\n=== %s ===\n' "$name"
  "$@"
  local code=$?
  printf -- '--- %s exit %s ---\n' "$name" "$code"
  GATE_NAMES+=("$name"); GATE_CODES+=("$code")
}

gate_in() {
  local name="$1" dir="$2"; shift 2
  printf '\n=== %s ===\n' "$name"
  ( cd "$REPO_ROOT/$dir" && "$@" )
  local code=$?
  printf -- '--- %s exit %s ---\n' "$name" "$code"
  GATE_NAMES+=("$name"); GATE_CODES+=("$code")
}

# Paired-coverage control, semantics per battery.sh: every target-start line
# must be followed by a `test result:` before the next target line.
paired_test() {
  local tmp code counts targets unpaired
  tmp=$(mktemp)
  "$@" 2>&1 | tee "$tmp"
  code=${PIPESTATUS[0]}
  counts=$(/usr/bin/awk '
    /^[[:space:]]+Running |^[[:space:]]+Doc-tests / { if (started) unpaired++; started = 1; targets++ }
    /test result:/ { started = 0 }
    END { if (started) unpaired++; print targets + 0, unpaired + 0 }' "$tmp")
  rm -f "$tmp"
  targets=${counts%% *}
  unpaired=${counts##* }
  if [ "$targets" -eq 0 ]; then
    printf 'COVERAGE CONTROL FAILED: the run started ZERO test targets.\n'
    return 1
  fi
  if [ "$unpaired" -ne 0 ]; then
    printf 'COVERAGE CONTROL FAILED: %d of %d target(s) started and never reported a result.\n' "$unpaired" "$targets"
    return 1
  fi
  printf 'coverage control ok: %d targets started, %d reached a result\n' "$targets" "$targets"
  return "$code"
}

# posture_control copied from battery.sh (behaviour-detected feature posture).
posture_control() {
  local label="$1" expect="$2" forbid="$3"
  shift 3
  local out code
  out=$("$@" 2>&1)
  code=$?
  if [ "$code" -ne 0 ]; then
    printf '%s\n' "$out"
    printf 'POSTURE CONTROL (%s): the run itself failed with exit %d\n' "$label" "$code"
    return "$code"
  fi
  if ! printf '%s' "$out" | grep -q -- "$expect"; then
    printf 'POSTURE CONTROL FAILED (%s): expected announcement `%s` never appeared.\n' "$label" "$expect"
    return 1
  fi
  if printf '%s' "$out" | grep -q -- "$forbid"; then
    printf 'POSTURE CONTROL FAILED (%s): forbidden announcement `%s` appeared.\n' "$label" "$forbid"
    return 1
  fi
  printf 'posture control ok (%s): %s\n' "$label" "$expect"
  return 0
}

# --- Leg 0: haematite solo adjudication (take-4 all-features sole red) ---------
# The two shard-actor timeouts under 3-concurrent-build contention. Solo run,
# post-emergency box. Contention pushes ordering assertions toward failure, so
# a green here under lighter load adjudicates the take-4 reds as contention-class.
gate "adjudication:haematite-solo" paired_test cargo test -p aion-store-haematite --lib --no-fail-fast

# --- Legs take-4 never reached -------------------------------------------------
source "$REPO_ROOT/scripts/battery-costly-announcements.sh"
gate "gates:costly-announced" costly_announcements

gate "posture:serde-json-sorted" posture_control "aion-package ALONE, default features" \
  "SERDE_JSON_MAP=key_sorted" \
  "SERDE_JSON_MAP=insertion_ordered" \
  cargo test -p aion-package --lib build_coverage:: -- --nocapture

gate "posture:serde-json-insertion" posture_control "aion-package, preserve_order forced ON" \
  "SERDE_JSON_MAP=insertion_ordered" \
  "SERDE_JSON_MAP=key_sorted" \
  cargo test -p aion-package --features serde_json/preserve_order --lib build_coverage:: -- --nocapture

gate "rust:preserve-order-pins" paired_test \
  cargo test -p aion-package -p aion-awl --features serde_json/preserve_order --no-fail-fast

gate_in "gleam:aion-flow" gleam/aion_flow gleam test
gate_in "gleam:assistant" examples/assistant gleam test
gate_in "gleam:agent-dev" examples/agent-dev gleam test

gate_in "console:typecheck" apps/aion-ops-console bun run typecheck
gate_in "console:test" apps/aion-ops-console bun test

gate_in "gates:worker-crate" workflows/gates/worker bash -c '
  first=0
  cargo fmt --check; s=$?; [ "$first" -eq 0 ] && first=$s
  cargo clippy --all-targets -- -D warnings; s=$?; [ "$first" -eq 0 ] && first=$s
  cargo test; s=$?; [ "$first" -eq 0 ] && first=$s
  exit "$first"'

gate_in "python:aion-worker" sdks/python/aion-worker uv run --extra dev python -m pytest tests/ -q
gate_in "python:aion-client" sdks/python/aion-client uv run --extra dev python -m pytest tests/ -q

gate_in "ts-worker:proto-drift" sdks/typescript/aion-worker bun run check:proto
gate_in "ts-worker:typecheck" sdks/typescript/aion-worker bun run typecheck
gate_in "ts-worker:test" sdks/typescript/aion-worker bun test

# --- Manifest ------------------------------------------------------------------
printf '\n================ TAIL MANIFEST ================\n'
failed=0
index=0
while [ "$index" -lt "${#GATE_NAMES[@]}" ]; do
  name="${GATE_NAMES[$index]}"
  code="${GATE_CODES[$index]}"
  if [ "$code" -eq 0 ]; then
    printf '  PASS  %-28s exit %s\n' "$name" "$code"
  else
    printf '  FAIL  %-28s exit %s\n' "$name" "$code"
    failed=$((failed + 1))
  fi
  index=$((index + 1))
done
printf '===============================================\n'
printf '%d of %d gates failed.\n' "$failed" "${#GATE_NAMES[@]}"
[ "$failed" -ne 0 ] && exit 1
exit 0
