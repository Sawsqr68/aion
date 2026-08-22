#!/usr/bin/env bash
#
# The house battery: every gate this repository has, in one command.
#
# WHY THIS EXISTS
#
# GitHub Actions is disabled on this repository, so no workflow in
# `.github/workflows/` has ever run. That turned out to matter three separate
# times, each the same shape — a correctly designed gate that executed nowhere:
#
#   * the all-features Rust gate, which existed and had never run;
#   * `wire-types-no-diff`, whose comment shows its author saw the exact trap it
#     was guarding and designed around it — in a lane that is off;
#   * the Python and TypeScript SDK proto-drift gates, which are the REMEDIATION
#     for an incident where both worker SDKs went wire-dead with green suites.
#
# Before this script there was no justfile, no Makefile, and no runnable
# equivalent, so "the battery" was whatever each person remembered to type. The
# Rust battery is what people type, and `cargo test --workspace` does not run
# pytest, does not run bun, and does not typecheck the console. Every gate
# outside Rust was therefore load-bearing and unexecuted.
#
# HOW IT REPORTS
#
# Every gate runs even if an earlier one fails — a battery that stops at the
# first failure hides the other three. Each gate's exit code is printed in a
# manifest at the end, and the script exits non-zero if ANY gate failed.
#
# A MISSING TOOLCHAIN IS A FAILURE, NOT A SKIP. A skip is how a gate becomes
# invisible, which is the exact defect this script exists to fix. If `uv` or
# `bun` is absent the battery says so and goes red; install them or run the
# gates you can and say which ones in your report.
#
# WHERE THE LEG LIST LIVES — and why it is not here any more.
#
# It used to be here AND in `gates.json`, and the two had drifted in BOTH
# directions: this script ran 23 legs and never ran `estate-guard`; `gates.json`
# declared 7 and named none of the posture, console, Gleam, Python or TypeScript
# legs. Neither was a superset, so which gates you lost depended on which runner
# executed — and the answer to "is this repository green?" depended on who asked.
#
# 🔴 THE CURE IS SUBTRACTION, NOT A CHECKER. A drift-checker would be a third
# copy of the same list. `gates.json` is now the ONE list and this script
# executes exactly what is declared there, so the two sets cannot differ by
# construction rather than by discipline.
#
# 🔴 AND THE POPULATION IS NOW FIXED. Legs used to be wrapped in
# `if require_tool gleam; then ... fi`, so a box without gleam did not run three
# named legs and recorded ONE `toolchain:gleam` entry instead — two manifests
# could list different populations with nothing saying so. Every declared leg now
# appears in the manifest with its own exit code, so a missing toolchain is
# visible PER LEG (127) instead of shrinking the population.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT" || exit 1

DECLARATION="$REPO_ROOT/gates.json"

# Sourced with `|| exit $?` — a sourced refusal is a `return`, and unchecked it
# is advice. This script needs exactly one thing from the library, `select_python3`,
# and takes it from there rather than carrying its own copy: an interpreter
# chosen one way here and another way in the legs is the same defect this whole
# refactor removed, one layer down.
. "$REPO_ROOT/scripts/gates/lib.sh" || exit $?

# The estate root the `estate-guard` leg needs. Ruled 2026-08-20 (Waffles,
# implemented by Athena in tools/gates/lib/estate-root.sh): THE INVOKING RUNNER
# OWNS THE EXPORT — never a shell profile, never machine config, so nothing lands
# in anyone's ambient environment. Measured the same day: four repositories had
# landed the leg and NOT ONE runner in the estate exported the variable, so every
# one of those legs refused rc 3 on every run. Until this, aion was one of them.
#
# The four lines below only LOCATE that file; they carry no judgement. The walk
# that decides the root is re-run inside it, from $ESTATE_ROOT_FROM, against its
# own marker — so a bootstrap that drifts cannot return a WRONG root, it can only
# fail to find the file, which leaves the variable unset and the leg refusing
# rc 3 as a named declared omission. An off-estate venue takes that same road.
# That is why duplicating the locator across runners is tolerable.
#
# 🔴 `|| exit $?` IS NOT DECORATION. A refusal from a sourced file is a `return`,
# and a return the caller does not check is advice, not a refusal — without it
# this runner sails past rc 86 (a root that is named but is not a directory) and
# runs the whole battery against a root it cannot scan.
#
# ⚠️ AND rc-CHECKING A SOURCE IS NOT SUFFICIENT ON ITS OWN — a file that is
# present but TRUNCATED is valid shell and sources with rc 0, so status catches
# only the cuts that land mid-construct. The check that actually holds here is on
# the VALUE this runner consumes, and the leg already performs it:
# `[ -n "${ABLATIVE_ESTATE_ROOT:-}" ] || exit 3`. Every truncation of
# estate-root.sh either stops before the export (unset → the leg refuses, loud)
# or after it (root already resolved); none yields a set-but-wrong root.
# Do NOT "harden" this with a `command -v` symbol check copied from the estate's
# class-A guard: that guard consumes a FUNCTION and fails OPEN on its absence,
# this runner consumes a VALUE and fails CLOSED. Same hole, opposite direction —
# check the thing that is actually consumed.
ESTATE_ROOT_FROM="$REPO_ROOT"
_d="$REPO_ROOT"
while [ "$_d" != "/" ] && [ ! -f "$_d/tools/gates/lib/estate-root.sh" ]; do _d="$(dirname "$_d")"; done
if [ -f "$_d/tools/gates/lib/estate-root.sh" ]; then
  . "$_d/tools/gates/lib/estate-root.sh" || exit $?
