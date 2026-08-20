#!/usr/bin/env bash
set -euo pipefail

proof=${1:-}
if [[ "$proof" == "3-unproven" && "${AION_BATTERY_ONLY:-}" != "fixture:true" ]]; then
  exec env AION_BATTERY_ONLY=fixture:true "$0" "$proof"
fi
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)
proof_root="$repo/workflows/repo-battery/proofs"
surface="$proof_root/evidence/worker-surface.json"
scratch="/tmp/repo-battery-proof-$proof"
rm -rf "$scratch"
mkdir -p "$scratch"

command_for() { jq -r --arg action "$1" '.actions[] | select(.name == $action) | .command' "$surface"; }
prepare_action() {
  local action=$1
  shift
  local command
  command=$(command_for "$action")
  mapfile -d '' -t ACTION_ARGV < <(python3 - "$command" "$@" <<'PY'
import shlex
import sys

command = sys.argv[1]
values = dict(item.split("=", 1) for item in sys.argv[2:])
for token in shlex.split(command):
    if token.startswith("$") and token[1:] in values:
        token = values[token[1:]]
    sys.stdout.write(token)
    sys.stdout.write("\0")
PY
  )
}
run_action() {
  local action=$1
  shift
  local command
  command=$(command_for "$action")
  printf '$ %s\n' "$command"
  prepare_action "$action" "$@"
  env -i PATH="$PATH" "${ACTION_ARGV[@]}"
}
build() {
  local fixture=$1
  "$proof_root/build-fixture.sh" "$fixture" "$repo" "$scratch/source"
}
full_measurement() {
  local only=${1:-}
  run_action provision subject_repo="$scratch/source" ref_requested=HEAD workspace_root="$scratch/work" workflow_id=run
  run_action precheck workspace_root="$scratch/work" workflow_id=run
  if [[ -z "$only" || "$AION_BATTERY_ONLY" == "$only" ]]; then
    run_action battery workspace_root="$scratch/work" workflow_id=run
  else
    printf 'proof selector did not reach the harness process environment\n' >&2
    exit 1
  fi
  run_action verify workspace_root="$scratch/work" workflow_id=run
  run_action receipt subject_repo="$scratch/source" ref_requested=HEAD workspace_root="$scratch/work" workflow_id=run waivers='[]'
}

case "$proof" in
  1-green)
    build green
    full_measurement
    ;;
  2-red)
    build red
    full_measurement
    ;;
  3-unproven)
    build green
    full_measurement fixture:true
    ;;
  4-undeclared)
    build no-declaration
    run_action provision subject_repo="$scratch/source" ref_requested=HEAD workspace_root="$scratch/work" workflow_id=run
    run_action precheck workspace_root="$scratch/work" workflow_id=run
    ;;
  5-absolute-path)
    build absolute-path
    run_action provision subject_repo="$scratch/source" ref_requested=HEAD workspace_root="$scratch/work" workflow_id=run
    run_action precheck workspace_root="$scratch/work" workflow_id=run
    ;;
  7-cancel)
    build green
    jq '.legs[0].cmd = "sleep 30"' "$scratch/source/gates.json" >"$scratch/source/gates.tmp"
    mv "$scratch/source/gates.tmp" "$scratch/source/gates.json"
    git -C "$scratch/source" add gates.json
    git -C "$scratch/source" commit -qm 'proof: slow battery'
    run_action provision subject_repo="$scratch/source" ref_requested=HEAD workspace_root="$scratch/work" workflow_id=run
    run_action precheck workspace_root="$scratch/work" workflow_id=run
    printf '$ %s & pid=$!; kill -TERM "$pid"; wait "$pid"\n' "$(command_for battery)"
    prepare_action battery workspace_root="$scratch/work" workflow_id=run
    env -i PATH="$PATH" "${ACTION_ARGV[@]}" >"$scratch/cancel.out" &
    pid=$!
    sleep 1
    kill -TERM "$pid"
    wait "$pid"
    cat "$scratch/cancel.out"
    printf 'post-cancel result: '
    cat "$scratch/work/run/battery-result.json"
    if grep -q '^EXECUTED_LEGS ' "$scratch/work/run/battery.log"; then
      printf 'unexpected completed leg manifest after cancellation\n' >&2
      exit 1
    fi
    printf 'no completed EXECUTED_LEGS manifest after cancellation\n'
    run_action teardown workspace_root="$scratch/work" workflow_id=run
    ;;
  8a-tree-source)
    build green
    mkdir -p "$scratch/work"
    printf '%s\n' '{"legs":[{"name":"venue:wrong","cmd":"false"}]}' >"$scratch/work/gates.json"
    run_action provision subject_repo="$scratch/source" ref_requested=HEAD workspace_root="$scratch/work" workflow_id=run
    run_action precheck workspace_root="$scratch/work" workflow_id=run
    printf 'tree legs: '
    jq -c '.legs_declared' "$scratch/work/run/census.json"
    printf 'venue legs: '
    jq -c '[.legs[].name]' "$scratch/work/gates.json"
    ;;
  8b-perturbations)
    build green
    mkdir -p "$scratch/escape"
    printf '$ bash workflows/repo-battery/bin/teardown.sh %s %s\n' "$scratch/work" '../escape'
    bash "$repo/workflows/repo-battery/bin/teardown.sh" "$scratch/work" '../escape'
    cp "$repo/workflows/repo-battery/bin/census-declaration.sh" "$scratch/census-perturbed.sh"
    cp "$repo/workflows/repo-battery/bin/lib.sh" "$scratch/lib.sh"
    sed -i 's/\.legs/\.missing/g' "$scratch/census-perturbed.sh"
    printf 'source_sha256 %s\n' "$(sha256sum "$repo/workflows/repo-battery/bin/census-declaration.sh" | awk '{print $1}')"
    printf '$ bash census-perturbed.sh %s\n' "$scratch/source"
    bash "$scratch/census-perturbed.sh" "$scratch/source"
    ;;
  8c-worktree)
    git -C "$repo" worktree add --detach "$scratch/venue" HEAD >/dev/null
    printf 'venue show-toplevel %s\n' "$(git -C "$scratch/venue" rev-parse --show-toplevel)"
    printf 'venue git-common-dir %s\n' "$(git -C "$scratch/venue" rev-parse --git-common-dir)"
    printf '$ %s 1-green\n' "$scratch/venue/workflows/repo-battery/proofs/run-offline-proof.sh"
    "$scratch/venue/workflows/repo-battery/proofs/run-offline-proof.sh" 1-green
    ;;
  *)
    printf 'unknown proof: %s\n' "$proof" >&2
    exit 2
    ;;
esac
