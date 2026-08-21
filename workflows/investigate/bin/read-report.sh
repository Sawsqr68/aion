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

parse_err=$(jq empty "$report" 2>&1 >/dev/null)
if [ $? -ne 0 ]; then
    refuse "report at $report does not parse: $parse_err"
fi

document_count=$(jq -n --slurpfile r "$report" '$r | length' 2>&1)
if [ $? -ne 0 ]; then
    refuse "report at $report could not be counted as JSON documents: $document_count"
fi
if [ "$document_count" -ne 1 ]; then
    refuse "report at $report carries $document_count JSON documents; a report is exactly one"
fi

problems=$(jq -r -f /dev/stdin "$report" 2>&1 <<'JQ'
def kind:
  if . == null then "Null"
  elif type == "object" then "Object"
  elif type == "array" then "Array"
  elif type == "string" then "String"
  elif type == "number" then "Number"
  elif type == "boolean" then "Boolean"
  else type
  end;

def found:
  if type == "string" then "String \(tojson)"
  elif type == "number" then "Number \(tostring)"
  elif type == "boolean" then "Boolean \(tostring)"
  else kind
  end;

def required_string($field):
  if has($field) | not then
    ["field `\($field)`: expected String, found ABSENT"]
  elif .[$field] | type != "string" then
    ["field `\($field)`: expected String, found \(.[$field] | found)"]
  else
    []
  end;

def optional_string($field):
  if has($field) | not then
    []
  elif .[$field] == null then
    ["field `\($field)`: expected String or ABSENT, found Null — an absent optional is conforming, an explicit null is not"]
  elif .[$field] | type != "string" then
    ["field `\($field)`: expected String or ABSENT, found \(.[$field] | found)"]
  else
    []
  end;

def verdict:
  "Reproduced | NotReproduced | AlreadyFixed | NeedsInfo | WrongRepo" as $allowed |
  if has("verdict") | not then
    ["field `verdict`: expected one of \($allowed), found ABSENT"]
  elif .verdict | type != "string" then
    ["field `verdict`: expected one of \($allowed), found \(.verdict | found)"]
  elif .verdict as $value | (["Reproduced", "NotReproduced", "AlreadyFixed", "NeedsInfo", "WrongRepo"] | index($value)) == null then
    ["field `verdict`: expected one of \($allowed), found \(.verdict | found)"]
  else
    []
  end;

def evidence_string($index; $field):
  if has($field) | not then
    ["field `evidence[\($index)].\($field)`: expected String, found ABSENT"]
  elif .[$field] | type != "string" then
    ["field `evidence[\($index)].\($field)`: expected String, found \(.[$field] | found)"]
  else
    []
  end;

def evidence_line($index):
  if has("line") | not then
    ["field `evidence[\($index)].line`: expected Int, found ABSENT"]
  elif (.line | type == "number" and (tostring | test("^-?[0-9]+$"))) | not then
    ["field `evidence[\($index)].line`: expected Int, found \(.line | found)"]
  else
    []
  end;

def evidence:
  if has("evidence") | not then
    ["field `evidence`: expected Array of Evidence, found ABSENT"]
  elif .evidence | type != "array" then
    ["field `evidence`: expected Array of Evidence, found \(.evidence | found)"]
  else
    [
      .evidence | to_entries[] |
      .key as $index |
      .value |
      if type != "object" then
        ["field `evidence[\($index)]`: expected Object, found \(found)"]
      else
        evidence_string($index; "path")
        + evidence_line($index)
        + evidence_string($index; "excerpt")
      end
    ] | add // []
  end;

if type != "object" then
  ["the report is \(if kind == "Array" or kind == "Object" then "an" else "a" end) \(kind), not an Object"]
else
  required_string("schema_version")
  + required_string("issue")
  + required_string("repo")
  + required_string("head_commit")
  + verdict
  + required_string("summary")
  + evidence
  + optional_string("repro")
  + optional_string("proposed_fix")
  + required_string("confidence")
end
| join("; ")
JQ
)
if [ $? -ne 0 ]; then
    refuse "report at $report could not be validated against InvestigationReport: $problems"
fi

if [ -n "$problems" ]; then
    refuse "report at $report does not conform to InvestigationReport: $problems"
fi

jq -n --slurpfile r "$report" '{parse_error: "", report: $r[0]}'
