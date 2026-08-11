#!/bin/sh
# Acceptance, driven through the REAL binary rather than through a test harness.
#
# Every case records its ARGV and its EXIT CODE, because a log of output alone
# cannot be re-run and cannot tell a refusal from a crash: a binary that dies
# before printing produces the same empty stdout as one that printed nothing on
# purpose.
set -u
AION=./target/debug/aion
FIXTURES=crates/aion-awl/tests/fixtures/rev2/harness-section

echo "# binary:  $AION"
echo "# version: $($AION --version 2>&1)"
echo "# tree:    $(git rev-parse HEAD) $(git status --porcelain | wc -l | tr -d ' ') uncommitted path(s)"
echo "# date:    $(date '+%Y-%m-%dT%H:%M:%S%z')"
echo

# Runs one case: prints the argv, the output, and the exit code.
case_run() {
    label="$1"
    shift
    echo "=== $label"
    echo "\$ $AION $*"
    "$AION" "$@" 2>&1
    echo "[exit=$?]"
    echo
}

case_run "rider (b): a bare invocation refuses LOUDLY, naming the new shape" \
    worker agent

case_run "migration: a deleted launch flag no longer parses, and the refusal TEACHES" \
    worker agent flows/x.awl --concurrency 4

case_run "migration: several deleted flags at once are ALL named, not just the first" \
    worker agent flows/x.awl --harness norn --norn-arg=--account --acp-permission deny

case_run "rider (a): the VERB survives and takes a document reference" \
    worker agent "$FIXTURES/valid/norn_harness.awl" --check

# B1 needs a document naming a variable this machine does NOT set — the fixture
# corpus deliberately names only variables that exist, so it cannot show this.
PROBE="${TMPDIR:-/tmp}/aion-acceptance-absent-$$.awl"
sed 's/env_pass "PATH", "HOME"/env_pass "PATH", "AION_ACCEPTANCE_UNSET_VARIABLE"/' \
    "$FIXTURES/valid/norn_harness.awl" > "$PROBE"
case_run "B1: an absent declared pass-through is REPORTED before --check returns" \
    worker agent "$PROBE" --check
rm -f "$PROBE"

case_run "the checker names the missing field, scoped to the declared kind" \
    awl check "$FIXTURES/invalid/missing_acp_permission.awl"

case_run "...and on the other kind, so the rule is witnessed on both" \
    awl check "$FIXTURES/invalid/missing_env_pass_on_norn.awl"

case_run "a setting for the kind the section does NOT declare is refused by name" \
    awl check "$FIXTURES/invalid/norn_section_carrying_an_acp_command.awl"

case_run "M4: a relative norn binary is refused by the same absolute-path doctrine" \
    awl check "$FIXTURES/invalid/relative_norn_binary.awl"

case_run "a complete acp section is COMPLETE without any norn setting" \
    awl check "$FIXTURES/valid/acp_harness.awl"

case_run "a complete norn section is COMPLETE without any acp setting" \
    awl check "$FIXTURES/valid/norn_harness.awl"

case_run "m8: the absence control checks clean too — no section is a first-class state" \
    awl check "$FIXTURES/valid/harness_absent_control.awl"

echo "# end of acceptance"
