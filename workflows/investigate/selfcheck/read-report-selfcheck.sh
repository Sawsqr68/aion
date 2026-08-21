#!/bin/sh
set -u

case "$0" in
    */*) here=${0%/*} ;;
    *) here=. ;;
esac

reader=$here/../bin/read-report.sh
fixtures=$here/fixtures
tmp=${TMPDIR:-/tmp}/read-report-selfcheck.$$
failures=0

rm -rf "$tmp"
mkdir -p "$tmp" || exit 1
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

run_case() {
    case_name=$1
    fixture=$2
    error_predicate=$3
    report_predicate=$4

    if [ "$fixture" != absent ]; then
        mkdir -p "$tmp/$case_name/repo/.investigation" || exit 1
        cp "$fixtures/$fixture" "$tmp/$case_name/repo/.investigation/report.json" || exit 1
    fi

    out=$(sh "$reader" "$tmp" "$case_name" 2>&1)
    status=$?
    case_failed=0

    if [ "$status" -ne 0 ]; then
        printf 'FAIL %s: exit status: expected 0, found %s\n' "$case_name" "$status"
        case_failed=1
    fi

    if ! printf '%s\n' "$out" | jq -e "$error_predicate" >/dev/null 2>&1; then
        printf 'FAIL %s: parse_error predicate failed: %s\n' "$case_name" "$error_predicate"
        case_failed=1
    fi

    if ! printf '%s\n' "$out" | jq -e "$report_predicate" >/dev/null 2>&1; then
        printf 'FAIL %s: report-presence predicate failed: %s\n' "$case_name" "$report_predicate"
        case_failed=1
    fi

    if [ "$case_failed" -eq 0 ]; then
        printf 'PASS %s\n' "$case_name"
    else
        diagnostic=$(printf '%s\n' "$out" | jq -c '{parse_error, has_report: has("report")}' 2>/dev/null)
        if [ $? -ne 0 ]; then
            diagnostic=$out
        fi
        printf '  output: %s\n' "$diagnostic"
        failures=$((failures + 1))
    fi
}

run_case conforming conforming.json \
    '.parse_error == ""' \
    'has("report")'

run_case actual-71061d6c run-71061d6c.json \
    '.parse_error != "" and (.parse_error | contains("`repro`") and contains("`proposed_fix`") and contains("expected String or ABSENT") and contains("found Object"))' \
    'has("report") | not'

run_case missing-required missing-required.json \
    '.parse_error | contains("`summary`") and contains("expected String") and contains("found ABSENT")' \
    'has("report") | not'

run_case bad-verdict bad-verdict.json \
    '.parse_error | contains("`verdict`") and contains("Reproduced | NotReproduced | AlreadyFixed | NeedsInfo | WrongRepo") and contains("found String \"Probably\"")' \
    'has("report") | not'

run_case absent absent \
    '.parse_error | startswith("report absent at ")' \
    'has("report") | not'

run_case invalid-json invalid-json.txt \
    '.parse_error | contains(" does not parse: ")' \
    'has("report") | not'

run_case optional-absent optional-absent.json \
    '.parse_error == ""' \
    'has("report")'

run_case optional-null optional-null.json \
    '.parse_error | contains("`repro`") and contains("`proposed_fix`") and contains("found Null") and contains("an absent optional is conforming, an explicit null is not")' \
    'has("report") | not'

run_case bad-evidence bad-evidence.json \
    '.parse_error | contains("`evidence[0].line`") and contains("expected Int") and contains("found Number 1.0") and contains("`evidence[1]`") and contains("expected Object") and contains("found String")' \
    'has("report") | not'

run_case root-array root-array.json \
    '.parse_error | contains("the report is an Array, not an Object")' \
    'has("report") | not'

run_case extra-key extra-key.json \
    '.parse_error == ""' \
    'has("report")'

run_case several-faults several-faults.json \
    '.parse_error | contains("`schema_version`") and contains("`issue`") and contains("`repo`") and contains("`verdict`") and contains("`summary`") and contains("`evidence[0].path`") and contains("`evidence[0].line`") and contains("`evidence[0].excerpt`") and contains("`repro`") and contains("`confidence`")' \
    'has("report") | not'

run_case multiple-documents multiple-documents.json \
    '.parse_error | contains("carries 2 JSON documents; a report is exactly one")' \
    'has("report") | not'

if [ "$failures" -ne 0 ]; then
    printf 'SELFCHECK-RED: %s case(s) failed\n' "$failures"
    exit 1
fi

printf 'SELFCHECK-GREEN\n'
