#!/bin/sh
# Canonical three-leg battery, verbatim from docs/GATE-EXTRACTION-STANDARD.md,
# with the amendment's REQUIRED completion artifacts: per-leg stderr + the
# nextest Summary line, so empty-because-green is distinguishable from
# empty-because-it-died.
cd /Users/tom/Developer/ablative/stack/aion-wt-r3-budget || exit 1
G=gate-logs/r3-cache-budget
{
  cargo check --workspace --all-targets --message-format=json --keep-going \
  2>"$G/check.stderr" |
  jq -c '
    select(.reason == "compiler-message")
    | select(.message.level == "error" or .message.level == "warning")
    | { type: "clippy", level: .message.level,
        file: ([.message.spans[]? | select(.is_primary)][0].file_name // .message.spans[0]?.file_name // "unknown"),
        line: ([.message.spans[]? | select(.is_primary)][0].line_start // .message.spans[0]?.line_start // null),
        column: ([.message.spans[]? | select(.is_primary)][0].column_start // .message.spans[0]?.column_start // null),
        lint: (.message.code.code // "compile-error"), message: .message.message }
  ' || true

  cargo clippy --workspace --all-targets --message-format=json --keep-going -- -D warnings \
    2>"$G/clippy.stderr" |
    jq -c '
      select(.reason == "compiler-message")
      | select(.message.level == "error" or .message.level == "warning")
      | { type: "clippy", level: .message.level,
          file: ([.message.spans[]? | select(.is_primary)][0].file_name // .message.spans[0]?.file_name // "unknown"),
          line: ([.message.spans[]? | select(.is_primary)][0].line_start // .message.spans[0]?.line_start // null),
          column: ([.message.spans[]? | select(.is_primary)][0].column_start // .message.spans[0]?.column_start // null),
          lint: (.message.code.code // "compile-error"), message: .message.message }
    '

  NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 \
    cargo nextest run --workspace --all-targets --cargo-quiet \
      --message-format libtest-json-plus --no-fail-fast \
      2>"$G/nextest.stderr" |
    jq -c '
      select(.type == "test" and .event == "failed")
      | { type: "nextest", test: .name, stdout: (.stdout // ""), message: "test failed" }
    ' || true
} > "$G/findings.jsonl"
grep -E "^ *Summary" "$G/nextest.stderr" > "$G/nextest-summary.txt"
echo "BATTERY COMPLETE" > "$G/battery-done.txt"
