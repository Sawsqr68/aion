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

# ── Vacuity: legs that PASSED while announcing they proved nothing ───────────
#
# A runtime-gated skip returns Ok(()) and libtest counts it a PASS, so a leg can
# be green while the proofs it is named for did not happen. The suites announce
# it on the raw stderr handle; until now nothing consumed the announcement.
#
# 🔴 THE MARKER IS DECLARED BY THE REPOSITORY, NOT ASSUMED HERE. This workflow
# runs against any repo, and a marker invented by the harness would silently
# match nothing in a repo that spells it differently — a scan that measures
# nothing while reporting clean. A repo that declares none is told so in the
# verdict rather than quietly skipped, because "not checked" and "checked and
# clean" must never look the same.
vacuity_marker=$(jq -r '.vacuity_marker // empty' "$tree/gates.json" 2>/dev/null || true)
vacuity='{"scanned":false,"total":0,"legs":[],"unattributed":[]}'
vacuity_refusal=""
if [[ -z "$vacuity_marker" ]]; then
  vacuity_refusal="gates.json declares no vacuity_marker, so announcements of the form a runtime-gated skip emits were NOT checked in this run. This is unmeasured, not clean."
else
  # The scanner is told which leg names are real, so a line printed BY a leg in
  # the shape of a frame cannot hijack attribution. This is not hypothetical:
  # the #113 landing log has 25 frame openers for 24 legs, the extra printed by
  # `estate-guard`'s own output.
  declared_legs_file="$run_dir/declared-legs.txt"
  jq -r '.legs[].name' "$tree/gates.json" >"$declared_legs_file"
  set +e
  vacuity_out=$(bash "$SCRIPT_DIR/scan-vacuity.sh" "$log_path" "$vacuity_marker" "$declared_legs_file" 2>&1)
  vacuity_rc=$?
  set -e
  if [[ "$vacuity_rc" -eq 0 ]]; then
    vacuity=$vacuity_out
  else
    vacuity_refusal="vacuity scanner refused (rc $vacuity_rc): $vacuity_out"
  fi
fi

# An announcement that belongs to no leg means the log framing changed or the
# log is truncated. Either way this verifier no longer knows what it measured,
# so it refuses instead of reporting the legs it did manage to attribute.
if [[ -z "$vacuity_refusal" ]] && [[ $(jq '.unattributed | length' <<<"$vacuity") -gt 0 ]]; then
  vacuity_refusal="vacuity scanner found $(jq '.unattributed | length' <<<"$vacuity") announcement(s) it could not attribute to a leg ($(jq '.frame_mismatches' <<<"$vacuity") of them from a closer naming a different leg than the open frame). A run that cannot say WHICH leg proved nothing has not measured the thing this verdict reports."
fi

