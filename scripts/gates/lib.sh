#!/usr/bin/env bash
# Gate-leg helpers, shared by every leg in `gates.json`.
#
# WHY THIS FILE EXISTS
#
# `gates.json` is now the ONE list of this repository's gate legs, and every leg
# there must be runnable by ANY runner — the house battery, the estate runner, a
# person pasting one line into a terminal. That means each leg's `cmd` has to be
# self-contained, so the helpers it needs cannot live inside the battery script.
#
# They live here, WITH the reasoning that produced them. That reasoning is the
# asset: each of these helpers exists because a gate once passed while proving
# nothing, and the comments record which one.
#
# 🔴 SOURCE THIS WITH `|| exit $?`. A sourced refusal is a `return`, not an
# `exit`, so an unchecked `.` makes a refusing library into advice the caller
# walks straight past. Every leg in `gates.json` checks it.

# --- running a leg inside a directory -----------------------------------------

# The `cd` happens in a subshell rather than via `env -C`, which BSD `env` did
# not carry until recently and which would fail on an older macOS with a message
# that looks nothing like the real problem.
#
# A MISSING DIRECTORY IS 127, NOT A PASS. A leg whose directory is gone has not
# been satisfied; it has been unable to ask its question.
in_dir() {
  local dir="$1"
  shift
  if [ ! -d "$dir" ]; then
    printf 'MISSING DIRECTORY: %s\n' "$dir"
    return 127
  fi
  ( cd "$dir" && "$@" )
}

# --- a missing toolchain is a failure, not a skip -----------------------------

# Fails the battery loudly when a required tool is absent, rather than skipping.
require_tool() {
  local tool="$1"
  local why="$2"
  if command -v "$tool" >/dev/null 2>&1; then
    return 0
  fi
  printf '\n=== toolchain: %s ===\n' "$tool"
  printf 'NOT INSTALLED. %s\n' "$why"
  printf 'This is a FAILURE, not a skip: a gate that cannot run is a gate that
proves nothing, and silently skipping it is how three gates in this repository
ended up correct and unexecuted.\n'
  return 127
}

# --- PRESENCE IS NOT EXECUTABILITY -------------------------------------------

# Prints the path of a python3 that ACTUALLY RUNS on this machine, or fails 127.
#
# 🔴 FOUND, NOT ANTICIPATED, on the machine this was written on. `command -v
# python3` there resolves to `~/.local/bin/python3` — which `file` reports as an
# `ELF 64-bit ... x86-64 ... for GNU/Linux 3.2.0` binary, sitting first on the
# PATH of an arm64 Mac. It cannot be exec'd at all; every invocation is 126,
# `cannot execute binary file`. Three further working python3s sat behind it.
#
# `require_tool` above would have called that tool PRESENT, because `command -v`
# answers "is there a file by this name on the PATH" and the question that
# matters is "will it run here". The two differ whenever a PATH entry is stale,
# cross-built, a broken symlink, or a wrapper for a runtime that is gone.
#
# So this selects by RUNNING each candidate, in PATH order, and taking the first
# that executes a trivial program. It hardcodes no interpreter path: candidates
# come from the PATH itself, so a box that puts its python somewhere unusual is
# served without this file knowing where that is. If none runs, that is 127 and
# the message names every path tried — a battery that cannot read its own
# declaration must say which interpreters it asked, or the next person debugs
# the wrong layer.
#
# Why this is not `require_tool`'s job: probing by execution is only safe for a
# tool with a known no-op invocation. `python3 -c pass` is one. There is no such
# universal argument, so `require_tool` keeps its cheaper question and the tools
# that can be probed properly are probed properly.
select_python3() {
  local candidate tried=""
  while IFS= read -r candidate; do
    [ -n "$candidate" ] || continue
    tried="$tried$candidate
"
    if "$candidate" -c 'pass' >/dev/null 2>&1; then
      printf '%s' "$candidate"
      return 0
    fi
  done < <(type -ap python3 2>/dev/null)

  printf 'NO WORKING python3.\n' >&2
  if [ -n "$tried" ]; then
    printf 'These were on the PATH and none of them executed:\n%s' "$tried" >&2
    printf 'Present on the PATH is not the same as runnable here — a cross-built or\n' >&2
    printf 'stale interpreter answers `command -v` and then exits 126.\n' >&2
  else
    printf 'No python3 was found on the PATH at all.\n' >&2
  fi
  return 127
}

# --- did the test run REACH THE END? -----------------------------------------

