#!/usr/bin/env bash
#
# Vacuity scanner: find tests that PASSED while announcing that they proved
# nothing, and attribute each announcement to the leg that produced it.
#
# WHY THIS EXISTS. A runtime-gated skip returns `Ok(())`, so libtest counts it as
# a PASS. The suites in this estate are honest about it — they write
# `… (PROVES NOTHING): …` to the raw stderr handle precisely so the line survives
# libtest's capture, which swallows `println!`/`eprintln!` on a passing test. So
# the announcement is emitted, correct, and visible to a human reading the log.
#
# 🔴 A GATE IS NOT A HUMAN READING THE LOG. Measured on the frozen fleet archive
# `fleet/rb-lockfix-result` @ 113401a7b (`.fleet/gates/RED-LATEST.md`): **198
# announcement lines in a single run**, and the run reported those legs green.
# The instrument spoke and nothing listened. That is what this file changes.
#
# 🔴 WHY NOT AN ENUMERATED POPULATION. `scripts/battery-costly-announcements.sh`
# already enumerates five suites and checks they DO announce; it is correct and
# this file does not replace it — the two are complements, asserting opposite
# directions. That gate catches a costly proof that stopped announcing. This one
# catches an announcement nobody's gate consumes. Neither subsumes the other.
#
# The reason this one cannot be a list: the same archive shows **four** distinct
# skip families, not the two that were known when this work started —
#
#     SKIPPED (PROVES NOTHING) .............. 190
#     COSTLY_PROOF_SKIPPED (PROVES NOTHING) ... 5
#     ISSUE36_E2E_SKIPPED (PROVES NOTHING) .... 2
#     SKIP (PROVES NOTHING) ................... 1
#
# Two of those four were not in anybody's list. An enumeration cannot see a
# member nobody wrote down, so this file enumerates nothing.
#
# ── THE PATTERN, AND WHY EACH HALF IS LOAD-BEARING ───────────────────────────
#
# MATCH   : the line CONTAINS  `(PROVES NOTHING): `
# EXCLUDE : the line MATCHES   `^[[:space:]]*[0-9]+[[:space:]]*\|`
#
# Both halves were measured against the 198 real lines, not reasoned about.
#
# 🔴 NO `^` ANCHOR. The announcement goes to the REAL stderr handle while
# libtest's progress output goes to stdout; under `2>&1` they interleave. 5 of
# the 198 lines do not start at column 0, and one of them reads
#
#     test …::a_rejected_module_is_a_defect ... okSKIPPED (PROVES NOTHING): …
#
# — the stderr write landing mid-line, fused onto libtest's `ok` with no
# separator at all. An anchored pattern misses it. This is the same defect that
# was removed from the costly gate on 2026-08-20 after six runs measured
# anchored 0 / unanchored 1 every time; it is not re-introduced here.
#
# 🔴 THE EXCLUSION IS WHAT BUYS SOURCE-IMMUNITY. Cargo quotes source in its
# diagnostics, and 8 real lines under `crates/**/*.rs` carry the match pattern
# (e.g. `"SKIPPED (PROVES NOTHING): \`gleam\` is not available on PATH, …"`).
# The costly gate was once inflated by exactly this: a dead-code warning quoting
# `pub const SKIP_TOKEN: &str = "COSTLY_PROOF_SKIPPED";`, and it fired precisely
# when a call site was deleted — the case the gate existed to detect.
#
# rustc frames quoted source as `<line-number> | <source>`. Excluding that shape
# was verified both ways: it drops **0** of the 198 real announcements, and it
# excludes **3 of 3** simulated diagnostics built from the real source lines.
#
# An earlier draft of this scanner required the announcement to carry a
# `<file>.rs:<line>` from `Location::caller()`. It was refuted by the same
# corpus: it missed 3 of 198, being exactly the two families that carry no
# caller location — and it missed them SILENTLY, which is the direction that
# ships a false green. The lesson is written here because the next person to
# "tighten" this pattern will reach for the same idea.
#
# ── ATTRIBUTION ──────────────────────────────────────────────────────────────
#
# 🔴 A WHOLE-LOG COUNT CANNOT SAY WHICH LEG WAS VACUOUS, and a verdict that
# cannot name its subject sends the reader hunting. `scripts/battery.sh :: gate()`
# already frames every leg:
#
#     === <name> ===
#         …leg output…
#     --- <name> exit <code> ---
#
# so every announcement is attributed to the leg that emitted it. An announcement
# found OUTSIDE any frame is a refusal, not a pass: it means either the log is
# malformed or the framing changed, and in both cases this scanner no longer
# knows what it is measuring.
#
# ── WHY THIS RUNS AT VERIFY TIME AND NEVER AS A LEG ──────────────────────────
#
# 🔴 This scanner PRINTS the offending lines as its reason, so its own output
# contains the marker. It therefore reads a battery log that is already closed,
# from outside the battery. Were it ever declared as a leg, its output would land
# in the log a later scan reads and the gate would find itself.
#
# ── THE SELF-CHECK ───────────────────────────────────────────────────────────
#
# 🔴 A SCANNER THAT FINDS NOTHING BECAUSE IT IS BROKEN MUST NOT BE
# INDISTINGUISHABLE FROM A CLEAN RUN. Before reading the log at all, the pattern
# is run against a known-positive line and a known-negative one. If the positive
# does not match, or the negative does, this exits 2 (refusal) rather than
# reporting a comfortable zero. The channel of the instrument is proven before
# the instrument is trusted.

