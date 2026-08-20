#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

if [[ $# -ne 1 ]]; then
  refuse_json "usage: census-declaration.sh TREE_PATH"
  exit 0
fi

tree=$1
selector_path="$tree/gates.json:.legs"
if [[ ! -f "$tree/gates.json" ]]; then
  jq -cn --arg selector_path "$selector_path" \
    '{ok:false, kind:"undeclared", refusal:"the provisioned tree has no gates.json; no contract, no green", selector_path:$selector_path, gates_json_sha256:"", battery_sha256:"", legs_declared:[], environment_names:[]}'
  exit 0
fi

if ! jq -e '.legs | type == "array"' "$tree/gates.json" >/dev/null 2>&1; then
  jq -cn --arg selector_path "$selector_path" \
    '{ok:false, kind:"invalid_declaration", refusal:"gates.json:.legs is not an array", selector_path:$selector_path, gates_json_sha256:"", battery_sha256:"", legs_declared:[], environment_names:[]}'
  exit 0
fi

legs_declared=$(jq -c '[.legs[].name]' "$tree/gates.json")
leg_count=$(jq 'length' <<<"$legs_declared")
if [[ "$leg_count" -eq 0 ]]; then
  jq -cn --arg selector_path "$selector_path" \
    '{ok:false, kind:"zero_legs", refusal:("typed zero-leg refusal: selector " + $selector_path + " resolved to zero legs"), selector_path:$selector_path, gates_json_sha256:"", battery_sha256:"", legs_declared:[], environment_names:[]}'
  exit 0
fi

absolute_leg=$(jq -r '
  .legs[]
  | select((.cmd // "") | test("(^|[^A-Za-z0-9_$])/[A-Za-z0-9._~]"))
  | .name
' "$tree/gates.json" | head -n 1)
if [[ -n "$absolute_leg" ]]; then
  jq -cn --arg selector_path "$selector_path" --arg leg "$absolute_leg" \
    '{ok:false, kind:"absolute_path", refusal:("declared leg " + $leg + " contains a literal absolute path in its command"), selector_path:$selector_path, gates_json_sha256:"", battery_sha256:"", legs_declared:[], environment_names:[]}'
  exit 0
fi

battery="$tree/scripts/battery.sh"
verify="$tree/scripts/gates/verify-declaration.sh"
lib="$tree/scripts/gates/lib.sh"
announcements="$tree/scripts/battery-costly-announcements.sh"
for required in "$battery" "$verify" "$lib" "$announcements"; do
  if [[ ! -f "$required" ]]; then
    jq -cn --arg selector_path "$selector_path" --arg required "$required" \
      '{ok:false, kind:"missing_instrument", refusal:("required battery instrument is absent: " + $required), selector_path:$selector_path, gates_json_sha256:"", battery_sha256:"", legs_declared:[], environment_names:[]}'
    exit 0
  fi
done

environment_names=$(jq -c '[.legs[] | (.environment // [])[]] | unique' "$tree/gates.json")
gates_sha=$(sha256_file "$tree/gates.json")
battery_sha=$(sha256_file "$battery")

jq -cn \
  --arg selector_path "$selector_path" \
  --arg gates_json_sha256 "$gates_sha" \
  --arg battery_sha256 "$battery_sha" \
  --argjson legs_declared "$legs_declared" \
  --argjson environment_names "$environment_names" \
  '{ok:true, kind:"declared", refusal:"", selector_path:$selector_path, gates_json_sha256:$gates_json_sha256, battery_sha256:$battery_sha256, legs_declared:$legs_declared, environment_names:$environment_names}'
