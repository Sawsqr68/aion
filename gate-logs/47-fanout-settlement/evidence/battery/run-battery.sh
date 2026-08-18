#!/bin/zsh
# Canonical three-leg extraction battery, ablative/docs/GATE-EXTRACTION-STANDARD.md.
# Verbatim jq (extract-compiler.jq / extract-nextest.jq, byte-identical to the
# aion#1 lane's filters). Per-leg stderr and the nextest Summary are kept beside
# the findings, per caveat 1 of the standard.
set -u
cd /Users/tom/Developer/ablative/stack/aion-wt-47-fanout
export NORN_BIN=/Users/tom/.cargo/bin/norn
OUT=gate-logs/47-fanout-settlement/evidence/battery

cargo check --workspace --all-targets --message-format=json --keep-going \
  2>"$OUT/leg1-check.stderr" \
  | jq -c -f "$OUT/extract-compiler.jq" > "$OUT/leg1-check-findings.jsonl"
echo "leg1_check_rc=${pipestatus[1]}" >> "$OUT/SUMMARY.txt"

cargo clippy --workspace --all-targets --message-format=json --keep-going -- -D warnings \
  2>"$OUT/leg2-clippy.stderr" \
  | jq -c -f "$OUT/extract-compiler.jq" > "$OUT/leg2-clippy-findings.jsonl"
echo "leg2_clippy_rc=${pipestatus[1]}" >> "$OUT/SUMMARY.txt"

NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 \
  cargo nextest run --workspace --all-targets --cargo-quiet \
    --message-format libtest-json-plus --no-fail-fast \
  2>"$OUT/leg3-nextest.stderr" \
  | jq -c -f "$OUT/extract-nextest.jq" > "$OUT/leg3-nextest-findings.jsonl"
echo "leg3_nextest_rc=${pipestatus[1]}" >> "$OUT/SUMMARY.txt"

{
  echo "findings_leg1_check=$(wc -l < "$OUT/leg1-check-findings.jsonl" | tr -d ' ')"
  echo "findings_leg2_clippy=$(wc -l < "$OUT/leg2-clippy-findings.jsonl" | tr -d ' ')"
  echo "findings_leg3_nextest=$(wc -l < "$OUT/leg3-nextest-findings.jsonl" | tr -d ' ')"
  echo "nextest_summary=$(grep -E '^\s+Summary' "$OUT/leg3-nextest.stderr" | tail -1)"
  echo "nextest_binaries=$(grep -cE '^\s+Starting' "$OUT/leg3-nextest.stderr")"
} >> "$OUT/SUMMARY.txt"
