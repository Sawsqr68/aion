#!/bin/sh
# read-report.sh <workspace_root> <workflow_id>
# Parse <tree>/.investigation/report.json into the ReportProbe shape. The
# `report` key is OMITTED (never nulled) when the tree carries nothing
# parseable — "nothing went wrong" and "nobody wrote it" must not arrive as
# the same value. EXIT ZERO ALWAYS on anticipated faces.
set -u

refuse() {
    jq -n --arg e "$1" '{parse_error: $e}'
    exit 0
}

[ $# -eq 2 ] || refuse "read-report.sh expects exactly 2 arguments, got $#"
workspace_root=$1
workflow_id=$2
report="$workspace_root/$workflow_id/repo/.investigation/report.json"

[ -f "$report" ] || refuse "report absent at $report"

parse_err=$(jq -e . "$report" 2>&1 >/dev/null)
if [ $? -ne 0 ]; then
    refuse "report at $report does not parse: $parse_err"
fi

jq -n --slurpfile r "$report" '{parse_error: "", report: $r[0]}'