fi
unset _d

# Gate name -> exit code, in declaration order. Bash 3.2 (macOS system bash) has
# no associative arrays, so this is two parallel arrays rather than one map.
GATE_NAMES=()
GATE_CODES=()

# Runs one gate, records its exit code, and never lets a failure stop the run.
#
# `set +e` around the invocation is deliberate: `set -e` is NOT set for this
# script precisely so a red gate cannot abort the battery, and this makes that
# explicit at the call site rather than relying on it.
gate() {
  local name="$1"
  shift
  printf '\n=== %s ===\n' "$name"
  # `</dev/null` is load-bearing: the leg loop feeds this function from a
  # heredoc, so an unredirected leg inherits the REMAINING LEG LIST as stdin.
  # Any leg whose tool reads stdin then eats the legs after it — observed live
  # on .205 (gleam:aion-flow's erl consumed legs 15-25 in two consecutive
  # rounds, 25 declared / 14 executed, no partial-run flag). Venue-dependent:
  # fires only where a leg's tool actually reads stdin, which is exactly why
  # it survived 322 full-set runs on another box.
  "$@" </dev/null
  local code=$?
  GATE_NAMES+=("$name")
  GATE_CODES+=("$code")
  # `printf` is given the leading dashes as an ARGUMENT, not inside the format
  # string: bash's printf reads a format beginning with `--` as an option
  # terminator and refuses the call.
  printf '%s %s exit %d %s\n' '---' "$name" "$code" '---'
  return 0
}

printf 'aion house battery — repo root %s\n' "$REPO_ROOT"
printf 'HEAD %s\n' "$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"

# --- Reading the declaration --------------------------------------------------
#
# 🔴 THIS RUNS IN EVERY MODE, INCLUDING THE SELF-TEST, and that is the point.
# It used to sit inside the `else` arm, so the self-test branched straight past
# it — which is how a reader that could not execute at all survived a green
# self-test. A harness check that skips the step most likely to break is
# checking the easy half.
#
# The declaration is read with python3 rather than jq: jq is not guaranteed on a
# fresh box, and a battery that cannot start because its own reader is missing
# is the failure mode this script exists to prevent.
#
# 🔴 AND THE READER IS CHOSEN BY RUNNING IT, not by asking the PATH. On the box
# this was written on, the FIRST python3 on the PATH is a Linux x86-64 ELF binary
# that cannot exec on an arm64 Mac — so the reader this script picked to avoid a
# missing dependency was itself unrunnable, and the battery could not start. The
# self-test never caught it, because self-test mode branches around the reader:
# the harness was proven and the thing that reads the list was not. See
# `select_python3` in scripts/gates/lib.sh.
#
# Names and commands are
# emitted base64-encoded because a leg's `cmd` legitimately contains quotes,
# newlines and shell metacharacters — round-tripping them through a delimiter
# would be a parser, and a parser is a place for this to go quietly wrong.
if [ ! -f "$DECLARATION" ]; then
  printf '🔴 NO DECLARATION at %s. This script runs what gates.json declares, so\n' "$DECLARATION"
  printf 'without it there are no legs to run — which is a failure, not a clean tree.\n'
  exit 1
fi

PYTHON3=$(select_python3) || {
  printf '🔴 the battery cannot read its declaration: no working python3 (see above).\n'
  exit 1
}

