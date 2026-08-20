#!/usr/bin/env bash
#
# Measures the claim `scripts/battery.sh` makes about itself: that the set of
# gates it EXECUTED is exactly the set `gates.json` DECLARES.
#
# WHY THIS IS NOT THE DRIFT-CHECKER THAT WAS REJECTED
#
# When aion's two gate lists were collapsed into one, a drift-checker comparing
# the two was refused on the grounds that it would be a THIRD copy of the same
# list — the cure for one truth in two places is subtraction, not a third party
# to referee the pair. That still holds, and this is a different instrument.
#
# It holds no list of its own. It reads the one declaration and compares it
# against what a run actually did. The battery's claim — "the two sets cannot
# differ by construction" — is exactly the kind of confident claim that stops
# people measuring, and construction arguments have been wrong here before: the
# same script argued a fixed population while `if require_tool gleam` was
# quietly shrinking it. So the claim gets an instrument.
#
# WHAT IT CHECKS
#
#   1. Every declared leg appears in the run.  A leg that vanished — skipped,
#      swallowed by a conditional, lost to a reader bug — is the original defect
#      returning, and a shrunken population reads as a clean tree.
#   2. Every executed leg was declared.  A leg the runner invents is a gate
#      nobody can find by reading the declaration.
#   3. The COUNTS match, not just the sets.  Two legs sharing a name collapse
#      into one set member, so a set comparison alone would call a declaration
#      with a duplicated name clean while one of the pair never ran.
#   4. The log carries EXACTLY ONE `EXECUTED_LEGS` line.  Two concatenated runs
#      in one file would otherwise be judged on whichever line was read first.
#
# WHAT IT CANNOT SEE — stated so the green is not read as more than it is.
# It compares NAMES. It does not know whether a leg's `cmd` ran the thing the
# leg is named after; a leg named `rust:clippy` whose command is `true` passes
# here. That is the paired-test/posture-control layer's job, not this one's.
#
# ITS OWN RED PATH IS RUNNABLE, and does not need a broken battery to show:
#
#   AION_BATTERY_SELF_TEST=1 scripts/battery.sh > /tmp/self.log 2>&1
#   scripts/gates/verify-declaration.sh /tmp/self.log     # MUST exit 1
#
# Self-test mode runs two synthetic legs and no declared one, so a verifier
# that passes on that log is a verifier that cannot fail on anything.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DECLARATION="$REPO_ROOT/gates.json"

# `|| exit $?` — a sourced refusal is a `return`, and unchecked it is advice.
# The interpreter comes from `select_python3` for the same reason the battery's
# does: PRESENCE ON THE PATH IS NOT EXECUTABILITY, and this repository has a
# cross-built python3 sitting first on the PATH to prove it.
. "$REPO_ROOT/scripts/gates/lib.sh" || exit $?

if [ "$#" -ne 1 ]; then
  printf 'usage: %s <battery-log>\n' "$0" >&2
  printf '  the log is the full output of scripts/battery.sh, which ends with an\n' >&2
  printf '  EXECUTED_LEGS line. Capture it with: scripts/battery.sh > run.log 2>&1\n' >&2
  exit 2
fi

LOG="$1"

if [ ! -f "$LOG" ]; then
  printf '🔴 no such battery log: %s\n' "$LOG" >&2
  exit 2
fi

if [ ! -f "$DECLARATION" ]; then
  printf '🔴 no declaration at %s — there is nothing to verify against.\n' "$DECLARATION" >&2
  exit 2
fi

PYTHON3=$(select_python3) || {
  printf '🔴 cannot verify: no working python3 (see above).\n' >&2
  exit 2
}

"$PYTHON3" - "$DECLARATION" "$LOG" <<'VERIFIER'
import json
import sys
from collections import Counter

declaration_path, log_path = sys.argv[1], sys.argv[2]

try:
    document = json.load(open(declaration_path))
except (OSError, ValueError) as exc:
    sys.exit(f"🔴 could not read the declaration {declaration_path}: {exc}")

declared = [leg["name"] for leg in (document.get("legs") or [])]
if not declared:
    sys.exit("🔴 the declaration lists no legs — an empty battery is a failure, not a clean tree.")

try:
    log_lines = open(log_path, errors="replace").read().splitlines()
except OSError as exc:
    sys.exit(f"🔴 could not read the battery log {log_path}: {exc}")

marker = "EXECUTED_LEGS "
executed_lines = [line for line in log_lines if line.startswith(marker)]

if not executed_lines:
    sys.exit(
        f"🔴 no EXECUTED_LEGS line in {log_path}.\n"
        "   Either the battery did not reach its manifest (it was killed, or it\n"
        "   died before the end) or the log is not a battery log. Absence is NOT\n"
        "   a pass: a run that emitted nothing to compare has been verified of\n"
        "   nothing."
    )

# Two runs in one file: judging on the first line silently grades the wrong run.
if len(executed_lines) > 1:
    sys.exit(
        f"🔴 {len(executed_lines)} EXECUTED_LEGS lines in {log_path} — this log holds\n"
        "   more than one battery run. Verify each run against its own log."
    )

executed = executed_lines[0][len(marker):].split()

declared_counts, executed_counts = Counter(declared), Counter(executed)

missing = sorted((declared_counts - executed_counts).elements())
extra = sorted((executed_counts - declared_counts).elements())

print(f"declaration : {declaration_path}")
print(f"battery log : {log_path}")
print(f"declared    : {len(declared)} legs")
print(f"executed    : {len(executed)} legs")

failed = False

if missing:
    failed = True
    print("\n🔴 DECLARED BUT NOT EXECUTED — the population shrank:")
    for name in missing:
        print(f"     {name}")

if extra:
    failed = True
    print("\n🔴 EXECUTED BUT NOT DECLARED — a gate no reader of gates.json can find:")
    for name in extra:
        print(f"     {name}")

duplicates = sorted(name for name, count in declared_counts.items() if count > 1)
if duplicates:
    failed = True
    print("\n🔴 DUPLICATE NAMES IN THE DECLARATION — a name is a leg's only identity")
    print("   here, so two legs sharing one cannot be told apart in any manifest:")
    for name in duplicates:
        print(f"     {name} × {declared_counts[name]}")

if failed:
    print("\nVERDICT: the executed set is NOT the declared set.")
    sys.exit(1)

print(f"\nVERDICT: exact match — all {len(declared)} declared legs executed, and nothing else.")
print("(names only: this says every leg RAN, not that any leg TESTS what it is named after.)")
VERIFIER
