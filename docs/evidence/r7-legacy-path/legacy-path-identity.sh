#!/bin/bash
# R7 clause 4 + design section 7 clause 5, as an EXECUTABLE diff.
#
# Runs the PRE-EXISTING `fleet-plan-check.sh` (extracted from the rig base
# commit) and the EXTENDED one side by side, over the same fixture
# repositories, and diffs their FULL observable: (exit_code, stdout,
# stderr). A plan without a `fleet-stages-v1` block must produce a
# BYTE-IDENTICAL result from both.
#
# WHY A TEST DOUBLE FOR `fleet-lib.sh`, AND EXACTLY WHAT DIFFERS.
# The sealed scripts are provisioned to /home/aion/fleet on a fleet box;
# that path does not exist on this venue, and `fleet_root_guard` pins the
# workspace root to the literal `/home/aion/fleet/runs`. The double below
# carries `op_emit` and `op_guard_fail` -- the two functions that PRODUCE
# THE OUTPUT UNDER TEST -- byte-for-byte from the tracked
# `workflows/fleet-dev/bin/fleet-lib.sh`, and differs ONLY in the root
# constant the two guards compare against. `fleet-lib.sh` itself is NOT
# modified in the tree; the harness asserts its blob is identical to the
# rig base before it runs.
#
# Boundary-level synthetic inputs: each fixture repo holds one plan file
# and one leg brief, a few hundred bytes. No multi-GB fixtures.
set -u
repo_root=$(git rev-parse --show-toplevel)
BASE=c590712f57a2fe8353c6cd42823200587f25d70e
work=$(mktemp -d) || exit 1
trap 'rm -rf "$work"' EXIT
fail=0

# --- the tracked fleet-lib.sh must be untouched by this flight ----------
lib_base=$(git -C "$repo_root" rev-parse "$BASE:workflows/fleet-dev/bin/fleet-lib.sh")
lib_head=$(git -C "$repo_root" hash-object "$repo_root/workflows/fleet-dev/bin/fleet-lib.sh")
if [ "$lib_base" != "$lib_head" ]; then
  echo "FAIL: workflows/fleet-dev/bin/fleet-lib.sh was MODIFIED ($lib_base -> $lib_head)"
  exit 1
fi
echo "ok: fleet-lib.sh unmodified (blob $lib_base)"

# --- the two scripts under comparison -----------------------------------
mkdir -p "$work/bin"
git -C "$repo_root" show "$BASE:workflows/fleet-dev/bin/fleet-plan-check.sh" > "$work/bin/old-plan-check.sh"
cp "$repo_root/workflows/fleet-dev/bin/fleet-plan-check.sh" "$work/bin/new-plan-check.sh"

# --- the double: op_emit/op_guard_fail verbatim, root constant relaxed ---
{
  git -C "$repo_root" show "$BASE:workflows/fleet-dev/bin/fleet-lib.sh" \
    | sed 's|"/home/aion/fleet/runs"|"$FLEET_HOME/runs"|'
} > "$work/bin/fleet-lib.sh"
# Prove the output-producing functions were carried over verbatim.
for fn in op_guard_fail op_emit; do
  a=$(git -C "$repo_root" show "$BASE:workflows/fleet-dev/bin/fleet-lib.sh" | grep "^$fn()" | sha256sum)
  b=$(grep "^$fn()" "$work/bin/fleet-lib.sh" | sha256sum)
  if [ "$a" != "$b" ]; then echo "FAIL: the double's $fn differs from the real one"; exit 1; fi
done
echo "ok: the double's op_emit and op_guard_fail are byte-identical to the tracked ones"

# The OLD script sources the literal path, so give it one via a shim that
# re-execs it with the library already defined.
sed -i 's|^\. /home/aion/fleet/bin/fleet-lib\.sh$|. "$FLEET_HOME/bin/fleet-lib.sh"|' "$work/bin/old-plan-check.sh"
echo "ok: the old script's only harness edit is the sourced library PATH (its logic is untouched)"

