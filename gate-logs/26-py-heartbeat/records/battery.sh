#!/bin/zsh
# Canonical extraction battery per ablative/docs/GATE-EXTRACTION-STANDARD.md.
# Three legs, all JSON message-format, all keep-going/no-fail-fast, each finding
# reduced by jq to one JSON object per line. No fmt gate (builders autoformat).
#
# Caveat 1 from the standard is honoured: per-leg stderr and the nextest Summary
# line are kept beside the findings file, so "empty because everything passed" is
# provably different from "empty because it died mid-run".
#
# This lane's diff is Python-only. The battery is the landing PROOF of that:
# an unchanged Rust tree must produce an unchanged (empty) extraction, and an
# empty extraction from a run that demonstrably completed is what says so.
set -u
cd /Users/tom/Developer/ablative/stack/aion-wt-26-py-heartbeat || exit 1
export NORN_BIN=/Users/tom/.cargo/bin/norn
OUT=gate-logs-local

{
  cargo check \
  --workspace \
  --all-targets \
  --message-format=json \
  --keep-going \
  2>"$OUT/leg1-check.stderr" |
  jq -c '
    select(.reason == "compiler-message")
    | select(.message.level == "error" or .message.level == "warning")
    | {
        type: "clippy",
        level: .message.level,
        file: (
          [.message.spans[]? | select(.is_primary)][0].file_name
          // .message.spans[0]?.file_name
          // "unknown"
        ),
        line: (
          [.message.spans[]? | select(.is_primary)][0].line_start
          // .message.spans[0]?.line_start
          // null
        ),
        column: (
          [.message.spans[]? | select(.is_primary)][0].column_start
          // .message.spans[0]?.column_start
          // null
        ),
        lint: (.message.code.code // "compile-error"),
        message: .message.message
      }
  ' || true

  cargo clippy \
    --workspace \
    --all-targets \
    --message-format=json \
    --keep-going \
    -- \
    -D warnings \
    2>"$OUT/leg2-clippy.stderr" |
    jq -c '
      select(.reason == "compiler-message")
      | select(.message.level == "error" or .message.level == "warning")
      | {
          type: "clippy",
          level: .message.level,
          file: (
            [.message.spans[]? | select(.is_primary)][0].file_name
            // .message.spans[0]?.file_name
            // "unknown"
          ),
          line: (
            [.message.spans[]? | select(.is_primary)][0].line_start
            // .message.spans[0]?.line_start
            // null
          ),
          column: (
            [.message.spans[]? | select(.is_primary)][0].column_start
            // .message.spans[0]?.column_start
            // null
          ),
          lint: (.message.code.code // "compile-error"),
          message: .message.message
        }
    '

  NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 \
    cargo nextest run \
      --workspace \
      --all-targets \
      --cargo-quiet \
      --message-format libtest-json-plus \
      --no-fail-fast \
      2>"$OUT/leg3-nextest.stderr" |
    jq -c '
      select(.type == "test" and .event == "failed")
      | {
          type: "nextest",
          test: .name,
          stdout: (.stdout // ""),
          message: "test failed"
        }
    ' || true
} > "$OUT/findings.jsonl" 2>"$OUT/battery-harness.stderr"

echo "BATTERY DONE $(date '+%Y-%m-%dT%H:%M:%S%z')" >> "$OUT/battery-harness.stderr"
date '+%Y-%m-%dT%H:%M:%S%z' > "$OUT/battery.done"
