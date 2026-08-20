#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

if [[ $# -ne 4 ]]; then
  refuse_json "usage: provision.sh SUBJECT_REPO REF_REQUESTED WORKSPACE_ROOT WORKFLOW_ID"
  exit 0
fi

subject_repo=$1
ref_requested=$2
workspace_root=$3
workflow_id=$4
run_dir=$(run_root "$workspace_root" "$workflow_id")
tree=$(subject_tree "$workspace_root" "$workflow_id")

if [[ -e "$run_dir" ]]; then
  jq -cn --arg refusal "destination already exists: $run_dir; a leftover tree from a collided dispatch is the dispatcher's to inspect" \
    '{ok:false, owns_tree:false, refusal:$refusal, tree_path:"", head_commit:""}'
  exit 0
fi

mkdir -p "$workspace_root"
if ! git clone --quiet --no-checkout -- "$subject_repo" "$tree" 2>"$workspace_root/.repo-battery-clone-error"; then
  clone_error=$(cat "$workspace_root/.repo-battery-clone-error")
  rm -f "$workspace_root/.repo-battery-clone-error"
  owns_tree=false
  tree_path=""
  if [[ -e "$run_dir" || -e "$tree" ]]; then
    owns_tree=true
    tree_path=$tree
  fi
  jq -cn --argjson owns_tree "$owns_tree" --arg tree_path "$tree_path" \
    --arg refusal "clone failed: $clone_error" \
    '{ok:false, owns_tree:$owns_tree, refusal:$refusal, tree_path:$tree_path, head_commit:""}'
  exit 0
fi
rm -f "$workspace_root/.repo-battery-clone-error"

if ! git -C "$tree" checkout --quiet --detach "$ref_requested" 2>"$run_dir/checkout-error"; then
  checkout_error=$(cat "$run_dir/checkout-error")
  jq -cn --arg refusal "checkout failed for ref $ref_requested: $checkout_error" --arg tree_path "$tree" \
    '{ok:false, owns_tree:true, refusal:$refusal, tree_path:$tree_path, head_commit:""}' | tee "$run_dir/provision.json"
  exit 0
fi
rm -f "$run_dir/checkout-error"
head_commit=$(git -C "$tree" rev-parse HEAD)

jq -cn --arg head_commit "$head_commit" --arg tree_path "$tree" \
  '{ok:true, owns_tree:true, refusal:"", tree_path:$tree_path, head_commit:$head_commit}' | tee "$run_dir/provision.json"
