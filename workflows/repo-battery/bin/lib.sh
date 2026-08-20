#!/usr/bin/env bash
set -euo pipefail

json_string() {
  jq -Rn --arg value "$1" '$value'
}

run_root() {
  local workspace_root=$1
  local workflow_id=$2
  printf '%s/%s' "${workspace_root%/}" "$workflow_id"
}

subject_tree() {
  local workspace_root=$1
  local workflow_id=$2
  printf '%s/repo' "$(run_root "$workspace_root" "$workflow_id")"
}

refuse_json() {
  local reason=$1
  jq -cn --arg reason "$reason" '{ok:false, refusal:$reason}'
}

sha256_file() {
  sha256sum "$1" | awk '{print $1}'
}

secret_shaped() {
  case "$1" in
    *TOKEN*|*SECRET*|*PASSWORD*|*PASSWD*|*PRIVATE*|*CREDENTIAL*|*API_KEY*|*ACCESS_KEY*) return 0 ;;
    *) return 1 ;;
  esac
}

require_directory() {
  local path=$1
  if [[ ! -d "$path" ]]; then
    printf 'not a directory: %s\n' "$path" >&2
    return 1
  fi
}
