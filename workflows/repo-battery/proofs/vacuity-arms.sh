#!/usr/bin/env bash
#
# Proof of the vacuity scanner (bin/scan-vacuity.sh).
#
# 🔴 THIS PROOF NEVER PRINTS A MARKER. The thing under test scans battery logs
# for an announcement token; if this proof echoed a matched line, that line would
# land in the battery log of any run that executed this proof, and the next scan
# would find the gate rather than the defect. Every arm below reports COUNTS,
# ATTRIBUTION and VERDICTS only. Fixtures live in files, never in this output.
#
# The arms exist because a scanner has two failure directions and only one of
# them is loud. Missing an announcement (A, B, C2) ships a false green. Matching
# something that is not one (C) manufactures a false red, which teaches the
# wave-through that eventually covers a real red. Mis-attributing one (G2) is
# worse than both: the count stays right while the name goes wrong, so nothing
# in a summary can show it. D is the guard that outranks all of them — a scanner
# broken outright must refuse, never report a comfortable zero.

set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
SCANNER="$SCRIPT_DIR/../bin/scan-vacuity.sh"
FIXTURES="$SCRIPT_DIR/fixtures/vacuity"
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../../.." && pwd)

# 🔴 THE MARKER IS READ FROM THE DECLARATION, NOT HARDCODED HERE, and that is
# the arm nobody would think to write. The fixtures below hold announcement
# lines copied verbatim from real runs; taking the marker from `gates.json`
# turns every arm into a check that THE DECLARED MARKER STILL MATCHES WHAT THE
# SUITES ACTUALLY EMIT. A typo in the declaration cannot then pass as a clean
# scan — it fails here by name, with the fixture that stopped matching.
#
# A hardcoded copy would have been a second source of truth for the same string,
# and the two would agree right up until the moment the answer depended on it.
MARKER=$(jq -r '.vacuity_marker // empty' "$REPO_ROOT/gates.json")
INFRA_MARKER=$(jq -r '.infrastructure_marker // empty' "$REPO_ROOT/gates.json")
if [[ -z "$MARKER" ]]; then
  printf 'NON-VACUITY GUARD FAILED: %s/gates.json declares no vacuity_marker, so\n' "$REPO_ROOT"
  printf 'every arm below would be testing the empty string. Refusing.\n'
  exit 1
fi
if [[ -z "$INFRA_MARKER" ]]; then
  printf 'NON-VACUITY GUARD FAILED: %s/gates.json declares no infrastructure_marker,\n' "$REPO_ROOT"
  printf 'so arm J would be testing the empty string. Refusing.\n'
  exit 1
fi

failures=0
checked=0

