#!/bin/sh
# Acceptance, driven through the REAL binary rather than through a test harness:
# the three migration riders and the kind-scoped checker refusals.
set -u
AION=./target/debug/aion

echo "=== rider (b): a bare invocation refuses LOUDLY, naming the new shape ==="
$AION worker agent
echo "exit=$?"
echo

echo "=== migration: a deleted launch flag no longer parses at all ==="
$AION worker agent flows/x.awl --concurrency 4
echo "exit=$?"
echo

echo "=== rider (a): the VERB survives and takes a document reference ==="
$AION worker agent \
  crates/aion-awl/tests/fixtures/rev2/harness-section/valid/norn_harness.awl --check
echo "exit=$?"
echo

echo "=== the checker names the missing field, scoped to the declared kind ==="
$AION awl check \
  crates/aion-awl/tests/fixtures/rev2/harness-section/invalid/missing_acp_permission.awl
echo
$AION awl check \
  crates/aion-awl/tests/fixtures/rev2/harness-section/invalid/missing_env_pass_on_norn.awl
echo
$AION awl check \
  crates/aion-awl/tests/fixtures/rev2/harness-section/invalid/norn_section_carrying_an_acp_command.awl
echo

echo "=== a complete acp section is COMPLETE without any norn setting ==="
$AION awl check crates/aion-awl/tests/fixtures/rev2/harness-section/valid/acp_harness.awl
echo "=== a complete norn section is COMPLETE without any acp setting ==="
$AION awl check crates/aion-awl/tests/fixtures/rev2/harness-section/valid/norn_harness.awl
