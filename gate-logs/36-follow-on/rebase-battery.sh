#!/usr/bin/env bash
# Post-rebase battery of record for 36-follow-on (battery re-runs on a new tree).
# Same three legs as docs/GATE-EXTRACTION-STANDARD.md, with per-leg stderr KEPT
# so the run's completion is provable (a Summary line, not just an empty
# extraction shielded by `|| true`).
set -uo pipefail
cd /Users/tom/Developer/ablative/stack/aion/.worktrees/36-follow-on || exit 9
G=gate-logs/36-follow-on
echo $$ > "$G/rebase-battery.pid"
: > "$G/rebase-findings.jsonl"
RUSTC_JQ='select(.reason == "compiler-message") | select(.message.level == "error" or .message.level == "warning") | {type:"clippy", level:.message.level, file:([.message.spans[]? | select(.is_primary)][0].file_name // .message.spans[0]?.file_name // "unknown"), line:([.message.spans[]? | select(.is_primary)][0].line_start // .message.spans[0]?.line_start // null), lint:(.message.code.code // "compile-error"), message:.message.message}'
cargo check --workspace --all-targets --message-format=json --keep-going 2> "$G/rebase-check.stderr" | jq -c "$RUSTC_JQ" >> "$G/rebase-findings.jsonl"
echo "check rc=${PIPESTATUS[0]}" >> "$G/rebase-battery.log"
cargo clippy --workspace --all-targets --message-format=json --keep-going -- -D warnings 2> "$G/rebase-clippy.stderr" | jq -c "$RUSTC_JQ" >> "$G/rebase-findings.jsonl"
echo "clippy rc=${PIPESTATUS[0]}" >> "$G/rebase-battery.log"
NORN_BIN=/Users/tom/.cargo/bin/norn NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 cargo nextest run --workspace --all-targets --cargo-quiet --message-format libtest-json-plus --no-fail-fast 2> "$G/rebase-nextest.stderr" | jq -c 'select(.type == "test" and .event == "failed") | {type:"nextest", test:.name, stdout:(.stdout // ""), message:"test failed"}' >> "$G/rebase-findings.jsonl"
echo "nextest rc=${PIPESTATUS[0]}" >> "$G/rebase-battery.log"
echo done > "$G/rebase-battery.exit"
