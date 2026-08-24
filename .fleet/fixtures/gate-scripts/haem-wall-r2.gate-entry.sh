#!/bin/bash
set -u
# WALL flight gate: haematite fleet-records mechanical wall (brief: .fleet/BRIEF.md).
#
# WHAT THIS GATE PROVES, AND WHAT IT DOES NOT (scope note):
# (1) PATHS census — the flight's wall in the FILE dimension. Deliverables are
#     gates.json (new leg), a known-carries file beside it, scripts/release/,
#     red-first evidence under docs/evidence/, and the R4 doc section at the
#     repo's landing/release docs. Everything else in the tree is out of
#     bounds and reds here regardless of intent.
# (2) DELIVERABLE PRESENCE + MECHANICAL VALIDITY — the new leg exists in
#     gates.json by name, gates.json still parses as JSON (a battery with a
#     broken gates.json is a dead battery), cut-tag.sh exists, is executable,
#     and parses (sh -n), and red-first evidence is committed.
# What it does NOT prove: R-substance. The R1 leg CANNOT be executed
# expecting green on this tree — the rig tree itself carries .fleet/ BY
# DESIGN (fleet/* namespace, exempt), so a HEAD-axis run here is either a
# correct red or an exercise of the leg's namespace exemption, and neither
# is a gate-shaped assertion. The red/green demonstrations, the carry
# mechanism, the in-population positive, the enumeration guard, and the F6
# triple binding are CONTENT and live at the judge, who re-runs them with
# his own receipts. A green here must never be read as acceptance.
#
# Why the CARGO closure is empty for this flight: no Rust source may change
# (the census enforces it), so the base commit's battery history attests the
# workspace and no cargo leg runs here.
BASE=a4134c3cd78e18b2f81d64603d05b0275517ac99  # round 2: flight1 flagged head
echo "GATE SCOPE: paths census + deliverable presence/validity against base ${BASE} (file dimension; R-substance = judge)"

changed=$(git diff --name-only "${BASE}"..HEAD)
if [ -z "$changed" ]; then
  echo "GATE-RED: empty diff against base — no implementation committed"
  exit 1
fi

# Positive controls: the deliverables must be in the diff. Keeps the census
# from passing vacuously on a rig-only tree.
# Round-2 scope (FINDINGS-R2.md): the deliverable is the repaired
# cut-tag.sh plus its new evidence. gates.json/known-carries stand from
# flight1 and are validity-checked below, not diff-required.
if ! printf '%s\n' "$changed" | grep -qx "scripts/release/cut-tag.sh"; then
  echo "GATE-RED: deliverable scripts/release/cut-tag.sh absent from the diff"
  exit 1
fi
if ! printf '%s\n' "$changed" | grep -qE '^docs/evidence/'; then
  echo "GATE-RED: no red-first evidence committed under docs/evidence/"
  exit 1
fi

# The wall, file dimension. NOTE ON ANCHORING: this is an ALLOWLIST over
# changed paths, so root-anchored components are the CORRECT semantics here
# (each entry names an exact site) — this is not the invariant matcher, which
# must use the component form per standards/fleet-records-invariant.md.
escapees=$(printf '%s\n' "$changed" | grep -Ev '^(gates\.json$|known-carries[^/]*$|scripts/release/|docs/evidence/|gate-logs/[^/]+\.md$|CONTRIBUTING\.md$|README\.md$|docs/DOCUMENTATION\.md$|\.fleet/)' || true)
if [ -n "$escapees" ]; then
  echo "GATE-RED: paths outside the wall (no-Rust/no-crate/docs-and-gates-only fence, file dimension):"
  printf '%s\n' "$escapees"
  exit 1
fi

# Mechanical validity of the deliverables (named causes, no semantics).
if ! command -v python3 >/dev/null 2>&1; then
  echo "GATE-RED: python3 absent at venue — tooling precondition unmet (named cause: python3_missing)"
  exit 1
fi
if ! python3 -c 'import json,sys; json.load(open("gates.json"))' 2>/dev/null; then
  echo "GATE-RED: gates.json no longer parses as JSON (named cause: gates_json_invalid)"
  exit 1
fi
leg_present=$(python3 -c 'import json; d=json.load(open("gates.json")); print(sum(1 for l in d.get("legs",[]) if l.get("name")=="repo-tree-clean"))')
if [ "$leg_present" != "1" ]; then
  echo "GATE-RED: gates.json does not declare exactly one leg named repo-tree-clean (named cause: leg_absent_or_duplicated) — count: ${leg_present}"
  exit 1
fi
if [ ! -x scripts/release/cut-tag.sh ]; then
  echo "GATE-RED: scripts/release/cut-tag.sh missing or not executable (named cause: script_not_executable)"
  exit 1
fi
if ! sh -n scripts/release/cut-tag.sh; then
  echo "GATE-RED: cut-tag.sh fails sh -n syntax parse (named cause: script_syntax)"
  exit 1
fi
# The leg's cmd must itself parse: extract and sh -n it.
python3 -c 'import json; d=json.load(open("gates.json")); [print(l["cmd"]) for l in d["legs"] if l["name"]=="repo-tree-clean"]' > /tmp/wall-gate-legcmd.$$ || {
  echo "GATE-RED: could not extract repo-tree-clean cmd from gates.json (named cause: leg_cmd_missing)"; exit 1; }
if ! sh -n /tmp/wall-gate-legcmd.$$; then
  rm -f /tmp/wall-gate-legcmd.$$
  echo "GATE-RED: repo-tree-clean leg cmd fails sh -n syntax parse (named cause: leg_cmd_syntax)"
  exit 1
fi
rm -f /tmp/wall-gate-legcmd.$$

# (R2's no-force/no-override rule is JUDGE content, not a grep: the script's
# own documentation is REQUIRED to state "no --force", so a spelling check
# here would red on compliance. The judge reads the code.)

echo "GATE-GREEN: wall held (paths census), deliverables present and mechanically valid against base ${BASE}"
exit 0
