#!/bin/sh
# file-report.sh <workspace_root> <workflow_id> <repo> <number> <session_id> <control_reply>
# Render the typed report mechanically and comment it onto the issue. The
# agent's prose reaches the tracker only through the report schema; the
# comment leads with the read-by-hand law. EXIT ZERO ALWAYS on anticipated
# faces — a red filing is data and the report survives in the workflow
# payload, never lost to a tracker hiccup.
set -u

refuse() {
    jq -n --arg e "$1" --argjson c "${2:-1}" '{ok: false, exit_code: $c, stderr: $e, url: ""}'
    exit 0
}

[ $# -eq 6 ] || refuse "file-report.sh expects exactly 6 arguments, got $#"
workspace_root=$1
workflow_id=$2
repo=$3
number=$4
session_id=$5
control_reply=$6
report="$workspace_root/$workflow_id/repo/.investigation/report.json"

[ -f "$report" ] || refuse "report absent at $report at filing time"

body=$(mktemp) || refuse "mktemp failed"
trap 'rm -f "$body"' EXIT

{
    printf '**Automated investigation. Every citation must be read by hand before anything acts on this.**\n\n'
    jq -r '
        "**Verdict: " + .verdict + "** (investigator confidence: " + .confidence + ")\n\n" +
        "Measured tree: `" + .head_commit + "` of " + .repo + "\n\n" +
        .summary + "\n\n" +
        "### Evidence\n" +
        ( [ .evidence[] | "- `" + .path + ":" + (.line | tostring) + "` — " + .excerpt ] | join("\n") ) +
        ( if .repro then "\n\n### Reproduction\n" + .repro else "" end ) +
        ( if .proposed_fix then "\n\n### Proposed fix (described, never applied)\n" + .proposed_fix else "" end )
    ' "$report"
    printf '\n\n---\n*schema %s · workflow `%s` · session `%s` · control proof: `%s`*\n' \
        "$(jq -r '.schema_version' "$report")" "$workflow_id" "$session_id" "$control_reply"
} > "$body" || refuse "rendering failed for $report"

out=$(gh issue comment "$number" --repo "$repo" --body-file "$body" 2>&1)
rc=$?
if [ $rc -ne 0 ]; then
    refuse "gh issue comment failed: $out" "$rc"
fi

jq -n --arg u "$out" '{ok: true, exit_code: 0, stderr: "", url: $u}'
