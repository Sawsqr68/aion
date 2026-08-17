#!/usr/bin/env bash
# aion#35 retire-libsql — canonical battery, verbatim from
# /Users/tom/Developer/ablative/docs/GATE-EXTRACTION-STANDARD.md.
#
# Green = EMPTY findings.jsonl + 0 failed on the nextest Summary line.
# Per-leg stderr and the Summary line are kept beside the findings so an empty
# extraction is provably "everything passed" rather than "the run died".
#
# Caveat 2 of the standard applies to this lane: it REMOVES a cargo feature, so
# the non-default feature sets are compiled by their own named clippy legs
# below, outside the three canonical legs.
set -u

OUT="$(cd "$(dirname "$0")" && pwd)"
cd "$OUT/../.." || exit 1

: >"$OUT/findings.jsonl"

{
  cargo check \
  --workspace \
  --all-targets \
  --message-format=json \
  --keep-going \
  2>"$OUT/check.stderr" |
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
    2>"$OUT/clippy.stderr" |
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
      2>"$OUT/nextest.stderr" |
    jq -c '
      select(.type == "test" and .event == "failed")
      | {
          type: "nextest",
          test: .name,
          stdout: (.stdout // ""),
          message: "test failed"
        }
    ' || true
} >>"$OUT/findings.jsonl"

# Standard caveat 2 — the feature sets the three legs never compile. This lane
# deletes the `haematite-backend` feature, so both ends of the remaining
# feature space are compiled explicitly.
#
# These land in their OWN findings file, not the canonical one. Both non-default
# sets are already red at the base commit d4317268f (measured, control set in
# feature-leg-control-base-d4317268f.txt), so folding them into findings.jsonl
# would break the canonical "green = empty" rule for defects this lane did not
# author. The verdict for these legs is the control DIFF, not their emptiness:
# no finding may appear here that is absent from the control.
: >"$OUT/feature-legs-findings.jsonl"
for spec in \
  "aion-server:--no-default-features" \
  "aion-server:--all-features" \
  "aion-cli:--no-default-features" \
  "aion-cli:--all-features"
do
  crate="${spec%%:*}"
  flags="${spec##*:}"
  label="${crate}${flags//--/-}"
  cargo clippy -p "$crate" --all-targets "$flags" \
    --message-format=json --keep-going -- -D warnings \
    2>"$OUT/clippy-${label}.stderr" |
    jq -c '
      select(.reason == "compiler-message")
      | select(.message.level == "error" or .message.level == "warning")
      | {
          type: "clippy-feature-leg",
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
          lint: (.message.code.code // "compile-error"),
          message: .message.message
        }
    ' >>"$OUT/feature-legs-findings.jsonl"
done

grep -E "^ *Summary" "$OUT/nextest.stderr" >"$OUT/nextest-summary.txt" 2>/dev/null
echo done >"$OUT/battery.done"