# arm NAME EXPECT_RC EXPECT_TOTAL EXPECT_UNATTRIBUTED EXPECT_LEGS FIXTURE [MARKER] [LEGS_FILE]
arm() {
  local name=$1 want_rc=$2 want_total=$3 want_unattr=$4 want_legs=$5 fixture=$6
  # 🔴 `${7-…}`, NOT `${7:-…}`. The colon form substitutes the default when the
  # argument is EMPTY as well as unset, so the empty-marker arm silently received
  # the real marker and reported the scanner broken when the scanner was never
  # asked. A control that cannot deliver its own input is not a control, and this
  # one failed in the direction that looks like a finding.
  local marker=${7-$MARKER}
  local legs=${8-$FIXTURES/declared-legs.txt}
  local out rc total unattr got
  checked=$((checked + 1))

  set +e
  out=$(bash "$SCANNER" "$fixture" "$marker" "$legs" 2>&1)
  rc=$?
  set -e

  if [[ "$rc" -ne "$want_rc" ]]; then
    printf 'ARM FAILED [%s]: exit %d, expected %d\n' "$name" "$rc" "$want_rc"
    failures=$((failures + 1))
    return 0
  fi
  # A refusal has no JSON to read; its exit code IS the assertion.
  if [[ "$want_rc" -ne 0 ]]; then
    printf '  %-46s refused as required (exit %d)\n' "$name" "$rc"
    return 0
  fi

  total=$(jq -r '.total' <<<"$out")
  unattr=$(jq -r '.unattributed | length' <<<"$out")
  if [[ "$total" -ne "$want_total" || "$unattr" -ne "$want_unattr" ]]; then
    printf 'ARM FAILED [%s]: total=%s unattributed=%s, expected total=%s unattributed=%s\n' \
      "$name" "$total" "$unattr" "$want_total" "$want_unattr"
    failures=$((failures + 1))
    return 0
  fi

  # 🔴 ASSERT THE ATTRIBUTION, NOT JUST THE COUNT. A count is a proxy and it
  # cannot see an announcement handed to the WRONG leg — the totals are identical
  # either way. A mutation making every forged frame a real one survived this
  # proof until this check existed, because the arms measured how many rather
  # than whose.
  got=$(jq -r '[.legs[].name] | join(",")' <<<"$out")
  if [[ "$got" != "$want_legs" ]]; then
    printf 'ARM FAILED [%s]: attributed to [%s], expected [%s]\n' "$name" "$got" "$want_legs"
    failures=$((failures + 1))
    return 0
  fi
  printf '  %-46s total=%s unattr=%s legs=[%s]\n' "$name" "$total" "$unattr" "$got"
}

# ── A: a gleam-family announcement inside a leg frame is found and attributed.
arm 'A gleam-family attributed' 0 1 0 'rust:test' "$FIXTURES/a-gleam-family.log"

# ── B: a costly-family announcement — a DIFFERENT token — is found by the same
#      pattern. This is what "token-agnostic" means, and it is the property an
#      enumerated gate cannot have.
arm 'B costly-family, different token' 0 1 0 'rust:test-all-features' "$FIXTURES/b-costly-family.log"

# ── B2: two families in ONE log. Four families exist in this estate and two of
#       them were in nobody's list when this was written.
arm 'B2 two families in one log' 0 2 0 'rust:test' "$FIXTURES/b2-two-families.log"

# ── C: rustc quoting the source line that DEFINES the token is not an
#      announcement. This is the false-positive that once inflated the costly
#      gate, and it fired precisely when a call site was deleted.
arm 'C rustc diagnostic is not an announcement' 0 0 0 '' "$FIXTURES/c-rustc-diagnostic.log"

# ── C2: the announcement fused mid-line onto libtest output, exactly as observed
#       in a real run (`... okSKIPPED (PROVES NOTHING): ...`). An anchored
#       pattern misses this; 5 of 198 real lines had this shape.
arm 'C2 mid-line fusion, no anchor' 0 1 0 'rust:test' "$FIXTURES/c2-midline-fusion.log"

# ── D: THE NON-VACUITY GUARD. The scanner proves its own matcher before reading
#      a log — a positive line it must match and a rustc-framed line it must not
#      — and refuses rather than reporting a comfortable zero when either fails.
#      That guard cannot be armed from here (a wrong marker is a legitimate clean
#      scan, not a broken matcher); it is held by the mutation record instead:
#      disabling the exclusion or the match kills 9 of these arms outright.
#      What CAN be armed from here are the inputs the scanner must refuse.
arm 'D2 empty marker refuses' 2 0 0 '' "$FIXTURES/a-gleam-family.log" ''
arm 'D3 absent log refuses' 2 0 0 '' "$FIXTURES/does-not-exist.log"

# ── D4: a marker that is genuinely absent from the log is a CLEAN scan, not a
#       refusal. A repository with no skipped proofs must be able to be green,
#       or the verdict stops meaning anything.
arm 'D4 absent marker is clean, not a refusal' 0 0 0 '' "$FIXTURES/a-gleam-family.log" 'NO-SUCH-MARKER-IN-THIS-LOG'

