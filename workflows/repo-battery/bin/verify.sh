#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

if [[ $# -ne 2 ]]; then
  refuse_json "usage: verify.sh WORKSPACE_ROOT WORKFLOW_ID"
  exit 0
fi

workspace_root=$1
workflow_id=$2
run_dir=$(run_root "$workspace_root" "$workflow_id")
tree=$(subject_tree "$workspace_root" "$workflow_id")
log_path="$run_dir/battery.log"
verify_log="$run_dir/verify.log"
result_path="$run_dir/verify.json"

if [[ ! -f "$log_path" || ! -r "$log_path" ]]; then
  jq -cn --arg refusal "battery log is absent or unreadable: $log_path" \
    '{measured:false, exact:false, verify_rc:2, verify_verdict:"", verdict:"", legs_executed:[], legs:[], refusal:$refusal}' | tee "$result_path"
  exit 0
fi

set +e
bash "$tree/scripts/gates/verify-declaration.sh" "$log_path" >"$verify_log" 2>&1
verify_rc=$?
set -e
verify_verdict=$(cat "$verify_log")
executed_line=$(grep '^EXECUTED_LEGS ' "$log_path" || true)
legs_executed=$(sed 's/^EXECUTED_LEGS //' <<<"$executed_line" | tr ' ' '\n' | awk 'NF' | jq -Rsc 'split("\n") | map(select(length > 0))')
legs='[]'
while read -r result name _ rc; do
  [[ "$result" == "PASS" || "$result" == "FAIL" ]] || continue
  status="green"
  [[ "$result" == "PASS" ]] || status="red"
  record=$(jq -cn --arg name "$name" --argjson rc "$rc" --arg status "$status" '{name:$name, rc:$rc, status:$status}')
  legs=$(jq -c --argjson record "$record" '. + [$record]' <<<"$legs")
done < <(sed -n '/^================ BATTERY MANIFEST ================/,/^=================================================/p' "$log_path" | sed '1d;$d')

measured=true
exact=false
verdict=""
refusal=""
if [[ "$verify_rc" -eq 0 ]]; then
  exact=true
  if jq -e 'all(.[]; .status == "green")' <<<"$legs" >/dev/null; then verdict="green"; else verdict="red"; fi
elif [[ "$verify_rc" -eq 1 ]]; then
  verdict="unproven"
else
  measured=false
  refusal="declaration verifier could not measure (rc $verify_rc): $verify_verdict"
fi
jq -cn --argjson measured "$measured" --argjson exact "$exact" --argjson verify_rc "$verify_rc" \
  --arg verify_verdict "$verify_verdict" --arg verdict "$verdict" \
  --arg refusal "$refusal" --argjson legs_executed "$legs_executed" --argjson legs "$legs" \
  '{measured:$measured, exact:$exact, verify_rc:$verify_rc, verify_verdict:$verify_verdict, verdict:$verdict, legs_executed:$legs_executed, legs:$legs, refusal:$refusal}' | tee "$result_path"
