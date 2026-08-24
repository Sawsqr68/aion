#!/usr/bin/env python3
"""Compose the sleepreap_dev start input from the rig's parts.

Usage: build-input.py <docs-repo-root> <fulcrum-repo-root> > input.json
       (never bare `python3` on the estate Mac — /usr/bin/python3)

Refuses (exit 2) while OPERATING-GROUND.md still carries the
RECORD-RUN-PENDING slot. The ground must state this box's MEASURED truth from
the baseline record run before any dispatch composes: a brief that predicts
where it could cite is the fault the beamr#27 ground rule exists to prevent,
and fulcrum has the sharper version of it — main is ABSOLUTE GREEN on the
estate Mac, so any Linux-only finding would land on the builder as if it were
theirs unless the ground names it first.
"""
import json
import sys
from pathlib import Path

if len(sys.argv) < 3:
    sys.stderr.write(__doc__)
    sys.exit(2)

docs = Path(sys.argv[1])
fulcrum = Path(sys.argv[2])
rig = docs / "dispatch-rigs" / "sleepreap"

ground = (rig / "OPERATING-GROUND.md").read_text()
if "RECORD-RUN-PENDING" in ground:
    sys.stderr.write(
        "REFUSED: OPERATING-GROUND.md still carries RECORD-RUN-PENDING — fill "
        "it with the box's measured record-run truth before composing.\n"
    )
    sys.exit(2)

# The brief lives in the FULCRUM repo, not in docs: it is lane paper, ratified
# and landed there, and it travels to the builder verbatim from where it lands.
brief_path = fulcrum / "docs" / "design" / "briefs" / "SLEEP-REAP.md"
lane_brief = brief_path.read_text()

brief = (
    ground
    + "\n=== END OPERATING GROUND — the commissioned brief follows verbatim. "
    + "Where its run-shape sections name estate-box mechanics (worktree per "
    + "leg, the battery queue, the five-leg battery, seat-side merging and "
    + "landing), the OPERATING GROUND above OVERRIDES them for this venue. "
    + "Its REQUIREMENTS, BOUNDARIES and DECLARED FILE SURFACE are not venue "
    + "mechanics and are NOT overridden by anything. ===\n\n"
    + lane_brief
)

json.dump(
    {
        "control_prompt": (rig / "control-prompt.txt").read_text(),
        "control_expect": "0.1.0-1.89",
        "brief": brief,
        "gate_script": "/home/rocketfish/gate-sleepreap.sh",
        # Beside the gate, not derived from it: the two scripts are copied to
        # the box independently, so a path computed from `gate_script` would
        # name a file that may never have been copied. Flight 03's deliverable
        # was stranded on the box because there was no emitter at all; a
        # dispatch that composes this key while the box has no such file fails
        # LOUDLY at `patch_stranded` instead of silently re-stranding.
        "emit_script": "/home/rocketfish/emit-patch.sh",
    },
    sys.stdout,
)
