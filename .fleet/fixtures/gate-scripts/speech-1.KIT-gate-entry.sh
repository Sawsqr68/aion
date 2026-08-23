#!/bin/bash
set -u
# SPEECH-1 gate: sdks/liminal-py implementation flight (brief: .fleet/BRIEF.md).
#
# WHAT THIS GATE PROVES, AND WHAT IT DOES NOT (scope note): two dimensions.
# (1) PATHS census — the flight's wall in the FILE dimension: every change
#     confined to sdks/liminal-py/ or .fleet/. This is the MECHANICAL
#     enforcement of the no-kernel/no-spine/no-Rust-changes fence: a builder
#     that drifts outside the wall goes red here regardless of intent.
# (2) The package's OWN suite — pytest green inside a fresh venv, which
#     includes the R1 golden-vector round-trip (decode AND re-encode
#     byte-identical against docs/wire/golden-trace/, read-only in-repo).
# What it does NOT prove: R-substance (normative token derivation, §16.3
# persistence discipline, E2E-against-spine quality) — that is CONTENT and
# lives at the judge stage. A green here must never be read as acceptance.
#
# Why the CARGO closure is empty for this flight: every allowed change is a
# NEW file under sdks/liminal-py/ (not referenced by the workspace build
# graph — an include of a then-nonexistent path would have failed the base
# build) or under .fleet/. The base commit's Rust health is attested by the
# publish-wave gates and the drift-gate receipt named in the cut message.
# The battery scope follows the closure, and the Rust closure is empty.
BASE=__CUT_COMMIT__
echo "GATE SCOPE: paths census + package suite against base ${BASE} (file dimension + package tests; R-substance = judge)"

changed=$(git diff --name-only "${BASE}"..HEAD)
if [ -z "$changed" ]; then
  echo "GATE-RED: empty diff against base — no implementation committed"
  exit 1
fi

# Positive control: the deliverable package must be in the diff. Keeps the
# census from passing vacuously on a rig-only tree.
if ! printf '%s\n' "$changed" | grep -qx "sdks/liminal-py/pyproject.toml"; then
  echo "GATE-RED: deliverable sdks/liminal-py/pyproject.toml absent from the diff"
  exit 1
fi

# The wall, file dimension: every changed path under sdks/liminal-py/ or .fleet/.
escapees=$(printf '%s\n' "$changed" | grep -Ev '^(sdks/liminal-py/|\.fleet/)' || true)
if [ -n "$escapees" ]; then
  echo "GATE-RED: paths outside the wall (no-kernel/no-spine/no-Rust fence, file dimension):"
  printf '%s\n' "$escapees"
  exit 1
fi

# Package suite in a fresh venv OUTSIDE the tree (a committed venv would
# itself be an escapee; positive tooling control before any test verdict).
if ! command -v python3 >/dev/null 2>&1; then
  echo "GATE-RED: python3 absent at venue — tooling precondition unmet (named cause: python3_missing)"
  exit 1
fi
VENV_DIR="$(mktemp -d /tmp/speech1-gate-venv.XXXXXX)"
trap 'rm -rf "$VENV_DIR"' EXIT
python3 -m venv "$VENV_DIR" || { echo "GATE-RED: venv creation failed (named cause: venv_failed)"; exit 1; }
"$VENV_DIR/bin/pip" install --quiet --upgrade pip || { echo "GATE-RED: pip bootstrap failed (named cause: pip_bootstrap_failed)"; exit 1; }
"$VENV_DIR/bin/pip" install --quiet -e "sdks/liminal-py[test]" || { echo "GATE-RED: package install failed (named cause: package_install_failed)"; exit 1; }

# Positive control for the test runner itself: the suite must EXIST and
# collect >0 tests — an empty collection green is absence wearing a tick.
# (Hermes F1, measured at pytest 9.1.1: a zero-test tree ends with
# "no tests collected", which a bare *" tests collected"* glob MATCHES —
# so red the zero-state FIRST, then require a leading nonzero count.)
collected=$("$VENV_DIR/bin/python" -m pytest sdks/liminal-py --collect-only -q 2>/dev/null | tail -1)
if printf '%s' "$collected" | grep -q "no tests collected"; then
  echo "GATE-RED: pytest collected no tests (named cause: empty_suite) — got: ${collected}"
  exit 1
fi
if ! printf '%s' "$collected" | grep -qE '^[1-9][0-9]* tests? collected'; then
  echo "GATE-RED: could not verify a nonzero collection count (named cause: empty_suite) — got: ${collected}"
  exit 1
fi

"$VENV_DIR/bin/python" -m pytest sdks/liminal-py -q
rc=$?
if [ "$rc" -ne 0 ]; then
  echo "GATE-RED: package suite failed (pytest rc=${rc})"
  exit 1
fi

echo "GATE-GREEN: wall held (paths census), package suite green (pytest rc=0) against base ${BASE}"
exit 0
