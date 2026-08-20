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

worker_env_value() {
  local name=$1
  local entry
  # 🔴 `${!name+set}`, NOT `[[ -v "$name" ]]`. `-v` is a bash 4.2 unary operator
  # and bash 3.2 — which is what macOS ships, and still the only bash on some of
  # the machines this estate runs gates from — does not know it. It is not a
  # runtime failure that a guard could catch: `[[` is parsed as a compound
  # command, so bash 3.2 rejects the line at PARSE time with "conditional binary
  # operator expected", and sourcing this library fails outright. Every script in
  # this directory sources it, so all of them were unusable there, whether or not
  # they ever call this function.
  #
  # The replacement is exactly equivalent — `+set` expands for a variable that is
  # set INCLUDING when it is empty, which is precisely what `-v` tests — and was
  # checked both ways on bash 3.2 rather than assumed.
  if [[ -n "${!name+set}" ]]; then
    printf '%s' "${!name}"
    return 0
  fi
  [[ -r "/proc/$PPID/environ" ]] || return 1
  while IFS= read -r -d '' entry; do
    case "$entry" in
      "$name="*) printf '%s' "${entry#*=}"; return 0 ;;
    esac
  done <"/proc/$PPID/environ"
  return 1
}

require_directory() {
  local path=$1
  if [[ ! -d "$path" ]]; then
    printf 'not a directory: %s\n' "$path" >&2
    return 1
  fi
}