set -euo pipefail

# The token every family in this estate uses, measured across all four: 198 of
# 198 real announcements carry it. Passed in rather than hardcoded so the repo
# under test owns its own convention — see verify.sh, which refuses loudly if
# the repository declares none.
marker=${2:-}

# A real announcement, kept verbatim from the archived run, and a real rustc
# diagnostic quoting a real source line. These are the scanner's own control.
readonly SELF_CHECK_POSITIVE='test x ... okSKIPPED (PROVES NOTHING): `gleam` is not available on PATH'
readonly SELF_CHECK_NEGATIVE='198 |         "SKIPPED (PROVES NOTHING): `gleam` is not available on PATH, so the \'
readonly DIAGNOSTIC_SHAPE='^[[:space:]]*[0-9]+[[:space:]]*\|'

usage() {
  printf 'usage: scan-vacuity.sh BATTERY_LOG MARKER DECLARED_LEGS_FILE\n' >&2
  printf '  BATTERY_LOG         a completed battery log framed by scripts/battery.sh gate()\n' >&2
  printf '  MARKER              the vacuity marker this repository announces with\n' >&2
  printf '  DECLARED_LEGS_FILE  one declared leg name per line; frames naming\n' >&2
  printf '                      anything else are not frames\n' >&2
}

if [[ $# -ne 3 ]]; then
  usage
  exit 2
fi

log_path=$1
legs_file=$3

if [[ ! -f "$legs_file" || ! -r "$legs_file" ]]; then
  printf 'REFUSED: declared-legs file is absent or unreadable: %s\n' "$legs_file" >&2
  exit 2
fi

# Read the declaration into a newline-delimited set.
#
# 🔴 NOT AN ASSOCIATIVE ARRAY. macOS ships bash 3.2.57, which has no
# `declare -A` — it is a bash 4 feature, and this estate's gate scripts run on
# the Mac as often as anywhere else. Found by running this, not by reading it:
# every arm refused with `declare: -A: invalid option`. The failure direction was
# right (a broken scanner refused rather than reporting a clean scan), but a gate
# that cannot start on the machine it is meant to guard is not a gate.
#
# A leg name is matched EXACTLY, delimiters included. A substring match would let
# `rust:test` claim `rust:test-all-features`'s frames — and those two legs run
# the same suites, so the mix-up would be invisible in the counts.
DECLARED_SET=$'\n'
declared_count=0
while IFS= read -r declared_name || [[ -n "$declared_name" ]]; do
  if [[ -z "$declared_name" ]]; then
    continue
  fi
  DECLARED_SET="$DECLARED_SET$declared_name"$'\n'
  declared_count=$((declared_count + 1))
done <"$legs_file"

# 🔴 AN EMPTY DECLARATION WOULD MAKE EVERY FRAME A FORGERY, so every
# announcement would land in `unattributed` and the run would refuse — loud, but
# for the wrong reason, and the reader would hunt the log instead of the
# declaration. Say which it is.
if [[ "$declared_count" -eq 0 ]]; then
  printf 'REFUSED: the declared-legs file names no legs (%s).\n' "$legs_file" >&2
  printf 'Every frame would be treated as a forgery and every announcement as\n' >&2
  printf 'unattributed. That is a declaration problem, not a log problem.\n' >&2
  exit 2
fi

# The `*` are unquoted so they glob; `"$1"` is quoted so a leg name containing a
# glob metacharacter is matched literally rather than as a pattern.
is_declared_leg() {
  case "$DECLARED_SET" in
    *$'\n'"$1"$'\n'*) return 0 ;;
  esac
  return 1
}

if [[ -z "$marker" ]]; then
  printf 'REFUSED: the vacuity marker is empty. An empty marker matches every\n' >&2
  printf 'line or none depending on the matcher, and either way this scanner\n' >&2
  printf 'would be reporting something other than what it measured.\n' >&2
  exit 2
fi

if [[ ! -f "$log_path" || ! -r "$log_path" ]]; then
  printf 'REFUSED: battery log is absent or unreadable: %s\n' "$log_path" >&2
  exit 2
fi

# `is_announcement LINE` — the whole pattern in one place, so the self-check and
# the scan cannot diverge. A second copy of a matcher is a matcher that will
# eventually disagree with itself.
is_announcement() {
  local line=$1
  case "$line" in
    *"$marker"*) ;;
    *) return 1 ;;
  esac
  # The exclusion is a regex rather than a glob because the shape is positional:
  # digits, optional space, a pipe, at the START of the line.
  if [[ "$line" =~ $DIAGNOSTIC_SHAPE ]]; then
    return 1
  fi
  return 0
}