LEGS=$("$PYTHON3" - "$DECLARATION" <<'READER'
import base64, json, sys
doc = json.load(open(sys.argv[1]))
legs = doc.get("legs") or []
if not legs:
    sys.exit("declaration contains no legs")
for leg in legs:
    name, cmd = leg["name"], leg["cmd"]
    print(base64.b64encode(name.encode()).decode(), base64.b64encode(cmd.encode()).decode())
READER
) || { printf '🔴 could not read the declaration at %s\n' "$DECLARATION"; exit 1; }

printf 'declaration %s read with %s — %d legs declared\n' \
  "$DECLARATION" "$PYTHON3" "$(printf '%s\n' "$LEGS" | grep -c .)"

# The battery's own failure path, executable.
#
# A reporting harness whose red path has never run is the same defect as a gate
# whose assert half has never run — and this repository has now produced three
# of those. `AION_BATTERY_SELF_TEST=1 scripts/battery.sh` runs one passing and
# one failing synthetic gate and MUST exit 1 with `1 of 2 gates failed`. If it
# ever exits 0, this script cannot report a real failure either.
#
# The declared legs are read but not run in this mode, so the self-test now
# exercises the reader as well as the reporting — and a self-test log is exactly
# the log `scripts/gates/verify-declaration.sh` must REJECT, which is how that
# verifier's own red path is demonstrated without breaking anything.
if [ "${AION_BATTERY_SELF_TEST:-}" = "1" ]; then
  printf 'SELF-TEST MODE: no real gate runs. Expect exit 1 and 1 of 2 failed.\n'
  gate "self-test:passes" true
  gate "self-test:fails" false
else

# --- running a subset, and why it cannot be mistaken for a full battery -------
#
# `AION_BATTERY_ONLY="rust:fmt posture:worker-default" scripts/battery.sh` runs
# only the named legs. This exists because the alternative is worse: without it,
# checking one leg means either firing the whole battery — minutes of load, on a
# machine someone else is working on — or running the leg's command by hand,
# outside the runner, which proves nothing about the runner.
#
# 🔴 IT CANNOT LAUNDER A PARTIAL RUN INTO A GREEN ONE. A subset log's
# EXECUTED_LEGS is a strict subset of the declaration, so
# `scripts/gates/verify-declaration.sh` REJECTS it — with the skipped legs named.
# The escape hatch is only usable while it is visible, which is the whole reason
# the executed set is emitted at all.
#
# AN UNKNOWN NAME IS A FAILURE, NOT AN EMPTY RUN. A typo would otherwise select
# nothing, run nothing, and exit 0 through a manifest with no rows — the exact
# shape of a green that measured nothing.
ONLY="${AION_BATTERY_ONLY:-}"
if [ -n "$ONLY" ]; then
  printf '\n🔴 PARTIAL RUN — only these legs will run: %s\n' "$ONLY"
  printf 'This log is NOT a battery result and cannot pass verify-declaration.sh.\n'
fi
SELECTED=""

while read -r name_b64 cmd_b64; do
  [ -n "$name_b64" ] || continue
  name=$(printf '%s' "$name_b64" | base64 --decode)
  cmd=$(printf '%s' "$cmd_b64" | base64 --decode)
  if [ -n "$ONLY" ]; then
    case " $ONLY " in
      *" $name "*) SELECTED="$SELECTED $name" ;;
      *) continue ;;
    esac
  fi
  gate "$name" bash -c "$cmd"
done <<EOF
$LEGS
EOF

# A name in AION_BATTERY_ONLY that matched no declared leg is a typo, and a typo
# that quietly selects nothing is indistinguishable from a leg that passed.
if [ -n "$ONLY" ]; then
  for requested in $ONLY; do
    case " $SELECTED " in
      *" $requested "*) ;;
      *)
        printf '\n🔴 AION_BATTERY_ONLY names `%s`, which no leg in %s declares.\n' \
          "$requested" "$DECLARATION"
        printf 'Refusing to report on a selection that selected nothing.\n'
        exit 1
        ;;
    esac
  done
fi

fi # end of the real-gates branch

# --- Manifest -----------------------------------------------------------------

printf '\n================ BATTERY MANIFEST ================\n'
failed=0
index=0
while [ "$index" -lt "${#GATE_NAMES[@]}" ]; do
  name="${GATE_NAMES[$index]}"
  code="${GATE_CODES[$index]}"
  if [ "$code" -eq 0 ]; then
    printf '  PASS  %-28s exit %s\n' "$name" "$code"
  else
    printf '  FAIL  %-28s exit %s\n' "$name" "$code"
    failed=$((failed + 1))
  fi
  index=$((index + 1))