# Fold the scan into the leg records. A leg that FAILED stays red: red is the
# stronger statement and a vacuous red is still a red.
if [[ -z "$vacuity_refusal" ]]; then
  # 🔴 AN INDEX, NOT A GENERATOR IN A BINDING. The first version of this used
  # `($vac.legs[] | select(.name == $leg.name) | .announcements) as $ann`. In jq
  # a binding over an EMPTY generator yields zero outputs, so `map` silently
  # DROPPED every leg that had no announcements — 22 of 24 legs vanished from the
  # record while the verdict still read correctly, because the verdict only
  # depends on the legs that remained. Caught by noticing the green count was 0
  # when it should have been 22; nothing in the verdict itself could show it.
  legs=$(jq -c --argjson vac "$vacuity" '
    ($vac.legs | map({key: .name, value: .announcements}) | from_entries) as $index
    | map(. as $leg
          | ($index[$leg.name] // []) as $ann
          | if ($ann | length) > 0
            then $leg + {announcements: $ann}
                 + (if $leg.status == "green" then {status: "unproven"} else {} end)
            else $leg end)' <<<"$legs")
fi

# ── Infrastructure failures: a red that proves nothing about the code ────────
#
# Some suites classify their own failure as an outage rather than a defect —
# `gleam build` dying in its dependency layer because the Hex registry was
# unreachable, for instance. The run 3 log of the rb-lockfix landing carries
# exactly one such line and the leg it belongs to went red on it.
#
# 🔴 THIS DOES NOT DOWNGRADE THE RED, and the earlier proposal that it should
# (gate-logs/rb-lockfix/RESULT.md) is withdrawn here on measurement. Turning a
# red into `unproven` on a text match is a hardening that fails OPEN: the phrase
# is emitted by code that believes the dependency layer died, and a genuinely
# broken build can believe that too. The reader's problem was never the
# severity — it was the misdiagnosis, being sent to hunt a code defect that was
# not there. So the classification is REPORTED next to the red, and the red
# stands.
infrastructure_marker=$(jq -r '.infrastructure_marker // empty' "$tree/gates.json" 2>/dev/null || true)
infrastructure='{"scanned":false,"total":0,"legs":[],"unattributed":[]}'
infra_refusal=""
if [[ -n "$infrastructure_marker" && -n "${declared_legs_file:-}" ]]; then
  set +e
  infra_out=$(bash "$SCRIPT_DIR/scan-vacuity.sh" "$log_path" "$infrastructure_marker" "$declared_legs_file" 2>&1)
  infra_rc=$?
  set -e
  if [[ "$infra_rc" -eq 0 ]]; then
    infrastructure=$infra_out
  else
    # 🔴 Its own variable, merged into `refusal` further down. Assigning
    # `refusal` here would be wiped by the `refusal=""` initialisation that
    # follows — a refusal that erases itself is worse than one never raised.
    infra_refusal="infrastructure scanner refused (rc $infra_rc): $infra_out"
  fi
fi

measured=true
exact=false
verdict=""
refusal=""
if [[ "$verify_rc" -eq 0 ]]; then
  exact=true
  # Precedence: a red outranks an unproven, and an unproven outranks a green.
  # Green is the only verdict that claims the run ESTABLISHED what it ran.
  if jq -e 'any(.[]; .status == "red")' <<<"$legs" >/dev/null; then
    verdict="red"
  elif jq -e 'any(.[]; .status == "unproven")' <<<"$legs" >/dev/null; then
    verdict="unproven"
  else
    verdict="green"
  fi
elif [[ "$verify_rc" -eq 1 ]]; then
  verdict="unproven"
else
  measured=false
  refusal="declaration verifier could not measure (rc $verify_rc): $verify_verdict"
fi

# A vacuity refusal never upgrades a verdict, and never hides behind one: it
# cannot make a red green, and it must not let a green read as established.
if [[ -n "$infra_refusal" ]]; then
  refusal="${refusal:+$refusal | }$infra_refusal"
fi
if [[ -n "$vacuity_refusal" ]]; then
  refusal="${refusal:+$refusal | }$vacuity_refusal"
  # 🔴 An `if`, not `[[ … ]] && …`. Under `set -e` a failing test at the end of
  # an AND-list carries its non-zero status to the statement and kills the
  # script — here that would mean a green run producing NO verdict file at all,
  # which reads as a harness crash rather than as the verdict it had computed.
  if [[ "$verdict" == "green" ]]; then
    verdict="unproven"
  fi
fi
jq -cn --argjson measured "$measured" --argjson exact "$exact" --argjson verify_rc "$verify_rc" \
  --arg verify_verdict "$verify_verdict" --arg verdict "$verdict" \
  --arg refusal "$refusal" --argjson legs_executed "$legs_executed" --argjson legs "$legs" \
  --argjson vacuity "$vacuity" --argjson infrastructure "$infrastructure" \
  '{measured:$measured, exact:$exact, verify_rc:$verify_rc, verify_verdict:$verify_verdict, verdict:$verdict, legs_executed:$legs_executed, legs:$legs, vacuity:$vacuity, infrastructure:$infrastructure, refusal:$refusal}' | tee "$result_path"