# ── Prove the channel before using it ────────────────────────────────────────
if ! is_announcement "$SELF_CHECK_POSITIVE"; then
  printf 'SELF-CHECK FAILED: the scanner did not match a KNOWN announcement.\n' >&2
  printf 'A zero from this run would mean nothing. Refusing rather than\n' >&2
  printf 'reporting a clean scan.\n' >&2
  printf '  marker : %s\n' "$marker" >&2
  printf '  line   : %s\n' "$SELF_CHECK_POSITIVE" >&2
  exit 2
fi
if is_announcement "$SELF_CHECK_NEGATIVE"; then
  printf 'SELF-CHECK FAILED: the scanner matched a rustc DIAGNOSTIC quoting\n' >&2
  printf 'source. That is the false-positive this gate was rebuilt to avoid —\n' >&2
  printf 'it fires exactly when a call site is deleted, which is the case the\n' >&2
  printf 'gate exists to detect.\n' >&2
  printf '  line : %s\n' "$SELF_CHECK_NEGATIVE" >&2
  exit 2
fi

# ── Walk the log, attributing each announcement to its leg ───────────────────
current=""
legs_json='[]'
announcements_json='[]'
unattributed_json='[]'
total=0
mismatches=0

flush_leg() {
  local name=$1 lines=$2
  local count
  count=$(jq 'length' <<<"$lines")
  # 🔴 An `if`, not `[[ … ]] && return 0`. Under `set -e` the AND-list carries
  # the test's non-zero status out of the function, and this function is called
  # from the main loop where that would abort the scan mid-log — reporting fewer
  # announcements than the log holds, which is the silent direction.
  if [[ "$count" -eq 0 ]]; then
    return 0
  fi
  legs_json=$(jq -c --arg name "$name" --argjson lines "$lines" \
    '. + [{name:$name, announcements:$lines}]' <<<"$legs_json")
}

