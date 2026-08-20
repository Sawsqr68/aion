#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)
sidecar="$root/workflows/repo-battery/proofs/evidence/repo_battery.beam.json"
expected='["subject_repo","ref_requested","head_commit","tree_path","gates_json_sha256","battery_sha256","script_witnesses","venue","environment","legs_declared","legs_executed","legs","verify_verdict","waivers","verdict"]'
payload=$(jq -c '.output_schema.oneOf[] | select(.properties.outcome.const == "reported") | .properties.payload' "$sidecar")
actual=$(jq -c '.required' <<<"$payload")
[[ "$actual" == "$expected" ]] || { printf 'required order mismatch\nexpected %s\nactual   %s\n' "$expected" "$actual" >&2; exit 1; }

jq -e '.properties.verdict.description | contains("status answers \"did I measure?\", the receipt answers \"what did I measure?\"")' <<<"$payload" >/dev/null
jq -e '.properties.ref_requested.description | contains("A BATTERY RUNS WITH THE VENUE")' <<<"$payload" >/dev/null
jq -e '.properties.venue.description | contains("AUTHORITY-BEARING ACT")' <<<"$payload" >/dev/null
jq -e '.properties.legs_declared.description | contains("Zero legs is a typed refusal naming the selector path")' <<<"$payload" >/dev/null
undeclared=$(jq -c '.output_schema.oneOf[] | select(.properties.outcome.const == "undeclared") | .properties.payload' "$sidecar")
jq -e '.properties.verdict.description | contains("not-a-verdict") and contains("pass-by-omission")' <<<"$undeclared" >/dev/null
printf 'schema assertions passed: ordered receipt fields and G2/R1/R2/R3 descriptions\n'
