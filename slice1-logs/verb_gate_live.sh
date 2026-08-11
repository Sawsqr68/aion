#!/bin/sh
# NEW-1 proved through the REAL binary: the same four flag spellings, two verbs,
# opposite truths — and the sweep claims DEFERRED.md makes about --help/--version.
set -u
AION=./target/debug/aion

echo "# binary:  $AION  ($($AION --version 2>&1))"
echo "# tree:    $(git rev-parse HEAD)"
echo "# date:    $(date '+%Y-%m-%dT%H:%M:%S%z')"
echo

echo "=== worker awl: the line docs/workers/AWL-WORKER-DOCUMENTS.md publishes, plus a typo"
echo "\$ $AION worker awl greeter.awl --concurrency 4 --reconnect-initial-backoff 0.1 --reconnect-max-backoff 5 --reconnect-max-attempts 1000 --identty greeter-worker"
$AION worker awl greeter.awl --concurrency 4 \
    --reconnect-initial-backoff 0.1 --reconnect-max-backoff 5 \
    --reconnect-max-attempts 1000 --identty greeter-worker 2>&1
echo "[exit=$?]"
echo "# ^ MUST NOT mention DELETED or harness: those flags are live and REQUIRED here."
echo

echo "=== worker agent: the same four spellings, on the verb where they really were deleted"
echo "\$ $AION worker agent flows/review.awl --concurrency 4 --reconnect-initial-backoff 1"
$AION worker agent flows/review.awl --concurrency 4 --reconnect-initial-backoff 1 2>&1
echo "[exit=$?]"
echo

echo "=== the DEFERRED.md sweep: --help and --version still exit 0 through parse_or_refuse"
$AION --help > /dev/null 2>&1
echo "--help    [exit=$?]"
$AION --version > /dev/null 2>&1
echo "--version [exit=$?]"
$AION worker agent --help > /dev/null 2>&1
echo "worker agent --help [exit=$?]"
