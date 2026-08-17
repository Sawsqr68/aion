#!/bin/sh
# Completion proof for the 42-fork-ordering battery. Same three legs, same jq,
# as GATE-EXTRACTION-STANDARD.md, but each leg's stderr is KEPT to a file
# instead of /dev/null, so the run's COMPLETION is provable (a nextest Summary
# line, per-leg rc) rather than only its emptiness. The VERDICT of record is the
# verbatim run (battery-final.jsonl); this pass exists to prove that run's legs
# actually executed, and its findings are cross-checked against it.
cd /Users/tom/Developer/ablative/stack/aion/.worktrees/42-fork-ordering || exit 9
G=gate-logs/42-fork-ordering
: > "$G/proof-findings.jsonl"
RUSTC_JQ='select(.reason == "compiler-message") | select(.message.level == "error" or .message.level == "warning") | {type:"clippy", level:.message.level, file:([.message.spans[]? | select(.is_primary)][0].file_name // .message.spans[0]?.file_name // "unknown"), line:([.message.spans[]? | select(.is_primary)][0].line_start // .message.spans[0]?.line_start // null), column:([.message.spans[]? | select(.is_primary)][0].column_start // .message.spans[0]?.column_start // null), lint:(.message.code.code // "compile-error"), message:.message.message}'
cargo check --workspace --all-targets --message-format=json --keep-going 2> "$G/proof-check.stderr" | jq -c "$RUSTC_JQ" >> "$G/proof-findings.jsonl"
echo "check rc=$?" >> "$G/proof-legs.log"
cargo clippy --workspace --all-targets --message-format=json --keep-going -- -D warnings 2> "$G/proof-clippy.stderr" | jq -c "$RUSTC_JQ" >> "$G/proof-findings.jsonl"
echo "clippy rc=$?" >> "$G/proof-legs.log"
NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 cargo nextest run --workspace --all-targets --cargo-quiet --message-format libtest-json-plus --no-fail-fast 2> "$G/proof-nextest.stderr" | jq -c 'select(.type == "test" and .event == "failed") | {type:"nextest", test:.name, stdout:(.stdout // ""), message:"test failed"}' >> "$G/proof-findings.jsonl"
echo "nextest rc=$?" >> "$G/proof-legs.log"