while IFS= read -r line || [[ -n "$line" ]]; do
  # A leg opens. `gate()` writes this before running the command.
  #
  # 🔴 A FRAME IS ONLY A FRAME IF IT NAMES A DECLARED LEG, and this is not
  # defensive programming — it is a real defect found in a real log. The 24-leg
  # battery log for the #113 landing contains 25 opener lines. The extra one is
  # at line 16080:
  #
  #     === must-be-claimed gate ===
  #
  # printed by the `estate-guard` LEG'S OWN OUTPUT. A leg can emit a line in the
  # shape of a frame, and an unvalidated scanner hands every later announcement
  # to a leg that does not exist — which matches no declared leg downstream and
  # is therefore dropped SILENTLY, the one direction that ships a false green.
  #
  # Checking the name against the declaration makes the forgery inert: the
  # phantom opener is ignored and `current` correctly remains `estate-guard`.
  # A leg name that appears in the log but not in the declaration puts its
  # announcements in `unattributed`, which refuses loudly.
  #
  # (An earlier version of this guard required the name to contain no `=`, on a
  # guess that the manifest banner was the culprit. It was not: the opener
  # pattern requires `=== ` with a SPACE, and the banner is sixteen `=` with no
  # space, so it never matched. The guess was never measured, and a mutation test
  # exposed it by surviving.)
  if [[ "$line" =~ ^===\ (.+)\ ===$ ]] && is_declared_leg "${BASH_REMATCH[1]}"; then
    flush_leg "$current" "$announcements_json"
    current="${BASH_REMATCH[1]}"
    announcements_json='[]'
    continue
  fi
  # A leg closes, carrying its own name and exit code. Validated against the
  # declaration for the same reason the opener is.
  if [[ "$line" =~ ^---\ (.+)\ exit\ ([0-9]+)\ ---$ ]] && is_declared_leg "${BASH_REMATCH[1]}"; then
    closing="${BASH_REMATCH[1]}"
    # 🔴 A CLOSER THAT DOES NOT MATCH THE OPEN FRAME MEANS THE LOG IS CORRUPT,
    # and picking one of the two names would be a guess presented as a
    # measurement. The opener and the closer are two independent statements of
    # whose output this is; when they disagree, this scanner does not know, and
    # the announcements go to `unattributed` so the verifier refuses.
    #
    # This is also the only condition under which the closer's name and the
    # running `current` can differ — which is why a mutation replacing one with
    # the other survived every other arm. Two mechanisms that always agree cannot
    # test each other.
    if [[ -n "$current" && "$current" != "$closing" ]]; then
      while IFS= read -r orphan; do
        if [[ -n "$orphan" ]]; then
          unattributed_json=$(jq -c --arg line "$orphan" '. + [$line]' <<<"$unattributed_json")
        fi
      done < <(jq -r '.[]' <<<"$announcements_json")
      mismatches=$((mismatches + 1))
    else
      flush_leg "$closing" "$announcements_json"
    fi
    current=""
    announcements_json='[]'
    continue
  fi
  is_announcement "$line" || continue
  total=$((total + 1))
  if [[ -z "$current" ]]; then
    unattributed_json=$(jq -c --arg line "$line" '. + [$line]' <<<"$unattributed_json")
    continue
  fi
  announcements_json=$(jq -c --arg line "$line" '. + [$line]' <<<"$announcements_json")
done <"$log_path"

# A log that ended mid-leg still owes its announcements to that leg.
flush_leg "$current" "$announcements_json"

jq -cn --argjson legs "$legs_json" --argjson unattributed "$unattributed_json" \
  --argjson total "$total" --arg marker "$marker" --argjson mismatches "$mismatches" \
  '{scanned:true, marker:$marker, total:$total, legs:$legs, unattributed:$unattributed, frame_mismatches:$mismatches}'