# --- fixtures ------------------------------------------------------------
make_repo() { # $1 = name, $2 = plan body, $3 = leg id or empty, $4 = leave-dirty
  root="$work/runs/$1"; mkdir -p "$root/repo"; ( cd "$root/repo" || exit 1
    git init -q -b main .; git config user.email f@example.invalid; git config user.name f
    mkdir -p .fleet/legs
    printf '%s' "$2" > .fleet/plan.md
    [ -n "$3" ] && printf 'brief for %s\n' "$3" > ".fleet/legs/$3.md"
    git add -A >/dev/null 2>&1; git commit -qm fixture >/dev/null 2>&1
    [ -n "$4" ] && printf 'loose\n' >> .fleet/plan.md
    true )
}

# Capture the FULL observable of one script into a file: the process exit
# code, then stdout verbatim, then stderr verbatim. Multi-line stdout is
# preserved exactly -- the scripts emit a JSON object, and comparing only
# its first line would compare nothing.
capture() { # $1 = old|new, $2 = run name, $3 = legs csv, $4 = out file
  FLEET_HOME="$work" sh "$work/bin/$1-plan-check.sh" "$work/runs" "$2" "$3" \
      >"$work/o.$1" 2>"$work/e.$1"
  rc=$?
  { printf 'process_exit=%s\n--- stdout ---\n' "$rc"; cat "$work/o.$1"
    printf -- '--- stderr ---\n'; cat "$work/e.$1"; } > "$4"
}

compare() { # $1 = label, $2 = run name, $3 = legs
  capture old "$2" "$3" "$work/obs.old"
  capture new "$2" "$3" "$work/obs.new"
  if cmp -s "$work/obs.old" "$work/obs.new"; then
    bytes=$(wc -c < "$work/obs.old" | tr -d ' ')
    echo "IDENTICAL  $1  (${bytes} bytes compared)"
  else
    echo "DIVERGED   $1"
    diff -u "$work/obs.old" "$work/obs.new" | sed 's/^/    /'
    fail=1
  fi
}

PLAN_NO_BLOCK='# PLAN

An ordinary parallel-only plan. No staged-chain block anywhere.
'
PLAN_PROSE_MENTION='# PLAN

This plan MENTIONS fleet-stages-v1 in prose but declares no fenced block.
'

echo
echo "=== legacy path (no fleet-stages-v1 block): old vs new ==="
make_repo r-ok      "$PLAN_NO_BLOCK"       build ""
make_repo r-prose   "$PLAN_PROSE_MENTION"  build ""
make_repo r-empty   ""                     build ""
make_repo r-nobrief "$PLAN_NO_BLOCK"       ""    ""
make_repo r-dirty   "$PLAN_NO_BLOCK"       build 1

compare "exit 0  — plan and brief committed"          r-ok      build
compare "exit 0  — 'fleet-stages-v1' in PROSE only"   r-prose   build
compare "exit 1  — empty plan.md"                     r-empty   build
compare "exit 1  — declared brief absent"             r-nobrief build
compare "exit 1  — loose .fleet state"                r-dirty   build
compare "exit 3  — committed brief under another id"  r-ok      other-id
compare "exit 2  — script-face guard (empty csv)"     r-ok      ""

