#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

if [[ $# -ne 2 ]]; then
  refuse_json "usage: teardown.sh WORKSPACE_ROOT WORKFLOW_ID"
  exit 0
fi

workspace_root=$1
workflow_id=$2
run_dir=$(run_root "$workspace_root" "$workflow_id")
resolved_workspace=$(realpath -m "$workspace_root")
resolved_run=$(realpath -m "$run_dir")

case "$resolved_run" in
  "$resolved_workspace"/*) ;;
  *)
    jq -cn --arg refusal "teardown refused path outside workspace root: $resolved_run" \
      '{ok:false, removed:"", refusal:$refusal}'
    exit 0
    ;;
esac

if [[ ! -d "$resolved_run" ]]; then
  jq -cn --arg refusal "teardown refused absent run-owned tree: $resolved_run" \
    '{ok:false, removed:"", refusal:$refusal}'
  exit 0
fi

rm -rf --one-file-system -- "$resolved_run"
jq -cn --arg removed "$resolved_run" '{ok:true, removed:$removed, refusal:""}'