done
printf '=================================================\n'

# 🔴 THE EXECUTED LEG SET, EMITTED. The claim "the runner cannot differ from the
# declaration" is exactly the kind that must be measured rather than argued, and
# it cannot be measured against a manifest that only a human reads. This line is
# machine-comparable against `gates.json`; scripts/gates/verify-declaration.sh
# does the comparison.
printf 'EXECUTED_LEGS %s\n' "$(printf '%s\n' "${GATE_NAMES[@]}" | sort | tr '\n' ' ')"

if [ "${#GATE_NAMES[@]}" -eq 0 ]; then
  printf 'NO GATES RAN. That is a failure of this script, not a clean tree.\n'
  exit 1
fi

printf '%d of %d gates failed.\n' "$failed" "${#GATE_NAMES[@]}"

# 🔴 A GREEN ABOVE IS NOT YET A BATTERY RESULT. `0 of N failed` is a claim about
# the legs that RAN; whether those were the legs this repository declares is a
# separate question, and the whole reason the two lists drifted for as long as
# they did is that nobody was asked it. That question is now asked HERE, in the
# run, not left as a remembered follow-up: 5 of 14 audited landing records had
# no verify-declaration invocation on file — their safety rested on a human
# reading EXECUTED_LEGS.
#
# The runner writes its own EXECUTED_LEGS line to a scratch log and puts it
# through the same verifier a bank auditor uses. In self-test mode the
# expectation INVERTS rather than the step being skipped (a harness check that
# skips the step most likely to break is checking the easy half): the self-test
# log runs synthetic legs and no declared one, so the verifier MUST reject it —
# a verifier that passes that log is a verifier that cannot fail on anything.
#
# 🔴 AUDITOR TRAP, recorded where the trailer is generated: every battery log
# CONTAINS the string `verify-declaration.sh` because this trailer prints it.
# String presence in a log is NOT evidence the verifier ran — only an
# invocation/result record like the block below is.
SELF_VERIFY_LOG=$(mktemp "${TMPDIR:-/tmp}/battery-self-verify.XXXXXX") || {
  printf '🔴 could not create the self-verification scratch log.\n'
  exit 1
}
printf 'EXECUTED_LEGS %s\n' "$(printf '%s\n' "${GATE_NAMES[@]}" | sort | tr '\n' ' ')" \
  > "$SELF_VERIFY_LOG"
printf '\n--- declaration verification (in-run, mandatory) ---\n'
"$REPO_ROOT/scripts/gates/verify-declaration.sh" "$SELF_VERIFY_LOG"
verify_rc=$?
rm -f "$SELF_VERIFY_LOG"
if [ -n "${AION_BATTERY_SELF_TEST:-}" ]; then
  if [ "$verify_rc" -eq 1 ]; then
    printf 'self-test: verify-declaration.sh REJECTED the synthetic leg set (exit 1) — its red path is proven.\n'
  else
    printf '🔴 self-test: verify-declaration.sh exited %d on a log it must reject.\n' "$verify_rc"
    printf 'A verifier that cannot fail on the synthetic set cannot fail on anything.\n'
    exit 1
  fi
elif [ -n "${ONLY:-}" ]; then
  # A deliberate subset is LOUD three times over (the banner above, the
  # verifier's rejection here naming every skipped leg, and the subset
  # EXECUTED_LEGS in any banked copy), so its rc stays the selected legs'
  # truth: folding the expected rejection in would make the escape hatch exit
  # 1 even when the leg under test passed, which un-usables the hatch without
  # closing any laundering path the banner has not already closed.
  printf 'partial run: the rejection above is EXPECTED — it names what this run skipped.\n'
elif [ "$verify_rc" -ne 0 ]; then
  printf '🔴 the executed leg set does not verify against gates.json (exit %d).\n' "$verify_rc"
  printf 'This run is NOT a battery result regardless of the manifest above.\n'
  exit 1
fi

# Independent re-verification of the BANKED log stays available (and is what an
# auditor of the bank runs — the in-run check above verified this process's leg
# set, not the file you saved):
printf '\nTo re-verify a captured copy of this output: scripts/gates/verify-declaration.sh <this-log>\n'

if [ "$failed" -ne 0 ]; then
  exit 1
fi
exit 0
