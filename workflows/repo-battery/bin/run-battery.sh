#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

if [[ $# -ne 2 ]]; then
  refuse_json "usage: run-battery.sh WORKSPACE_ROOT WORKFLOW_ID"
  exit 0
fi

workspace_root=$1
workflow_id=$2
tree=$(subject_tree "$workspace_root" "$workflow_id")
run_dir=$(run_root "$workspace_root" "$workflow_id")
log_path="$run_dir/battery.log"
result_path="$run_dir/battery-result.json"
mkdir -p "$run_dir"

if [[ ! -f "$tree/scripts/battery.sh" || ! -r "$tree/scripts/battery.sh" ]]; then
  jq -cn --arg log_path "$log_path" \
    '{measured:false, cancelled:false, rc:126, contended:false, log_path:$log_path, refusal:"scripts/battery.sh is absent or unreadable"}'
  exit 0
fi

if [[ -f "$run_dir/runtime-environment.json" ]]; then
  while IFS=$'\t' read -r name value; do
    printf -v "$name" '%s' "$value"
    export "$name"
  done < <(jq -r '.[] | [.name, .value] | @tsv' "$run_dir/runtime-environment.json")
fi
if only=$(worker_env_value AION_BATTERY_ONLY); then
  AION_BATTERY_ONLY=$only
  export AION_BATTERY_ONLY
fi

child_pid=""
cancelled=false
finish_cancelled() {
  cancelled=true
  if [[ -n "$child_pid" ]]; then
    kill -TERM -- "-$child_pid" 2>/dev/null || true
    wait "$child_pid" 2>/dev/null || true
  fi
  jq -cn --arg log_path "$log_path" \
    '{measured:false, cancelled:true, rc:130, contended:false, log_path:$log_path, refusal:"CANCELLED MEASUREMENT: the contained battery process group was terminated before a result existed"}' >"$result_path"
  cat "$result_path"
  exit 0
}
trap finish_cancelled INT TERM HUP

set +e
(
  cd "$tree"
  exec setsid bash "$tree/scripts/battery.sh"
) >"$log_path" 2>&1 &
child_pid=$!
wait "$child_pid"
rc=$?
set -e

contended=false
if grep -qE 'Too many open files|LLVM ERROR|signal: 11|SIGSEGV|timed out waiting for shard actor' "$log_path"; then
  contended=true
fi

jq -cn \
  --argjson rc "$rc" \
  --argjson contended "$contended" \
  --arg log_path "$log_path" \
  '{measured:true, cancelled:false, rc:$rc, contended:$contended, log_path:$log_path, refusal:""}' | tee "$result_path"
