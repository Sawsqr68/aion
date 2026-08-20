#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

if [[ $# -ne 5 ]]; then
  refuse_json "usage: receipt.sh SUBJECT_REPO REF_REQUESTED WORKSPACE_ROOT WORKFLOW_ID WAIVERS_JSON"
  exit 0
fi

subject_repo=$1
ref_requested=$2
workspace_root=$3
workflow_id=$4
waivers=$5
run_dir=$(run_root "$workspace_root" "$workflow_id")
for artifact in provision.json census.json precheck.json battery-result.json verify.json; do
  if [[ ! -f "$run_dir/$artifact" ]]; then
    jq -cn --arg refusal "receipt artifact absent: $run_dir/$artifact" \
      '{ok:false, disposition:"refused", refusal:$refusal}'
    exit 0
  fi
done

receipt=$(jq -cn \
  --arg subject_repo "$subject_repo" --arg ref_requested "$ref_requested" \
  --slurpfile p "$run_dir/provision.json" --slurpfile c "$run_dir/census.json" \
  --slurpfile pre "$run_dir/precheck.json" --slurpfile v "$run_dir/verify.json" \
  --argjson waivers "$waivers" \
  '{subject_repo:$subject_repo, ref_requested:$ref_requested,
    head_commit:$p[0].head_commit, tree_path:$p[0].tree_path,
    gates_json_sha256:$c[0].gates_json_sha256, battery_sha256:$c[0].battery_sha256,
    script_witnesses:($pre[0].script_witnesses | sort_by(.path)), venue:$pre[0].venue,
    environment:$pre[0].environment, legs_declared:$c[0].legs_declared,
    legs_executed:$v[0].legs_executed, legs:$v[0].legs,
    verify_verdict:$v[0].verify_verdict, waivers:$waivers, verdict:$v[0].verdict}')
disposition="measured"
if [[ $(jq -r '.verdict' <<<"$receipt") == "unproven" ]]; then disposition="unproven"; fi
jq -cn --arg disposition "$disposition" --argjson receipt "$receipt" \
  '{ok:true, disposition:$disposition, refusal:"", receipt:$receipt}' | tee "$run_dir/receipt.json"
