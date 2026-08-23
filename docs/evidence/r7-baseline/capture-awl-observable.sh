#!/bin/bash
# R7 capture — the pre-existing-AWL-document observable, before and after.
#
# Usage: capture-awl-observable.sh <out-dir> <aion-binary> <path-list-file>
#
# WHAT IS CAPTURED, and why this shape:
#   * `aion awl emit` FULL observable = (exit_code, stdout, stderr). Not
#     stdout alone: 234 of the 424 pre-existing documents are deliberately
#     invalid checker fixtures whose REFUSAL must also be unchanged, and a
#     refusal lives in (exit, stderr).
#   * The M1 fmt fixpoint. `aion awl fmt --check` DOES NOT EXIST on this
#     tree (crates/aion-cli/src/awl.rs, `AwlCommand::Fmt` — one `file`
#     field, and the doc says "the printer is the single formatter, one
#     rendering, so there is no `--check` mode"). The design's INTENT is a
#     fixpoint proof, so it is discharged with the CLI that exists: format a
#     COPY in a scratch dir and diff against the original. THE TREE IS NEVER
#     FORMATTED IN PLACE.
#   * fmt_output_sha256 — the sha256 of the formatted copy. 72 of the 424
#     are NOT fmt-canonical today (workflows/fleet-dev/fleet_dev.awl among
#     them), so "fixpoint" is not the invariant; PRESERVATION of each
#     document's exact fmt image is. Hashing the formatted bytes makes
#     "exactly as non-canonical after" a byte-level claim, not a boolean.
#
# BOUNDARY-LEVEL INPUTS ONLY: every input here is a document already tracked
# in the repository (largest is 434 lines). No generated corpus, no
# multi-GB fixtures — the whole captured manifest is a few tens of KB.
set -u
out="$1"; aion="$2"; paths="$3"
mkdir -p "$out"
scratch=$(mktemp -d) || exit 1
trap 'rm -rf "$scratch"' EXIT
manifest="$out/manifest.tsv"
printf 'path\temit_exit\temit_stdout_sha256\temit_stderr_sha256\tfmt_exit\tfmt_fixpoint\tfmt_output_sha256\n' > "$manifest"
n=0
while IFS= read -r doc; do
  [ -n "$doc" ] || continue
  n=$((n+1))
  so=$("$aion" awl emit "$doc" 2>"$scratch/e"); rc=$?
  osha=$(printf '%s' "$so" | sha256sum | cut -d' ' -f1)
  esha=$(sha256sum < "$scratch/e" | cut -d' ' -f1)
  cp "$doc" "$scratch/copy.awl"
  "$aion" awl fmt "$scratch/copy.awl" >/dev/null 2>&1; frc=$?
  if diff -q "$doc" "$scratch/copy.awl" >/dev/null 2>&1; then fx=fixpoint; else fx=NOT-FIXPOINT; fi
  fsha=$(sha256sum < "$scratch/copy.awl" | cut -d' ' -f1)
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$doc" "$rc" "$osha" "$esha" "$frc" "$fx" "$fsha" >> "$manifest"
done < "$paths"
echo "captured $n documents into $manifest"