# Runs a cargo test invocation and asserts the run REACHED THE END (#120).
#
# A test run's exit code and its tally are two different claims, and only the
# first is about completeness. `--no-fail-fast` continues past failing tests, but
# a target that dies — or a run cut short — simply stops producing output, and
# the surviving tally still reads `0 failed`. That is how "688 passed, 1 failed"
# was taken as one known exception on an otherwise green suite when it was in
# fact a truncated run reported as a complete one, hiding a P0 (see the comment
# above `rust:test`).
#
# 🔴 THE OBVIOUS CONTROL — comparing the COUNT of target-start lines against the
# count of `test result:` lines — DOES NOT WORK, and this was measured rather
# than assumed. On fully green runs the two disagree:
#
#     .evidence/118/battery-final.log        45 targets, 48 results
#     .evidence/118/shakedown-allfeatures    240 targets, 243 results
#
# Because a test that RE-EXECUTES ITS OWN BINARY as a child prints the child's
# `running N tests` / `test result:` into the parent's stdout, mid-target — e.g.
# `config::load::home_tests::aion_home_env_probe_child` from the AION_HOME work.
# A counting gate would fire on every green run; worse, a real truncation
# coinciding with N child extras would net to zero and read as complete.
#
# So assert the PAIRING, not the count: every target-start line must be followed
# by at least one `test result:` before the next target line. Child extras land
# inside a target's own block and are tolerated by construction; a truncation
# leaves a started target with nothing after it.
#
# Validated both ways before being wired in: `unpaired=0` on three real green
# logs (including both counting-mismatch cases above) and `unpaired=1` on the
# same log with one result line removed.
#
# Note the filter trap: a legitimately filtered top-level run also reports
# `filtered out > 0`, so that field cannot be used to identify child lines — the
# `posture:*` gates filter deliberately.
#
# 🔴 TWO NOUNS THAT LOOK LIKE ONE, both consequences of those same child
# re-invocations. Worked out at two hands on the 2026-08-01 green run and
# written here because the next person re-derives them from scratch otherwise:
#
#   * `N passed` is a count of PASS EVENTS, not of DISTINCT TESTS. A subprocess
#     test passes twice — once inside its parent target's tally, once as the
#     filtered child. That run reported 3952 passed; distinct is ~3949.
#   * VERDICT LINES ARE NOT TARGETS. The same run had 243 `test result:` lines
#     and 240 targets; the gap is exactly the 3 nested child verdicts.
#
# Which is why this control counts TARGETS and pairs them rather than summing a
# column: a nested child verdict cannot reach it. Summing across verdict lines
# treats child re-invocations as peers of top-level runs, and an aggregate over
# heterogeneous rows is a number about no population at all. (The same run's
# `2133 filtered out` is 3 x 711 from one self-filtering binary — it looks like a
# third of the suite was skipped and nothing was.)
#
# And the pattern needs its COLON. `grep '^test result'` also matches a test
# NAMED `result_...`, which is how the 244-vs-243 discrepancy arose in the first
# place. Count `^test result:`.
paired_test() {
  local log
  log=$(mktemp) || return 1
  "$@" 2>&1 | tee "$log"
  local code=${PIPESTATUS[0]}

  local counts targets unpaired
  counts=$(awk '
    /^[[:space:]]+Running |^[[:space:]]+Doc-tests / { if (started) unpaired++; started = 1; targets++ }
    /^test result:/ { started = 0 }
    END { if (started) unpaired++; print targets + 0, unpaired + 0 }
  ' "$log")
  targets=${counts%% *}
  unpaired=${counts##* }
  rm -f "$log"

  if [ "$targets" -eq 0 ]; then
    printf 'COVERAGE CONTROL FAILED: the run started ZERO test targets.\n'
    printf 'An exit code from a run that tested nothing is not a result about this code.\n'
    return 1
  fi
  if [ "$unpaired" -ne 0 ]; then
    printf 'COVERAGE CONTROL FAILED: %d of %d target(s) started and never reported a result.\n' \
      "$unpaired" "$targets"
    printf 'The run was TRUNCATED. Its tally is not a coverage claim, whatever it says.\n'
    return 1
  fi
  printf 'coverage control ok: %d targets started, %d reached a result\n' "$targets" "$targets"
  return "$code"
}

# --- build-posture controls --------------------------------------------------

#
# Two crates carry a default-OFF feature that gates an entire surface, so a
# per-package run of either compiles none of it and exits 0:
#
#   aion-worker / liminal-transport  — the whole liminal transport
#   aion-proto  / generated          — the tonic/prost stubs the hand-written
#                                      wire contracts are supposed to be
#                                      checked AGAINST
#
# Both crates now ANNOUNCE their posture through a test name, which a runner
# always prints. But `reconcile-nextest.sh` states the law this would otherwise
# break: *a pinned test denominator that nothing mechanically compares is an
# instruction, not a control.* A printed announcement that nothing checks is
# observability, not a gate — a run where the wrong arm fires reconciles
# perfectly, because every count still adds up.
#
# So the gate asserts BOTH directions: the posture-appropriate name appeared,
# AND its opposite did not. That is also what anchors the negative arms — they
# assert little on their own, and deleting one would otherwise go unnoticed.
posture_control() {
  local label="$1" expect="$2" forbid="$3"
  shift 3
  local out code
  out=$("$@" 2>&1)
  code=$?
  if [ "$code" -ne 0 ]; then
    printf '%s\n' "$out"
    printf 'POSTURE CONTROL (%s): the run itself failed with exit %d\n' "$label" "$code"
    return "$code"
  fi
  if ! printf '%s' "$out" | grep -q -- "$expect"; then
    printf 'POSTURE CONTROL FAILED (%s): expected announcement `%s` never appeared.\n' "$label" "$expect"
    printf 'The build did not have the feature posture this gate believes it ran.\n'
    return 1
  fi
  if printf '%s' "$out" | grep -q -- "$forbid"; then
    printf 'POSTURE CONTROL FAILED (%s): forbidden announcement `%s` appeared.\n' "$label" "$forbid"
    printf 'This build is NOT the posture the gate believes it is.\n'
    return 1
  fi
  printf 'posture control ok (%s): %s\n' "$label" "$expect"
  return 0
}

# --- costly proofs announce their own absence (#86) --------------------------
#
# Kept in its own file since before this refactor; sourced here so a leg naming
# `costly_announcements` needs one source line rather than two.
. "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/scripts/battery-costly-announcements.sh" || return $?