# ── E: an announcement outside any leg frame is reported as UNATTRIBUTED, so the
#      verifier can refuse rather than silently attributing it to nothing.
arm 'E unattributed announcement is reported' 0 1 1 '' "$FIXTURES/e-unattributed.log"

# ── F: an announcement after the battery manifest belongs to no leg. The fixture
#      puts one there deliberately: an earlier version ended at the banner, so a
#      greedy opener and a correct one produced identical output and the arm
#      passed under both.
arm 'F post-manifest announcement is unattributed' 0 2 1 'rust:test' "$FIXTURES/f-manifest-banner.log"

# ── G: THE FORGED FRAME, as it really occurs. The 24-leg battery log for the
#      #113 landing has 25 openers, the extra being `=== must-be-claimed gate ===`
#      printed BY the `estate-guard` leg. Here a real closer follows, and
#      attribution must land on `estate-guard`.
arm 'G forged frame, real closer follows' 0 1 0 'estate-guard' "$FIXTURES/g-forged-frame.log"

# ── G2: THE FORGED FRAME WITH NO CLOSER — the arm that actually tests the
#       declaration guard. Attribution is driven by the CLOSER's name, so a real
#       closer repairs a hijacked opener and arm G passes with or without the
#       guard (measured: the mutant survived it). Truncate the log before the
#       closer and the opener's name is all there is: an unguarded scanner
#       attributes the announcement to the phantom leg `must-be-claimed gate`,
#       which matches no declared leg downstream and is dropped in silence.
arm 'G2 forged frame, no closer (guard arm)' 0 1 0 'estate-guard' "$FIXTURES/g2-forged-no-closer.log"

# ── I: A CLOSER NAMING A DIFFERENT LEG THAN THE OPEN FRAME. The opener and the
#      closer are two independent statements of whose output this is; when they
#      disagree the log is corrupt and this scanner does not know the answer.
#      Guessing either name would present a guess as a measurement, so the
#      announcements go to `unattributed` and the verifier refuses.
#
#      This is also the ONLY condition under which the closer's name and the
#      running `current` can differ. Before this guard existed, a mutation
#      swapping one for the other survived every arm — not because the arms were
#      weak but because the two were always equal. With the guard in place the
#      swap is an EQUIVALENT mutant by construction, which is a different thing
#      from an untested one, and worth saying so plainly.
arm 'I closer/opener mismatch is unattributed' 0 1 1 '' "$FIXTURES/i-frame-mismatch.log"

# ── J: THE INFRASTRUCTURE MARKER, read from the declaration like the vacuity one
#      and exercised against a line copied from the run that hit a real Hex
#      registry outage. The fixture deliberately also contains the two test-NAME
#      lines carrying the lowercase word `infrastructure`: 102 such lines appear
#      in the real log, and a marker that matched them would classify every run
#      as an outage. Exactly one line here is the classification.
arm 'J infrastructure marker, not test names' 0 1 0 'rust:test-all-features' "$FIXTURES/j-infrastructure.log" "$INFRA_MARKER"

# ── H: an empty declaration refuses rather than turning every real frame into a
#      forgery. Loud for the wrong reason is still the wrong reason.
arm 'H empty declaration refuses' 2 0 0 '' "$FIXTURES/a-gleam-family.log" "$MARKER" "$FIXTURES/empty-legs.txt"
arm 'H2 absent declaration refuses' 2 0 0 '' "$FIXTURES/a-gleam-family.log" "$MARKER" "$FIXTURES/nope.txt"

if [[ "$checked" -eq 0 ]]; then
  printf 'NON-VACUITY GUARD FAILED: no arms ran, so this proof asserted nothing\n'
  printf 'while reporting success.\n'
  exit 1
fi
if [[ "$failures" -ne 0 ]]; then
  printf '%d of %d vacuity arm(s) failed.\n' "$failures" "$checked"
  exit 1
fi
printf 'vacuity scanner ok: %d arms — miss, false match, and MIS-ATTRIBUTION all covered\n' "$checked"