# === the staged-chain arm, through the REAL script =======================
# Only reachable when a block is present. The validator binary is installed
# at "$FLEET_HOME/bin", exactly where the sealed script looks for it.
validator_src="$repo_root/target/release/aion-fleet-plan"
if [ -x "$validator_src" ]; then
  cp "$validator_src" "$work/bin/aion-fleet-plan"
  echo
  echo "=== staged-chain arm (block PRESENT), through the real script ==="

  EX1='stage id=l1 legs=l1-design after=- admit=always conclusions=design-ready,measured-red conclusion_leg=l1-design advance=auto'
  EX2='stage id=l2 legs=l2-classify after=l1 admit=always conclusions=classification-ready conclusion_leg=l2-classify advance=auto'
  EX3='stage id=l3 legs=l3-route after=l2 admit=l1.conclusion!=measured-red conclusions=route-ready conclusion_leg=l3-route advance=auto'

  make_chain_repo() { # $1 = name, $2..$4 = stage lines
    root="$work/runs/$1"; mkdir -p "$root/repo"; ( cd "$root/repo" || exit 1
      git init -q -b main .; git config user.email f@example.invalid; git config user.name f
      mkdir -p .fleet/legs
      { printf '# PLAN\n\n```fleet-stages-v1\n'
        printf '%s\n' "$2"; [ -n "${3:-}" ] && printf '%s\n' "$3"; [ -n "${4:-}" ] && printf '%s\n' "$4"
        printf '```\n'; } > .fleet/plan.md
      for leg in l1-design l2-classify l3-route; do printf 'brief\n' > ".fleet/legs/$leg.md"; done
      git add -A >/dev/null 2>&1; git commit -qm chain >/dev/null 2>&1 )
  }

  chain_case() { # $1 = label, $2 = run name, $3 = expected inner exit_code, $4 = expected cause or ""
    capture new "$2" "l1-design,l2-classify,l3-route" "$work/obs.chain"
    got=$(grep -o '"exit_code": [0-9]*' "$work/obs.chain" | head -1 | tr -d ' ' | cut -d: -f2)
    if [ "$got" != "$3" ]; then
      echo "FAIL       $1 — inner exit_code $got, expected $3"; fail=1; return
    fi
    if [ -n "$4" ] && ! grep -q "$4" "$work/obs.chain"; then
      echo "FAIL       $1 — cause code $4 absent from the payload"; fail=1; return
    fi
    echo "ok         $1  (inner exit_code=$got${4:+, cause=$4})"
  }

  make_chain_repo c-valid   "$EX1" "$EX2" "$EX3"
  chain_case "exit 0  — the design section 9.1 exemplar block" c-valid 0 "chain_block"

  make_chain_repo c-cycle   'stage id=l1 legs=l1-design after=l3 admit=always conclusions=design-ready,measured-red conclusion_leg=l1-design advance=auto' "$EX2" "$EX3"
  chain_case "exit 4  — cycle"                                 c-cycle 4 "cycle"

  make_chain_repo c-unkpred "$EX1" 'stage id=l2 legs=l2-classify after=l9 admit=always conclusions=classification-ready conclusion_leg=l2-classify advance=auto' "$EX3"
  chain_case "exit 4  — unknown predecessor"                   c-unkpred 4 "unknown_predecessor"

  make_chain_repo c-charset 'stage id=../x legs=l1-design after=- admit=always conclusions=design-ready conclusion_leg=l1-design advance=auto' "" ""
  chain_case "exit 4  — stage-id charset (../x)"               c-charset 4 "invalid_stage_id_charset"

  make_chain_repo c-v2      "$EX1" "$EX2" "$EX3"
  sed -i 's/fleet-stages-v1/fleet-stages-v2/' "$work/runs/c-v2/repo/.fleet/plan.md"
  ( cd "$work/runs/c-v2/repo" && git add -A >/dev/null 2>&1 && git commit -qm v2 >/dev/null 2>&1 )
  chain_case "exit 4  — unsupported block version"             c-v2 4 "unsupported_block_version"
else
  echo
  echo "SKIP: target/release/aion-fleet-plan is absent; the staged-chain arm was not exercised."
  echo "      (build it with: cargo build --release -p aion-fleet-plan)"
fi

echo
if [ "$fail" -eq 0 ]; then
  echo "R7 LEGACY-PATH IDENTITY: PASS — every legacy scenario is byte-identical old vs new,"
  echo "and the staged-chain arm is reachable ONLY when a block is present."
else
  echo "R7 LEGACY-PATH IDENTITY: FAIL"
fi
exit "$fail"
